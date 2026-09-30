//! specs/004-cartridges-action-types FR-002 and FR-004a: the built-in
//! cartridge catalog, checked against research.md §2's invariants so that a
//! catalog edit that breaks one fails here, not in front of a user.

use std::collections::{BTreeSet, HashMap};

use hoplodex_lib::services::cartridges::{
    CaliberSource, catalog, derive_caliber, leading_designation,
};
use hoplodex_lib::services::entry_text::entry_key;

const CATALOG_FILE: &str = include_str!("../src/services/cartridges/catalog.tsv");

/// research.md §2's class list, in its order.
const CLASSES: &[&str] = &[
    ".17",
    ".20",
    ".22",
    "6mm",
    ".25",
    "6.5mm",
    ".27",
    "7mm",
    ".30",
    ".32",
    "8mm",
    ".338",
    ".35",
    "9.3mm",
    ".357",
    "9mm",
    ".375",
    ".40",
    ".41",
    ".44",
    ".45",
    ".50",
    "10 gauge",
    "12 gauge",
    "16 gauge",
    "20 gauge",
    "28 gauge",
    ".410 bore",
];

/// research.md §2 invariant 5: entries whose name's first number is not
/// their bore.
const EXCEPTIONS: &[&str] = &[".38-40 Winchester", "7.65x53mm Argentine"];

/// research.md §2: ranks 1 to 25, which SC-001 is measured on.
const TOP_25: &[&str] = &[
    "9x19mm Parabellum",
    ".22 Long Rifle",
    ".223 Remington",
    "5.56x45mm NATO",
    "12 gauge",
    ".45 ACP",
    ".380 ACP",
    ".40 S&W",
    ".308 Winchester",
    ".38 Special",
    ".357 Magnum",
    "7.62x39mm",
    ".30-06 Springfield",
    "20 gauge",
    ".300 AAC Blackout",
    ".22 WMR",
    "6.5 Creedmoor",
    ".243 Winchester",
    ".270 Winchester",
    ".44 Magnum",
    "10mm Auto",
    ".30-30 Winchester",
    ".410 bore",
    "9x18mm Makarov",
    ".45 Colt",
];

fn digits(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| c.is_ascii_digit())
}

/// Invariant 4's grammar: `^\.\d{2,3}$`, `^\d+(\.\d)?mm$`, `^\d{1,2} gauge$`
/// or `^\.\d{3} bore$`.
fn is_class_spelling(caliber: &str) -> bool {
    if let Some(bore) = caliber.strip_suffix(" bore") {
        return bore.strip_prefix('.').is_some_and(|d| digits(d) && d.len() == 3);
    }
    if let Some(gauge) = caliber.strip_suffix(" gauge") {
        return digits(gauge) && gauge.len() <= 2;
    }
    if let Some(metric) = caliber.strip_suffix("mm") {
        return match metric.split_once('.') {
            Some((whole, tenths)) => digits(whole) && digits(tenths) && tenths.len() == 1,
            None => digits(metric),
        };
    }
    caliber.strip_prefix('.').is_some_and(|d| digits(d) && (2..=3).contains(&d.len()))
}

#[test]
fn the_file_starts_with_its_origin_and_license_header() {
    let header: Vec<&str> = CATALOG_FILE.lines().take_while(|line| line.starts_with('#')).collect();
    let header = header.join("\n");
    assert!(!header.is_empty(), "catalog.tsv must start with a # header");
    assert!(header.contains("written for HoploDex"), "the header states where it came from");
    assert!(header.contains("GPL-3.0-only"), "the header states its license");
    assert!(header.contains("SAAMI") && header.contains("C.I.P."), "research.md §1's note");
}

#[test]
fn invariant_1_ranks_run_from_1_to_n_with_at_least_200_entries() {
    let ranks: Vec<u32> = catalog().entries().iter().map(|entry| entry.rank).collect();
    assert!(ranks.len() >= 200, "only {} entries", ranks.len());
    let expected: Vec<u32> = (1..=ranks.len() as u32).collect();
    let mut sorted = ranks.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, expected, "ranks must be exactly 1..N");
}

#[test]
fn ranks_1_to_25_are_research_s_list_in_order() {
    let mut entries: Vec<_> = catalog().entries().iter().collect();
    entries.sort_by_key(|entry| entry.rank);
    let top: Vec<&str> = entries.iter().take(25).map(|entry| entry.name.as_str()).collect();
    assert_eq!(top, TOP_25);
}

#[test]
fn invariant_2_no_two_names_or_aliases_share_an_entry_key() {
    let mut seen: HashMap<String, String> = HashMap::new();
    for entry in catalog().entries() {
        for spelling in std::iter::once(&entry.name).chain(&entry.aliases) {
            if let Some(other) = seen.insert(entry_key(spelling), spelling.clone()) {
                panic!("{spelling:?} and {other:?} have the same entry key");
            }
        }
    }
}

#[test]
fn invariant_3_no_alias_has_the_key_of_a_class() {
    let class_keys: BTreeSet<String> = CLASSES.iter().map(|class| entry_key(class)).collect();
    for entry in catalog().entries() {
        for alias in &entry.aliases {
            assert!(
                !class_keys.contains(&entry_key(alias)),
                "alias {alias:?} of {:?} is spelled like a class",
                entry.name
            );
        }
    }
}

#[test]
fn invariant_4_every_caliber_is_a_class_on_research_s_list() {
    for entry in catalog().entries() {
        assert!(
            is_class_spelling(&entry.caliber),
            "{:?} has {:?}, which is not a class spelling",
            entry.name,
            entry.caliber
        );
        assert!(
            CLASSES.contains(&entry.caliber.as_str()),
            "{:?} has {:?}, which is not on research.md §2's list",
            entry.name,
            entry.caliber
        );
    }
}

#[test]
fn every_class_on_the_list_has_a_catalog_cartridge() {
    let used: BTreeSet<&str> =
        catalog().entries().iter().map(|entry| entry.caliber.as_str()).collect();
    for class in CLASSES {
        assert!(used.contains(class), "no catalog cartridge is in class {class:?}");
    }
    let classes: Vec<&str> =
        catalog().classes().iter().map(|class| class.spelling.as_str()).collect();
    assert_eq!(classes.len(), CLASSES.len(), "the catalog's calibers: {classes:?}");
}

#[test]
fn invariant_5_a_shared_leading_designation_means_a_shared_class() {
    let mut by_designation: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for entry in catalog().entries() {
        if EXCEPTIONS.contains(&entry.name.as_str()) {
            continue;
        }
        for spelling in std::iter::once(&entry.name).chain(&entry.aliases) {
            if let Some(designation) = leading_designation(spelling) {
                by_designation
                    .entry(entry_key(&designation))
                    .or_default()
                    .push((spelling.clone(), entry.caliber.clone()));
            }
        }
    }
    for (designation, spellings) in by_designation {
        let classes: BTreeSet<&str> = spellings.iter().map(|(_, class)| class.as_str()).collect();
        assert_eq!(classes.len(), 1, "designation {designation:?} spans classes: {spellings:?}");
    }
}

#[test]
fn every_catalog_name_has_a_leading_designation() {
    for entry in catalog().entries() {
        assert!(leading_designation(&entry.name).is_some(), "{:?} has none", entry.name);
    }
}

#[test]
fn invariant_6_an_improved_variant_is_guessed_into_its_own_class() {
    for entry in catalog().entries() {
        if EXCEPTIONS.contains(&entry.name.as_str()) {
            continue;
        }
        let variant = format!("{} Improved", entry.name);
        let derived =
            derive_caliber(&variant).unwrap_or_else(|| panic!("{variant:?} derived no caliber"));
        assert_eq!(
            (derived.caliber.as_str(), derived.source),
            (entry.caliber.as_str(), CaliberSource::Guess),
            "{variant:?}"
        );
    }
}

#[test]
fn the_named_exceptions_exist_and_really_are_exceptions() {
    for name in EXCEPTIONS {
        let entry = catalog()
            .entries()
            .iter()
            .find(|entry| entry.name == *name)
            .unwrap_or_else(|| panic!("{name:?} is not in the catalog"));
        let guessed = derive_caliber(&format!("{name} Improved")).map(|d| d.caliber);
        assert_ne!(guessed.as_deref(), Some(entry.caliber.as_str()), "{name:?} needs no exception");
    }
}

#[test]
fn research_s_aliases_are_in_the_catalog() {
    let alias_of = |alias: &str| {
        catalog()
            .entries()
            .iter()
            .find(|entry| entry.aliases.iter().any(|a| entry_key(a) == entry_key(alias)))
            .map(|entry| entry.name.as_str())
    };
    for (alias, name) in [
        (".22 LR", ".22 Long Rifle"),
        ("22LR", ".22 Long Rifle"),
        ("9mm Luger", "9x19mm Parabellum"),
        ("9mm Para", "9x19mm Parabellum"),
        ("9mm NATO", "9x19mm Parabellum"),
        ("9x19", "9x19mm Parabellum"),
        (".300 BLK", ".300 AAC Blackout"),
        (".300 Blackout", ".300 AAC Blackout"),
        ("5.56 NATO", "5.56x45mm NATO"),
        ("5.56", "5.56x45mm NATO"),
        ("7.65mm Browning", ".32 ACP"),
        (".410", ".410 bore"),
        ("410 gauge", ".410 bore"),
        ("12 ga", "12 gauge"),
    ] {
        assert_eq!(alias_of(alias), Some(name), "{alias:?}");
    }
}

#[test]
fn class_ranks_are_their_best_ranked_cartridge() {
    let classes = catalog().classes();
    let rank_of = |spelling: &str| {
        classes.iter().find(|class| class.spelling == spelling).map(|class| class.best_rank)
    };
    assert_eq!(rank_of("9mm"), Some(1));
    assert_eq!(rank_of(".22"), Some(2));
    assert_eq!(rank_of("12 gauge"), Some(5));
    assert_eq!(rank_of(".357"), Some(10));
}
