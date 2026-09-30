//! The built-in cartridge catalog and caliber derivation
//! (specs/004-cartridges-action-types research.md §1, §2 and §7).
//!
//! The catalog is `catalog.tsv`, compiled into the binary and parsed once on
//! first use. It is never written to a database and no firearm refers to it
//! (FR-002, FR-004): picking an entry copies its name and caliber onto the
//! record. [`derive_caliber`] gives a cartridge's caliber from the catalog, or
//! failing that by reading the bore designation at the start of its name.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Serialize;

use crate::services::entry_text::{entry_key, fold};

const CATALOG_TSV: &str = include_str!("catalog.tsv");

/// One built-in cartridge (data-model.md, "Catalog Cartridge").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    /// Commonness, 1 = most common.
    pub rank: u32,
    /// The spelling the catalog offers and snaps to.
    pub name: String,
    /// The bore class (FR-004a).
    pub caliber: String,
    /// Other names that narrow the list and derive the caliber, but are never
    /// snap targets (FR-013).
    pub aliases: Vec<String>,
}

/// One of the catalog's calibers: a bore class, ranked by its best-ranked
/// cartridge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogClass {
    pub spelling: String,
    pub best_rank: u32,
}

/// The parsed catalog with its lookups by entry key (research.md §3).
#[derive(Debug)]
pub struct Catalog {
    /// In rank order.
    entries: Vec<CatalogEntry>,
    /// Name and alias keys → entry index.
    by_key: HashMap<String, usize>,
    /// Name keys only → entry index.
    by_name_key: HashMap<String, usize>,
    /// In best-rank order.
    classes: Vec<CatalogClass>,
    class_by_key: HashMap<String, usize>,
    /// The key of a name's or alias's leading designation → the index of the
    /// best-ranked entry that has it (research.md §7 step 3b).
    by_designation: HashMap<String, usize>,
}

impl Catalog {
    fn parse(tsv: &str) -> Self {
        let mut entries = Vec::new();
        for (index, line) in tsv.lines().enumerate() {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            let malformed =
                |why: &str| -> ! { panic!("catalog.tsv line {}: {why}: {line:?}", index + 1) };
            let fields: Vec<&str> = line.split('\t').collect();
            let (rank, name, caliber, aliases) = match fields.as_slice() {
                [rank, name, caliber] => (rank, name, caliber, ""),
                [rank, name, caliber, aliases] => (rank, name, caliber, *aliases),
                _ => malformed("expected rank, name, caliber and aliases"),
            };
            let rank = rank.parse().unwrap_or_else(|_| malformed("the rank is not a number"));
            if name.trim().is_empty() || caliber.trim().is_empty() {
                malformed("the name and caliber must not be empty");
            }
            entries.push(CatalogEntry {
                rank,
                name: name.trim().to_owned(),
                caliber: caliber.trim().to_owned(),
                aliases: aliases
                    .split('|')
                    .map(str::trim)
                    .filter(|alias| !alias.is_empty())
                    .map(str::to_owned)
                    .collect(),
            });
        }
        entries.sort_by_key(|entry| entry.rank);

        let mut by_key = HashMap::new();
        let mut by_name_key = HashMap::new();
        let mut by_designation = HashMap::new();
        let mut classes: Vec<CatalogClass> = Vec::new();
        for (index, entry) in entries.iter().enumerate() {
            by_name_key.entry(entry_key(&entry.name)).or_insert(index);
            for spelling in std::iter::once(&entry.name).chain(&entry.aliases) {
                by_key.entry(entry_key(spelling)).or_insert(index);
                if let Some(designation) = read_designation(spelling) {
                    by_designation.entry(entry_key(&designation.text)).or_insert(index);
                }
            }
            if !classes.iter().any(|class| class.spelling == entry.caliber) {
                classes
                    .push(CatalogClass { spelling: entry.caliber.clone(), best_rank: entry.rank });
            }
        }
        let class_by_key = classes
            .iter()
            .enumerate()
            .map(|(index, class)| (entry_key(&class.spelling), index))
            .collect();

        Self { entries, by_key, by_name_key, classes, class_by_key, by_designation }
    }

    /// Every entry, most common first.
    pub fn entries(&self) -> &[CatalogEntry] {
        &self.entries
    }

    /// The catalog's calibers, each ranked by its best-ranked cartridge.
    pub fn classes(&self) -> &[CatalogClass] {
        &self.classes
    }

    /// The entry whose name or an alias has this entry key.
    pub fn entry_by_key(&self, key: &str) -> Option<&CatalogEntry> {
        self.by_key.get(key).map(|&index| &self.entries[index])
    }

    /// The entry whose name (not an alias) has this entry key: the snap
    /// target for a cartridge (research.md §6).
    pub fn entry_by_name_key(&self, key: &str) -> Option<&CatalogEntry> {
        self.by_name_key.get(key).map(|&index| &self.entries[index])
    }

    /// The class whose spelling has this entry key.
    pub fn class_by_key(&self, key: &str) -> Option<&CatalogClass> {
        self.class_by_key.get(key).map(|&index| &self.classes[index])
    }

    fn entry_by_designation(&self, key: &str) -> Option<&CatalogEntry> {
        self.by_designation.get(key).map(|&index| &self.entries[index])
    }
}

/// The catalog, parsed on first use. A malformed line panics with its line
/// number, which `tests/cartridge_catalog_test.rs` catches before a user
/// could.
pub fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| Catalog::parse(CATALOG_TSV))
}

/// Where a derived caliber came from (FR-005: a guess is marked as one).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CaliberSource {
    Catalog,
    Guess,
}

/// The caliber a cartridge derives (contracts/tauri-commands.md,
/// `settle_entry`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivedCaliber {
    pub caliber: String,
    pub source: CaliberSource,
}

/// A bore designation read from the start of a name (research.md §7's
/// table).
struct Designation {
    text: String,
    /// Read by rule 7, a bare number, which only counts when the catalog
    /// knows it (research.md §7 step 3).
    bare_number: bool,
}

/// The length of the run of ASCII digits starting at `at`.
fn digit_run(chars: &[char], at: usize) -> usize {
    chars.get(at..).map_or(0, |rest| rest.iter().take_while(|c| c.is_ascii_digit()).count())
}

fn text(chars: &[char], range: std::ops::Range<usize>) -> String {
    chars[range].iter().collect()
}

/// Whether `word` is at `at` and not followed by another letter.
fn word_at(chars: &[char], at: usize, word: &str) -> bool {
    let word: Vec<char> = word.chars().collect();
    chars.get(at..at + word.len()) == Some(word.as_slice())
        && !chars.get(at + word.len()).is_some_and(|c| c.is_alphabetic())
}

/// Skips one space or `-` at `at`, if there is one.
fn skip_separator(chars: &[char], at: usize) -> usize {
    if matches!(chars.get(at), Some(' ' | '-')) { at + 1 } else { at }
}

/// The ends of `\d{1,2}(\.\d{1,2})?` at the start: with the decimal part
/// first, then without it, as a regular expression would try them.
fn number_ends(chars: &[char]) -> Vec<usize> {
    let whole = digit_run(chars, 0);
    if !(1..=2).contains(&whole) {
        return Vec::new();
    }
    let mut ends = Vec::new();
    if chars.get(whole) == Some(&'.') && (1..=2).contains(&digit_run(chars, whole + 1)) {
        ends.push(whole + 1 + digit_run(chars, whole + 1));
    }
    ends.push(whole);
    ends
}

/// The name as research.md §7 reads it: NFKC, lower case, `×` as `x`, dashes
/// as `-`, trimmed, and a decimal comma between digits read as a point
/// (`7,62x39`).
fn designation_chars(name: &str) -> Vec<char> {
    let mut chars: Vec<char> = fold(name).trim().chars().collect();
    for index in 1..chars.len().saturating_sub(1) {
        if chars[index] == ','
            && chars[index - 1].is_ascii_digit()
            && chars[index + 1].is_ascii_digit()
        {
            chars[index] = '.';
        }
    }
    chars
}

/// research.md §7 step 2: the leading designation by the first of the seven
/// rules that matches.
fn read_designation(name: &str) -> Option<Designation> {
    let chars = designation_chars(name);
    let read = |text: String| Some(Designation { text, bare_number: false });

    // 1. `N gauge`: 1–2 digits, an optional space or `-`, then `ga`, `ga.`
    //    or `gauge`.
    let whole = digit_run(&chars, 0);
    if (1..=2).contains(&whole) {
        let at = skip_separator(&chars, whole);
        if word_at(&chars, at, "gauge") || word_at(&chars, at, "ga") {
            return read(format!("{} gauge", text(&chars, 0..whole)));
        }
    }

    // 2. `.NNN bore`: an optional `.`, 3 digits, an optional space or `-`,
    //    then `bore`.
    let start = usize::from(chars.first() == Some(&'.'));
    if digit_run(&chars, start) == 3 {
        let at = skip_separator(&chars, start + 3);
        if word_at(&chars, at, "bore") {
            return read(format!(".{} bore", text(&chars, start..start + 3)));
        }
    }

    // 3. `Nmm`: a number, an optional space, then `mm`.
    for end in number_ends(&chars) {
        let at = if chars.get(end) == Some(&' ') { end + 1 } else { end };
        if chars.get(at..at + 2) == Some(&['m', 'm']) {
            return read(format!("{}mm", text(&chars, 0..end)));
        }
    }

    // 4. `Nmm` from `NxM`: a number, optional spaces, `x`, optional spaces,
    //    a digit.
    for end in number_ends(&chars) {
        let mut at = end;
        while chars.get(at) == Some(&' ') {
            at += 1;
        }
        if chars.get(at) == Some(&'x') {
            at += 1;
            while chars.get(at) == Some(&' ') {
                at += 1;
            }
            if chars.get(at).is_some_and(char::is_ascii_digit) {
                return read(format!("{}mm", text(&chars, 0..end)));
            }
        }
    }

    // 5. `N.Nmm`: one digit, `.`, 1–2 digits, then a space, `-` or the end.
    if whole == 1 && chars.get(1) == Some(&'.') {
        let decimals = digit_run(&chars, 2);
        let end = 2 + decimals;
        if (1..=2).contains(&decimals) && matches!(chars.get(end), None | Some(' ' | '-')) {
            return read(format!("{}mm", text(&chars, 0..end)));
        }
    }

    // 6. `.NN` / `.NNN`: `.`, then 2–3 digits, then a non-digit or the end.
    if chars.first() == Some(&'.') && (2..=3).contains(&digit_run(&chars, 1)) {
        return read(text(&chars, 0..1 + digit_run(&chars, 1)));
    }

    // 7. A bare `NN` / `NNN`, then a space, `-`, `/` or the end: read as
    //    `.NN`, but only kept if the catalog knows it.
    if (2..=3).contains(&whole) && matches!(chars.get(whole), None | Some(' ' | '-' | '/')) {
        return Some(Designation {
            text: format!(".{}", text(&chars, 0..whole)),
            bare_number: true,
        });
    }

    None
}

/// The bore designation at the start of `name` by research.md §7's rules,
/// e.g. ".308" for ".308 Winchester", "7.62mm" for "7.62x39mm", or `None`.
pub fn leading_designation(name: &str) -> Option<String> {
    read_designation(name).map(|designation| designation.text)
}

/// The caliber `cartridge` derives (FR-003, FR-005; research.md §7): the
/// class of the catalog entry whose name or alias it is; otherwise a guess
/// from its leading designation, mapped to a class where the catalog knows
/// one; otherwise `None`. Never a made-up value.
pub fn derive_caliber(cartridge: &str) -> Option<DerivedCaliber> {
    let key = entry_key(cartridge);
    if key.is_empty() {
        return None;
    }
    let catalog = catalog();
    if let Some(entry) = catalog.entry_by_key(&key) {
        return Some(DerivedCaliber {
            caliber: entry.caliber.clone(),
            source: CaliberSource::Catalog,
        });
    }

    let designation = read_designation(cartridge)?;
    let designation_key = entry_key(&designation.text);
    let caliber = if let Some(class) =
        catalog.class_by_key(&designation_key).filter(|_| !designation.bare_number)
    {
        // 3a: it names a class.
        class.spelling.clone()
    } else if let Some(entry) = catalog.entry_by_designation(&designation_key) {
        // 3b: the class of the best-ranked cartridge that starts with it.
        entry.caliber.clone()
    } else if !designation.bare_number {
        // 3c: read, not invented.
        designation.text
    } else {
        // 3d: a bare number the catalog doesn't know ("16 Special").
        return None;
    };
    Some(DerivedCaliber { caliber, source: CaliberSource::Guess })
}
