//! Suggestion ranking and snapping for make, model, cartridge and caliber
//! (specs/004-cartridges-action-types research.md §4 and §6).
//!
//! The vocabulary is read from the tables at call time and kept nowhere
//! (FR-011): a value whose last record was deleted is gone from the next
//! call. Every comparison goes through the entry key
//! ([`crate::services::entry_text`]), so what counts as the same value here
//! is exactly what it counts as when saving and importing.
//!
//! A suggestion call reads the values on record once and keeps only those
//! that match what was typed, with the words of each value in buffers
//! reused from one to the next: at 20,000 distinct models over two tables,
//! building and dropping the whole vocabulary on every keystroke (a `String`
//! per word, per key and per value) took more than the query itself, on
//! Windows, whose allocator is slower than Linux's (specs/007 T162).

use std::cmp::Reverse;
use std::collections::HashMap;
use std::iter::once;
use std::sync::OnceLock;

use rusqlite::Connection;
use serde::Serialize;

use crate::services::cartridges::catalog;
use crate::services::entry_text::{EntryField, EntryWords, MAX_ENTRY_CHARS, check_entry_text};
use crate::services::registration::BUILT_IN_FORMS;

/// The most suggestions one call returns (research.md §4).
pub const MAX_SUGGESTIONS: usize = 20;

/// Why a settled value differs from what was typed (research.md §6): it was
/// snapped to the catalog's spelling or to one already on record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangedBy {
    Catalog,
    Record,
}

/// One row of the suggestion list (contracts/tauri-commands.md).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    /// The display spelling: the catalog's when it has one.
    pub value: String,
    /// Built in (FR-016).
    pub in_catalog: bool,
    /// Firearms on record, active and disposed, across every spelling;
    /// 0 = catalog only.
    pub use_count: i64,
    /// Catalog cartridges only: the bore class, as a hint.
    pub caliber: Option<String>,
}

/// One spelling of a value on record, and how many firearms use exactly it.
#[derive(Debug, Clone)]
struct Spelling {
    text: String,
    count: i64,
    /// The lowest firearm id using it: the earliest recorded.
    min_id: i64,
}

/// The spellings on record that share one entry key.
#[derive(Debug)]
struct Group {
    /// The first spelling seen; the others are in `more`, which most groups
    /// never allocate.
    first: Spelling,
    more: Vec<Spelling>,
    use_count: i64,
}

impl Group {
    fn new(text: &str, count: i64, min_id: i64) -> Self {
        Self {
            first: Spelling { text: text.to_owned(), count, min_id },
            more: Vec::new(),
            use_count: count,
        }
    }

    /// Adds the records using `text`, a spelling that may be known already.
    fn add(&mut self, text: &str, count: i64, min_id: i64) {
        self.use_count += count;
        match once(&mut self.first)
            .chain(self.more.iter_mut())
            .find(|spelling| spelling.text == text)
        {
            Some(spelling) => {
                spelling.count += count;
                spelling.min_id = spelling.min_id.min(min_id);
            }
            None => self.more.push(Spelling { text: text.to_owned(), count, min_id }),
        }
    }

    /// The spelling used by the most records; the earliest recorded on a
    /// tie (spec Edge Cases).
    fn display(&self) -> &str {
        once(&self.first)
            .chain(&self.more)
            // The ids of the two tables overlap, so the same count and first
            // id can come from two spellings; the later text then wins, as
            // it did when the rows came ordered by text.
            .max_by_key(|spelling| (spelling.count, Reverse(spelling.min_id), &spelling.text))
            .map(|spelling| spelling.text.as_str())
            .unwrap_or_default()
    }
}

/// The values of one field on record (active and disposed firearms and
/// accessories), grouped by entry key. It is what [`snap`] looks a value up
/// in, and what an import snapshots when it starts; a suggestion call reads
/// the values itself ([`suggest`]).
#[derive(Debug)]
pub struct FieldVocabulary {
    field: EntryField,
    groups: HashMap<String, Group>,
}

/// The queries that read every value of `field` on record, one per table that
/// holds it, each with how many records use the value and the first of them.
/// A table's query is its own, not one over both tables joined by
/// `UNION ALL`: it then reads the table's covering index in order and groups
/// as it goes, where the joined one sorted all the rows into a temporary
/// tree (about half of the 20,000 models' time at 10,000 firearms and
/// 10,000 accessories, on Windows over the 50ms of SC-004). The two tables'
/// groups are merged in [`FieldVocabulary::load`].
pub fn vocabulary_queries(field: EntryField) -> Vec<String> {
    let column = field.column();
    // The shared fields are on both tables (006 FR-003, research.md §16);
    // the registration fields are the firearms' alone.
    let tables: &[&str] = match field {
        EntryField::Make | EntryField::Model | EntryField::Caliber | EntryField::Cartridge => {
            &["firearms", "accessories"]
        }
        EntryField::RegistrationForm | EntryField::RegisteredTo => &["firearms"],
    };
    tables
        .iter()
        .map(|table| {
            // A make is NULL on an accessory that has none; its model still
            // counts.
            if field == EntryField::Model {
                format!(
                    "SELECT {column}, make, COUNT(*), MIN(id) FROM {table}
                     WHERE {column} IS NOT NULL GROUP BY make, {column}"
                )
            } else {
                format!(
                    "SELECT {column}, NULL, COUNT(*), MIN(id) FROM {table}
                     WHERE {column} IS NOT NULL GROUP BY {column}"
                )
            }
        })
        .collect()
}

/// Makes the first suggestion after a database opens as fast as the later
/// ones (SC-004's 50ms; 004 research.md §5): reads what each field's
/// suggestions read, once, so the connection's page cache already holds the
/// pages (a fresh connection decrypts each page from the file on first
/// touch, which on macOS cost 123ms for the first Make query at 10,000
/// firearms), and builds the catalog's index, which the first cartridge or
/// caliber suggestion would otherwise build. The results are dropped, so
/// nothing is kept (FR-011).
pub fn warm(conn: &Connection) -> rusqlite::Result<()> {
    let _ = catalog_index();
    for field in EntryField::ALL {
        for sql in vocabulary_queries(field) {
            conn.query_row(&format!("SELECT COUNT(*) FROM ({sql})"), [], |row| {
                row.get::<_, i64>(0)
            })?;
        }
    }
    Ok(())
}

/// Calls `visit` with each row of `field`'s queries: the value, its make
/// (Model only, and only when the record has one), how many records use it
/// and the first of them. The text is borrowed from the row.
fn for_each_row(
    conn: &Connection,
    field: EntryField,
    mut visit: impl FnMut(&str, Option<&str>, i64, i64),
) -> rusqlite::Result<()> {
    for sql in vocabulary_queries(field) {
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let text = row.get_ref(0)?.as_str()?;
            let make = row.get_ref(1)?.as_str_or_null()?;
            visit(text, make, row.get(2)?, row.get(3)?);
        }
    }
    Ok(())
}

impl FieldVocabulary {
    pub fn load(conn: &Connection, field: EntryField) -> rusqlite::Result<Self> {
        let mut groups: HashMap<String, Group> = HashMap::new();
        let mut words = EntryWords::default();
        for_each_row(conn, field, |text, _make, count, min_id| {
            words.set(text);
            if words.key().is_empty() {
                return;
            }
            match groups.get_mut(words.key()) {
                Some(group) => group.add(text, count, min_id),
                None => {
                    groups.insert(words.key().to_owned(), Group::new(text, count, min_id));
                }
            }
        })?;
        Ok(Self { field, groups })
    }
}

fn words_key(text: &str) -> String {
    EntryWords::new(text).key().to_owned()
}

/// The spelling the catalog or the vocabulary already has for `key`, and
/// which of the two it is: the catalog first (research.md §6).
fn known_spelling<'a>(vocabulary: &'a FieldVocabulary, key: &str) -> Option<(&'a str, ChangedBy)> {
    let from_catalog = match vocabulary.field {
        EntryField::Cartridge => catalog().entry_by_name_key(key).map(|entry| entry.name.as_str()),
        EntryField::Caliber => catalog().class_by_key(key).map(|class| class.spelling.as_str()),
        EntryField::RegistrationForm => {
            BUILT_IN_FORMS.iter().copied().find(|name| words_key(name) == key)
        }
        EntryField::Make | EntryField::Model | EntryField::RegisteredTo => None,
    };
    if let Some(spelling) = from_catalog {
        return Some((spelling, ChangedBy::Catalog));
    }
    vocabulary.groups.get(key).map(|group| (group.display(), ChangedBy::Record))
}

/// Steps 1–2 of research.md §6: what `text` becomes in the vocabulary's
/// field. Returns the trimmed text unchanged when nothing matches, and the
/// spelling that is already in use when it does.
pub fn snap(vocabulary: &FieldVocabulary, text: &str) -> (String, Option<ChangedBy>) {
    let trimmed = text.trim();
    if trimmed.chars().count() > MAX_ENTRY_CHARS {
        return (trimmed.to_owned(), None);
    }
    let key = words_key(trimmed);
    if key.is_empty() {
        return (trimmed.to_owned(), None);
    }
    match known_spelling(vocabulary, &key) {
        Some((spelling, by)) => (spelling.to_owned(), (spelling != trimmed).then_some(by)),
        None => (trimmed.to_owned(), None),
    }
}

/// One spelling met in a sheet's cells, and how many cells use it.
struct SheetSpelling {
    text: String,
    count: usize,
    /// The first cell (in row order) using it.
    first: usize,
}

/// The import's sheet pass for one field (contracts/spreadsheet-format.md
/// "Entry rules and snapping on import", FR-026): the spelling most of the
/// sheet's cells use for each entry key, the earliest row winning a tie. It
/// is consulted only for a value the catalog and the start-of-import
/// snapshot don't know.
#[derive(Default)]
pub struct SheetSpellings {
    chosen: HashMap<String, String>,
}

impl SheetSpellings {
    /// `cells` are the field's cells in row order. Blank cells, and cells
    /// that break FR-015's rules (a row error of their own), don't vote.
    pub fn from_cells<'a>(field: EntryField, cells: impl IntoIterator<Item = &'a str>) -> Self {
        let mut by_key: HashMap<String, Vec<SheetSpelling>> = HashMap::new();
        for (index, cell) in cells.into_iter().enumerate() {
            let text = cell.trim();
            if text.is_empty() || check_entry_text(field, text).is_err() {
                continue;
            }
            let key = words_key(text);
            if key.is_empty() {
                continue;
            }
            let spellings = by_key.entry(key).or_default();
            match spellings.iter_mut().find(|spelling| spelling.text == text) {
                Some(spelling) => spelling.count += 1,
                None => {
                    spellings.push(SheetSpelling { text: text.to_owned(), count: 1, first: index })
                }
            }
        }
        let chosen = by_key
            .into_iter()
            .filter_map(|(key, spellings)| {
                spellings
                    .into_iter()
                    .max_by_key(|spelling| (spelling.count, Reverse(spelling.first)))
                    .map(|spelling| (key, spelling.text))
            })
            .collect();
        Self { chosen }
    }
}

/// [`snap`] for an import row: the catalog, then a value on record when the
/// import started (`vocabulary`), then the sheet's own majority spelling.
/// Returns the trimmed text unchanged when none knows it.
pub fn snap_for_import(vocabulary: &FieldVocabulary, sheet: &SheetSpellings, text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() > MAX_ENTRY_CHARS {
        return trimmed.to_owned();
    }
    let key = words_key(trimmed);
    if key.is_empty() {
        return trimmed.to_owned();
    }
    if let Some((spelling, _)) = known_spelling(vocabulary, &key) {
        return spelling.to_owned();
    }
    sheet.chosen.get(&key).cloned().unwrap_or_else(|| trimmed.to_owned())
}

const PREFIX: u8 = 1;
const WORD_PREFIX: u8 = 2;
const INITIALISM_OR_ALIAS: u8 = 3;

/// Each typed word is a prefix of a later word of the candidate, in order;
/// a single typed word matches any word (research.md §4, tier 2).
fn word_prefix_match(typed: &EntryWords, candidate: &EntryWords) -> bool {
    let mut wanted = typed.words().peekable();
    if wanted.peek().is_none() {
        return false;
    }
    for word in candidate.words() {
        if let Some(next) = wanted.peek()
            && word.starts_with(next)
        {
            wanted.next();
            if wanted.peek().is_none() {
                return true;
            }
        }
    }
    false
}

/// The best tier at which `typed` matches a value with these words
/// (research.md §4). Empty text matches everything at tier 1.
fn text_tier(typed: &EntryWords, candidate: &EntryWords) -> Option<u8> {
    if candidate.key().starts_with(typed.key()) {
        Some(PREFIX)
    } else if word_prefix_match(typed, candidate) {
        Some(WORD_PREFIX)
    } else if typed.key().chars().count() >= 2 && initialism_starts_with(candidate, typed.key()) {
        Some(INITIALISM_OR_ALIAS)
    } else {
        None
    }
}

/// Whether the first characters of the candidate's words begin with `prefix`.
fn initialism_starts_with(candidate: &EntryWords, prefix: &str) -> bool {
    let mut firsts = candidate.words().filter_map(|word| word.chars().next());
    prefix.chars().all(|c| firsts.next() == Some(c))
}

/// The entry keys of a field's built-in values, each with a slot to count
/// the records that use it in: a value on record with one of these keys
/// takes the built-in spelling and appears once (US2-4).
#[derive(Default)]
struct KeySlots {
    by_key: HashMap<String, usize>,
}

impl KeySlots {
    /// The slot of `key`; two built-in values with one key share it.
    fn slot(&mut self, key: &str) -> usize {
        let next = self.by_key.len();
        *self.by_key.entry(key.to_owned()).or_insert(next)
    }
}

/// A catalog cartridge with the forms it is matched by, worked out once.
struct CatalogCartridge {
    name: &'static str,
    rank: u32,
    caliber: &'static str,
    words: EntryWords,
    aliases: Vec<EntryWords>,
    class_key: String,
    slot: usize,
}

/// A catalog caliber (a bore class).
struct CatalogCaliber {
    spelling: &'static str,
    rank: u32,
    words: EntryWords,
    slot: usize,
}

/// A built-in registration form name, in rank order.
struct CatalogForm {
    name: &'static str,
    words: EntryWords,
    slot: usize,
}

struct CatalogIndex {
    cartridges: Vec<CatalogCartridge>,
    cartridge_slots: KeySlots,
    calibers: Vec<CatalogCaliber>,
    caliber_slots: KeySlots,
    forms: Vec<CatalogForm>,
    form_slots: KeySlots,
}

fn catalog_index() -> &'static CatalogIndex {
    static INDEX: OnceLock<CatalogIndex> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut cartridge_slots = KeySlots::default();
        let cartridges = catalog()
            .entries()
            .iter()
            .map(|entry| {
                let words = EntryWords::new(&entry.name);
                CatalogCartridge {
                    name: &entry.name,
                    rank: entry.rank,
                    caliber: &entry.caliber,
                    slot: cartridge_slots.slot(words.key()),
                    words,
                    aliases: entry.aliases.iter().map(|alias| EntryWords::new(alias)).collect(),
                    class_key: words_key(&entry.caliber),
                }
            })
            .collect();
        let mut caliber_slots = KeySlots::default();
        let calibers = catalog()
            .classes()
            .iter()
            .map(|class| {
                let words = EntryWords::new(&class.spelling);
                CatalogCaliber {
                    spelling: &class.spelling,
                    rank: class.best_rank,
                    slot: caliber_slots.slot(words.key()),
                    words,
                }
            })
            .collect();
        let mut form_slots = KeySlots::default();
        let forms = BUILT_IN_FORMS
            .iter()
            .copied()
            .map(|name| {
                let words = EntryWords::new(name);
                CatalogForm { name, slot: form_slots.slot(words.key()), words }
            })
            .collect();
        CatalogIndex { cartridges, cartridge_slots, calibers, caliber_slots, forms, form_slots }
    })
}

/// One row before it is ordered. It borrows its text, so only the rows that
/// make the cut are copied.
struct Ranked<'a> {
    tier: u8,
    use_count: i64,
    rank: u32,
    value: &'a str,
    caliber: Option<&'a str>,
    in_catalog: bool,
}

fn best(current: Option<u8>, tier: Option<u8>) -> Option<u8> {
    match (current, tier) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

/// A value on record that matches what was typed.
struct Candidate {
    group: Group,
    tier: u8,
    /// Model only: the model is recorded with the make on the form.
    make_matched: bool,
}

/// The ranked suggestions for `text` in the field (research.md §4): at most
/// [`MAX_SUGGESTIONS`], best first, from the values on record now and the
/// built-in ones. `make` is the make on the form: for a model, only models
/// recorded with that make are offered, so a make that isn't on record
/// offers none; a blank make offers them all. Text over [`MAX_ENTRY_CHARS`]
/// characters matches nothing.
pub fn suggest(
    conn: &Connection,
    field: EntryField,
    text: &str,
    make: Option<&str>,
) -> rusqlite::Result<Vec<Suggestion>> {
    if text.chars().count() > MAX_ENTRY_CHARS {
        return Ok(Vec::new());
    }
    let typed = EntryWords::new(text);
    // Model only: the entered make's key. A make that is on no record
    // matches no model, so no record's make equals it.
    let make_filter = (field == EntryField::Model)
        .then(|| make.map(words_key).filter(|key| !key.is_empty()))
        .flatten();
    let index = catalog_index();
    let slots = match field {
        EntryField::Cartridge => Some(&index.cartridge_slots),
        EntryField::Caliber => Some(&index.caliber_slots),
        EntryField::RegistrationForm => Some(&index.form_slots),
        EntryField::Make | EntryField::Model | EntryField::RegisteredTo => None,
    };
    // How many records use each built-in value, in its slot.
    let mut slot_counts = vec![0i64; slots.map_or(0, |slots| slots.by_key.len())];

    // The values on record that match, by entry key. A row that doesn't
    // match is not kept, and neither is one of a built-in value, which the
    // built-in's own row stands for.
    let mut candidates: HashMap<String, Candidate> = HashMap::new();
    let mut words = EntryWords::default();
    let mut make_words = EntryWords::default();
    for_each_row(conn, field, |value, row_make, count, min_id| {
        words.set(value);
        let key = words.key();
        if key.is_empty() {
            return;
        }
        if let Some(&slot) = slots.and_then(|slots| slots.by_key.get(key)) {
            slot_counts[slot] += count;
            return;
        }
        let Some(tier) = text_tier(&typed, &words) else { return };
        let make_matched = match &make_filter {
            None => true,
            Some(filter) => row_make.is_some_and(|row_make| {
                make_words.set(row_make);
                make_words.key() == filter
            }),
        };
        match candidates.get_mut(key) {
            Some(candidate) => {
                candidate.group.add(value, count, min_id);
                candidate.tier = candidate.tier.min(tier);
                candidate.make_matched |= make_matched;
            }
            None => {
                candidates.insert(
                    key.to_owned(),
                    Candidate { group: Group::new(value, count, min_id), tier, make_matched },
                );
            }
        }
    })?;

    let mut ranked: Vec<Ranked> = Vec::new();
    // The built-in values first, so a value on record with the same key
    // takes the built-in spelling and appears once (US2-4).
    let mut offer = |name: &'static str,
                     slot: usize,
                     rank: u32,
                     caliber: Option<&'static str>,
                     tier: Option<u8>| {
        let Some(tier) = tier else { return };
        ranked.push(Ranked {
            tier,
            use_count: slot_counts[slot],
            rank,
            value: name,
            caliber,
            in_catalog: true,
        });
    };
    match field {
        EntryField::Cartridge => {
            for entry in &index.cartridges {
                let mut tier = text_tier(&typed, &entry.words);
                if entry.aliases.iter().any(|alias| text_tier(&typed, alias).is_some()) {
                    tier = best(tier, Some(INITIALISM_OR_ALIAS));
                }
                // The bore class: typing exactly it lists its cartridges,
                // most common first (US2-3); typing part of it narrows to
                // them at the lowest tier.
                if !typed.key().is_empty() && entry.class_key.starts_with(typed.key()) {
                    let class_tier =
                        if entry.class_key == typed.key() { PREFIX } else { INITIALISM_OR_ALIAS };
                    tier = best(tier, Some(class_tier));
                }
                offer(entry.name, entry.slot, entry.rank, Some(entry.caliber), tier);
            }
        }
        EntryField::Caliber => {
            for class in &index.calibers {
                offer(
                    class.spelling,
                    class.slot,
                    class.rank,
                    None,
                    text_tier(&typed, &class.words),
                );
            }
        }
        // The built-in form names, in rank order (data-model.md "Built-in
        // Form Name"); only the typed text decides which are offered.
        EntryField::RegistrationForm => {
            for (rank, form) in index.forms.iter().enumerate() {
                offer(form.name, form.slot, rank as u32 + 1, None, text_tier(&typed, &form.words));
            }
        }
        EntryField::Make | EntryField::Model | EntryField::RegisteredTo => {}
    }

    for candidate in candidates.values().filter(|candidate| candidate.make_matched) {
        ranked.push(Ranked {
            tier: candidate.tier,
            use_count: candidate.group.use_count,
            rank: u32::MAX,
            value: candidate.group.display(),
            caliber: None,
            in_catalog: false,
        });
    }

    let order = |a: &Ranked, b: &Ranked| {
        (a.tier, a.use_count == 0, Reverse(a.use_count), a.rank)
            .cmp(&(b.tier, b.use_count == 0, Reverse(b.use_count), b.rank))
            .then_with(|| a.value.cmp(b.value))
    };
    // Only the best twenty are ordered: with no text typed, every value
    // on record is a match.
    if ranked.len() > MAX_SUGGESTIONS {
        ranked.select_nth_unstable_by(MAX_SUGGESTIONS, order);
        ranked.truncate(MAX_SUGGESTIONS);
    }
    ranked.sort_by(order);
    Ok(ranked
        .into_iter()
        .map(|ranked| Suggestion {
            value: ranked.value.to_owned(),
            in_catalog: ranked.in_catalog,
            use_count: ranked.use_count,
            caliber: ranked.caliber.map(str::to_owned),
        })
        .collect())
}
