//! specs/004-cartridges-action-types FR-005, FR-007 and SC-002: the caliber
//! derived from a cartridge, from the catalog or by the guess of research.md
//! §7, is exactly what the hand-written corpus expects, and no guess is made
//! where none is expected.

use hoplodex_lib::services::cartridges::{CaliberSource, derive_caliber};

const CORPUS: &str = include_str!("fixtures/caliber_guess_corpus.tsv");

/// A corpus line: its number, the name, then the expected caliber and
/// source, or `None` for no guess.
type CorpusLine = (usize, &'static str, Option<(&'static str, CaliberSource)>);

fn corpus() -> Vec<CorpusLine> {
    CORPUS
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.starts_with('#') && !line.is_empty())
        .map(|(index, line)| {
            let number = index + 1;
            let columns: Vec<&str> = line.split('\t').collect();
            let expected = match columns.as_slice() {
                [_, "none"] => None,
                [_, caliber, "catalog"] => Some((*caliber, CaliberSource::Catalog)),
                [_, caliber, "guess"] => Some((*caliber, CaliberSource::Guess)),
                _ => panic!("corpus line {number} is malformed: {line:?}"),
            };
            (number, columns[0], expected)
        })
        .collect()
}

#[test]
fn the_corpus_covers_the_names_research_lists() {
    let names: Vec<&str> = corpus().iter().map(|(_, name, _)| *name).collect();
    for name in [
        ".22 LR",
        ".22 Long Rifle",
        ".17 HMR",
        ".300 BLK",
        ".308 Improved",
        ".30 Custom Improved",
        "7.62x39",
        "7,62x39",
        "6.5x47 Wildcat",
        "6.5 Creedmoor Ackley",
        "9x19",
        "9mm",
        "9mm Luger",
        ".380 Custom",
        ".45 ACP",
        "45-70 Wildcat",
        "12 gauge",
        "12ga",
        "16 gauge",
        ".410 bore",
        "410",
        "Wildcat Special",
        "Hornet",
        "16 Special",
        ".19 Calhoon",
        "14 gauge",
        "",
    ] {
        assert!(names.contains(&name), "the corpus is missing {name:?}");
    }
    assert!(names.len() >= 90, "the corpus has only {} names", names.len());
}

#[test]
fn every_corpus_name_derives_exactly_the_expected_caliber() {
    let mut failures = Vec::new();
    for (line, name, expected) in corpus() {
        let actual = derive_caliber(name).map(|derived| (derived.caliber, derived.source));
        let expected = expected.map(|(caliber, source)| (caliber.to_owned(), source));
        if actual != expected {
            failures.push(format!("line {line}, {name:?}: expected {expected:?}, got {actual:?}"));
        }
    }
    assert!(failures.is_empty(), "{} wrong:\n{}", failures.len(), failures.join("\n"));
}
