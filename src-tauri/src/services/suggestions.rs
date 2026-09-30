//! Suggestion ranking and snapping for make, model, cartridge and caliber
//! (specs/004-cartridges-action-types research.md §4 and §6).
//!
//! The vocabulary is read from `firearms` at call time and kept nowhere
//! (FR-011): a value whose last firearm was deleted is gone from the next
//! call. Every comparison goes through the entry key
//! ([`crate::services::entry_text`]), so what counts as the same value here
//! is exactly what it counts as when saving and importing.

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use rusqlite::Connection;
use serde::Serialize;

use crate::services::cartridges::catalog;
use crate::services::entry_text::{EntryField, MAX_ENTRY_CHARS, words};

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
    /// The words of the first spelling seen, for matching.
    words: Vec<String>,
    spellings: Vec<Spelling>,
    use_count: i64,
    /// Model only: the makes it is recorded with, by id in
    /// [`FieldVocabulary::make_ids`].
    makes: Vec<u32>,
}

impl Group {
    /// The spelling used by the most firearms; the earliest recorded on a
    /// tie (spec Edge Cases).
    fn display(&self) -> &str {
        self.spellings
            .iter()
            .max_by_key(|spelling| (spelling.count, Reverse(spelling.min_id)))
            .map(|spelling| spelling.text.as_str())
            .unwrap_or_default()
    }
}

/// The values of one field on record (active and disposed firearms), grouped
/// by entry key.
#[derive(Debug)]
pub struct FieldVocabulary {
    field: EntryField,
    groups: HashMap<String, Group>,
    /// Model only: an id for each make's entry key.
    make_ids: HashMap<String, u32>,
}

impl FieldVocabulary {
    pub fn load(conn: &Connection, field: EntryField) -> rusqlite::Result<Self> {
        let column = field.column();
        let sql = if field == EntryField::Model {
            "SELECT model, make, COUNT(*), MIN(id) FROM firearms GROUP BY make, model".to_owned()
        } else {
            format!(
                "SELECT {column}, NULL, COUNT(*), MIN(id) FROM firearms
                 WHERE {column} IS NOT NULL GROUP BY {column}"
            )
        };
        let mut make_ids: HashMap<String, u32> = HashMap::new();
        // A make's text → its id, so each distinct make is keyed once.
        let mut make_texts: HashMap<String, u32> = HashMap::new();
        let mut groups: HashMap<String, Group> = HashMap::new();
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let text: String = row.get(0)?;
            let make: Option<String> = row.get(1)?;
            let (count, min_id): (i64, i64) = (row.get(2)?, row.get(3)?);
            let words = words(&text);
            let key = words.concat();
            if key.is_empty() {
                continue;
            }
            let group = groups.entry(key).or_insert_with(|| Group {
                words,
                spellings: Vec::new(),
                use_count: 0,
                makes: Vec::new(),
            });
            group.use_count += count;
            match group.spellings.iter_mut().find(|spelling| spelling.text == text) {
                Some(spelling) => {
                    spelling.count += count;
                    spelling.min_id = spelling.min_id.min(min_id);
                }
                None => group.spellings.push(Spelling { text, count, min_id }),
            }
            if let Some(make) = make {
                let id = match make_texts.get(&make) {
                    Some(&id) => id,
                    None => {
                        let next = make_ids.len() as u32;
                        let id = *make_ids.entry(words_key(&make)).or_insert(next);
                        make_texts.insert(make, id);
                        id
                    }
                };
                if !group.makes.contains(&id) {
                    group.makes.push(id);
                }
            }
        }
        Ok(Self { field, groups, make_ids })
    }
}

fn words_key(text: &str) -> String {
    words(text).concat()
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
    let from_catalog = match vocabulary.field {
        EntryField::Cartridge => catalog().entry_by_name_key(&key).map(|entry| entry.name.as_str()),
        EntryField::Caliber => catalog().class_by_key(&key).map(|class| class.spelling.as_str()),
        EntryField::Make | EntryField::Model => None,
    };
    if let Some(spelling) = from_catalog {
        return (spelling.to_owned(), (spelling != trimmed).then_some(ChangedBy::Catalog));
    }
    if let Some(group) = vocabulary.groups.get(&key) {
        let spelling = group.display();
        return (spelling.to_owned(), (spelling != trimmed).then_some(ChangedBy::Record));
    }
    (trimmed.to_owned(), None)
}

/// What the user has typed, as the comparison form of research.md §3.
struct Typed {
    key: String,
    words: Vec<String>,
}

impl Typed {
    fn new(text: &str) -> Self {
        let words = words(text);
        Self { key: words.concat(), words }
    }
}

const PREFIX: u8 = 1;
const WORD_PREFIX: u8 = 2;
const INITIALISM_OR_ALIAS: u8 = 3;

/// Each typed word is a prefix of a later word of the candidate, in order;
/// a single typed word matches any word (research.md §4, tier 2).
fn word_prefix_match(typed: &[String], candidate: &[String]) -> bool {
    match typed {
        [] => false,
        [word] => candidate.iter().any(|w| w.starts_with(word.as_str())),
        _ => {
            let mut next = 0;
            for word in candidate {
                if word.starts_with(typed[next].as_str()) {
                    next += 1;
                    if next == typed.len() {
                        return true;
                    }
                }
            }
            false
        }
    }
}

/// The best tier at which `typed` matches a value with these words
/// (research.md §4). Empty text matches everything at tier 1.
fn text_tier(typed: &Typed, key: &str, words: &[String]) -> Option<u8> {
    if key.starts_with(&typed.key) {
        Some(PREFIX)
    } else if word_prefix_match(&typed.words, words) {
        Some(WORD_PREFIX)
    } else if typed.key.chars().count() >= 2 && initialism_of(words).starts_with(&typed.key) {
        Some(INITIALISM_OR_ALIAS)
    } else {
        None
    }
}

fn initialism_of(words: &[String]) -> String {
    words.iter().filter_map(|word| word.chars().next()).collect()
}

/// A catalog cartridge with the forms it is matched by, worked out once.
struct CatalogCartridge {
    name: &'static str,
    rank: u32,
    caliber: &'static str,
    key: String,
    words: Vec<String>,
    alias_words: Vec<(String, Vec<String>)>,
    class_key: String,
}

/// A catalog caliber (a bore class).
struct CatalogCaliber {
    spelling: &'static str,
    rank: u32,
    key: String,
    words: Vec<String>,
}

struct CatalogIndex {
    cartridges: Vec<CatalogCartridge>,
    calibers: Vec<CatalogCaliber>,
}

fn catalog_index() -> &'static CatalogIndex {
    static INDEX: OnceLock<CatalogIndex> = OnceLock::new();
    INDEX.get_or_init(|| CatalogIndex {
        cartridges: catalog()
            .entries()
            .iter()
            .map(|entry| CatalogCartridge {
                name: &entry.name,
                rank: entry.rank,
                caliber: &entry.caliber,
                key: words_key(&entry.name),
                words: words(&entry.name),
                alias_words: entry
                    .aliases
                    .iter()
                    .map(|alias| {
                        let words = words(alias);
                        (words.concat(), words)
                    })
                    .collect(),
                class_key: words_key(&entry.caliber),
            })
            .collect(),
        calibers: catalog()
            .classes()
            .iter()
            .map(|class| CatalogCaliber {
                spelling: &class.spelling,
                rank: class.best_rank,
                key: words_key(&class.spelling),
                words: words(&class.spelling),
            })
            .collect(),
    })
}

/// One row before it is ordered. It borrows its text, so only the rows that
/// make the cut are copied.
struct Ranked<'a> {
    with_make: bool,
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

/// The ranked suggestions for `text` in the field (research.md §4): at most
/// [`MAX_SUGGESTIONS`], best first. `make` is the make on the form, for
/// ranking a model. Text over [`MAX_ENTRY_CHARS`] characters matches
/// nothing.
pub fn suggest(vocabulary: &FieldVocabulary, text: &str, make: Option<&str>) -> Vec<Suggestion> {
    if text.chars().count() > MAX_ENTRY_CHARS {
        return Vec::new();
    }
    let typed = Typed::new(text);
    let make_id = make.map(words_key).and_then(|key| vocabulary.make_ids.get(&key).copied());
    let with_make = |group: &Group| make_id.is_some_and(|id| group.makes.contains(&id));

    let mut ranked: Vec<Ranked> = Vec::new();
    let mut consumed: HashSet<String> = HashSet::new();
    let index = catalog_index();

    // The catalog's values first, so a value on record with the same key
    // takes the catalog's spelling and appears once (US2-4).
    let mut offer =
        |name: &'static str, key: &str, rank: u32, caliber: Option<&'static str>, tier| {
            let use_count = vocabulary.groups.get(key).map_or(0, |group| group.use_count);
            consumed.insert(key.to_owned());
            let Some(tier) = tier else { return };
            ranked.push(Ranked {
                with_make: false,
                tier,
                use_count,
                rank,
                value: name,
                caliber,
                in_catalog: true,
            });
        };
    match vocabulary.field {
        EntryField::Cartridge => {
            for entry in &index.cartridges {
                let mut tier = text_tier(&typed, &entry.key, &entry.words);
                if entry
                    .alias_words
                    .iter()
                    .any(|(key, words)| text_tier(&typed, key, words).is_some())
                {
                    tier = best(tier, Some(INITIALISM_OR_ALIAS));
                }
                // The bore class: typing exactly it lists its cartridges,
                // most common first (US2-3); typing part of it narrows to
                // them at the lowest tier.
                if !typed.key.is_empty() && entry.class_key.starts_with(&typed.key) {
                    let class_tier =
                        if entry.class_key == typed.key { PREFIX } else { INITIALISM_OR_ALIAS };
                    tier = best(tier, Some(class_tier));
                }
                offer(entry.name, &entry.key, entry.rank, Some(entry.caliber), tier);
            }
        }
        EntryField::Caliber => {
            for class in &index.calibers {
                offer(
                    class.spelling,
                    &class.key,
                    class.rank,
                    None,
                    text_tier(&typed, &class.key, &class.words),
                );
            }
        }
        EntryField::Make | EntryField::Model => {}
    }

    for (key, group) in &vocabulary.groups {
        if consumed.contains(key) {
            continue;
        }
        let Some(tier) = text_tier(&typed, key, &group.words) else { continue };
        ranked.push(Ranked {
            with_make: with_make(group),
            tier,
            use_count: group.use_count,
            rank: u32::MAX,
            value: group.display(),
            caliber: None,
            in_catalog: false,
        });
    }

    ranked.sort_by(|a, b| {
        (Reverse(a.with_make), a.tier, a.use_count == 0, Reverse(a.use_count), a.rank)
            .cmp(&(Reverse(b.with_make), b.tier, b.use_count == 0, Reverse(b.use_count), b.rank))
            .then_with(|| a.value.cmp(b.value))
    });
    ranked
        .into_iter()
        .take(MAX_SUGGESTIONS)
        .map(|ranked| Suggestion {
            value: ranked.value.to_owned(),
            in_catalog: ranked.in_catalog,
            use_count: ranked.use_count,
            caliber: ranked.caliber.map(str::to_owned),
        })
        .collect()
}
