//! Released migration steps and stored release databases stay byte-for-byte
//! unchanged (009 US4-2, 009 FR-021, 009 SC-006, 009 research.md §11).
//!
//! `src/db/migrations/released.txt` lists, at each release's tag (#19), every
//! step that release shipped and every database it stored under
//! `tests/fixtures/releases/`: `<name or fixture path> <sha256 of its bytes>
//! <release>`, `#` starting a comment. [`problems`] is the check, taking the
//! manifest's text, the build's steps and a reader for fixtures, so a test can
//! hand it doctored inputs and see each one fail. The real manifest is empty
//! until 0.1.0 is tagged, and passes.

use std::path::Path;

use hoplodex_lib::db;
use sha2::{Digest, Sha256};

/// Said at the end of every problem, so whoever sees one knows the rule.
const RULE: &str = "CLAUDE.md: from the 0.1.0 tag on, a released migration step and a stored release \
                    database never change; make the change a new step in src/db/migrations/ (and \
                    its release_upgrade_test case) instead (009 FR-021, research.md §11)";

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// A line of the manifest.
enum Entry<'a> {
    Step { name: &'a str, sha256: &'a str, release: &'a str },
    Fixture { path: &'a str, sha256: &'a str, release: &'a str },
}

/// Every problem with `manifest`, given the steps the build has and a reader
/// that returns a stored fixture's bytes by its path (`None` when there is no
/// such file). Empty when the released steps are unchanged, are the first
/// entries of `steps` in order, and each stored fixture still hashes to its line.
fn problems(
    manifest: &str,
    steps: &[(&str, &str)],
    read_fixture: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> Vec<String> {
    let mut found = Vec::new();
    let mut problem = |message: String| found.push(format!("{message}. {RULE}"));

    let mut entries = Vec::new();
    for (index, line) in manifest.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        let well_formed = |sha: &str| sha.len() == 64 && sha.bytes().all(|b| b.is_ascii_hexdigit());
        match fields[..] {
            [name, sha256, release]
                if well_formed(sha256) && sha256 == sha256.to_ascii_lowercase() =>
            {
                if name.ends_with(".hoplodex") {
                    entries.push(Entry::Fixture { path: name, sha256, release });
                } else {
                    entries.push(Entry::Step { name, sha256, release });
                }
            }
            _ => problem(format!(
                "released.txt line {} is malformed (expected `<step name or fixture path> \
                 <64 lowercase hex digits of its sha256> <release>`): {line:?}",
                index + 1
            )),
        }
    }

    let mut listed_steps = Vec::new();
    for entry in &entries {
        match entry {
            Entry::Step { name, sha256: expected, release } => {
                if listed_steps.contains(name) {
                    problem(format!("released step {name} is listed twice"));
                    continue;
                }
                listed_steps.push(*name);
                match steps.iter().find(|(step, _)| step == name) {
                    None => problem(format!(
                        "step {name}, released in {release}, is missing from the build's steps \
                         (removed or renamed)"
                    )),
                    Some((_, sql)) if sha256(sql.as_bytes()) != *expected => problem(format!(
                        "step {name}, released in {release}, no longer hashes to its line in \
                         released.txt: its bytes changed"
                    )),
                    Some(_) => {}
                }
            }
            Entry::Fixture { path, sha256: expected, release } => match read_fixture(path) {
                None => problem(format!(
                    "the stored {release} database {path} is missing or unreadable"
                )),
                Some(bytes) if sha256(&bytes) != *expected => problem(format!(
                    "the stored {release} database {path} no longer hashes to its line in \
                     released.txt: its bytes changed"
                )),
                Some(_) => {}
            },
        }
    }

    // The released steps are the build's first entries, in the order listed.
    let first: Vec<&str> = steps.iter().take(listed_steps.len()).map(|(name, _)| *name).collect();
    let all_present = listed_steps.iter().all(|name| steps.iter().any(|(step, _)| step == name));
    if all_present && first != listed_steps {
        problem(format!(
            "the released steps {listed_steps:?} are not the first steps of the build, in that \
             order (the build starts with {first:?})"
        ));
    }
    found
}

fn real_manifest() -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/db/migrations/released.txt"),
    )
    .expect("src/db/migrations/released.txt")
}

fn read_real_fixture(path: &str) -> Option<Vec<u8>> {
    std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).ok()
}

/// 009 US4-2, 009 FR-021, 009 SC-006: the real manifest holds against this
/// build (today it lists nothing; from the 0.1.0 tag it lists the steps and the
/// stored database).
#[test]
fn the_real_manifest_holds_against_this_build() {
    let found = problems(&real_manifest(), db::MIGRATIONS, &read_real_fixture);
    assert!(found.is_empty(), "{}", found.join("\n"));
}

// --- Doctored inputs: each must fail ---------------------------------------

const FIXTURE: &str = "tests/fixtures/releases/9.9.9.hoplodex";
const FIXTURE_BYTES: &[u8] = b"a stored release database";

/// A step list standing in for a build that has shipped two steps and is
/// working on a third.
fn steps() -> Vec<(&'static str, &'static str)> {
    vec![
        ("0001_first", "CREATE TABLE a (id INTEGER PRIMARY KEY);"),
        ("0002_second", "CREATE TABLE b (id INTEGER PRIMARY KEY);"),
        ("0003_unreleased", "CREATE TABLE c (id INTEGER PRIMARY KEY);"),
    ]
}

/// The manifest 9.9.9's tag would add for the first two steps and the fixture.
fn manifest() -> String {
    let mut text = String::from("# header\n\n");
    for (name, sql) in steps().iter().take(2) {
        text.push_str(&format!("{name} {} 9.9.9\n", sha256(sql.as_bytes())));
    }
    text.push_str(&format!("{FIXTURE} {} 9.9.9\n", sha256(FIXTURE_BYTES)));
    text
}

fn fixture_reader(path: &str) -> Option<Vec<u8>> {
    (path == FIXTURE).then(|| FIXTURE_BYTES.to_vec())
}

fn assert_fails(found: Vec<String>, mentions: &[&str]) {
    assert!(!found.is_empty(), "the doctored input passed");
    let all = found.join("\n");
    for text in mentions {
        assert!(all.contains(text), "{text:?} is not in:\n{all}");
    }
    assert!(
        found.iter().all(|message| message.contains("CLAUDE.md")),
        "every failure names CLAUDE.md's rule:\n{all}"
    );
}

#[test]
fn the_untouched_manifest_passes() {
    let found = problems(&manifest(), &steps(), &fixture_reader);
    assert!(found.is_empty(), "{found:?}");
    // An empty manifest, and one of only comments, pass too (today's).
    assert!(problems("", &steps(), &fixture_reader).is_empty());
    assert!(problems("# only a comment\n", &[], &fixture_reader).is_empty());
}

#[test]
fn a_listed_step_whose_bytes_changed_fails() {
    let mut changed = steps();
    changed[1].1 = "CREATE TABLE b (id INTEGER PRIMARY KEY, extra TEXT);";
    assert_fails(
        problems(&manifest(), &changed, &fixture_reader),
        &["0002_second", "bytes changed"],
    );
    // A single byte, a trailing newline, is a change.
    let mut newline = steps();
    newline[0].1 = "CREATE TABLE a (id INTEGER PRIMARY KEY);\n";
    assert_fails(problems(&manifest(), &newline, &fixture_reader), &["0001_first"]);
}

#[test]
fn a_listed_step_missing_from_the_build_fails() {
    let mut missing = steps();
    missing.remove(1);
    assert_fails(problems(&manifest(), &missing, &fixture_reader), &["0002_second", "missing"]);
    assert_fails(problems(&manifest(), &[], &fixture_reader), &["0001_first", "0002_second"]);
}

#[test]
fn a_renamed_listed_step_fails() {
    let mut renamed = steps();
    renamed[0].0 = "0001_first_renamed";
    assert_fails(problems(&manifest(), &renamed, &fixture_reader), &["0001_first", "missing"]);
}

#[test]
fn released_steps_out_of_order_or_not_the_first_entries_fail() {
    // The build lists them the other way round.
    let mut swapped = steps();
    swapped.swap(0, 1);
    assert_fails(problems(&manifest(), &swapped, &fixture_reader), &["not the first steps"]);

    // The manifest lists them the other way round.
    let lines: Vec<String> = manifest().lines().map(str::to_owned).collect();
    let (first, second) = (lines[2].clone(), lines[3].clone());
    let reversed =
        manifest().replace(&format!("{first}\n{second}\n"), &format!("{second}\n{first}\n"));
    assert_ne!(reversed, manifest());
    assert_fails(problems(&reversed, &steps(), &fixture_reader), &["not the first steps"]);

    // A new step slipped in before a released one.
    let mut inserted = steps();
    inserted.insert(1, ("0001b_inserted", "CREATE TABLE z (id INTEGER PRIMARY KEY);"));
    assert_fails(problems(&manifest(), &inserted, &fixture_reader), &["not the first steps"]);
}

#[test]
fn a_listed_fixture_whose_bytes_changed_or_is_missing_fails() {
    let changed = |path: &str| (path == FIXTURE).then(|| b"another database".to_vec());
    assert_fails(problems(&manifest(), &steps(), &changed), &[FIXTURE, "bytes changed"]);
    let none = |_: &str| None;
    assert_fails(problems(&manifest(), &steps(), &none), &[FIXTURE, "missing"]);
}

#[test]
fn a_malformed_line_fails() {
    let good = manifest();
    for (bad, why) in [
        ("0003_unreleased", "no hash or release"),
        ("0003_unreleased abc 9.9.9", "a short hash"),
        (&format!("0003_unreleased {} 9.9.9", "g".repeat(64)), "a hash that is not hex"),
        (&format!("0003_unreleased {} 9.9.9", "A".repeat(64)), "an uppercase hash"),
        (&format!("0003_unreleased {} 9.9.9 extra", "a".repeat(64)), "a fourth field"),
        (&format!("0003_unreleased {}", "a".repeat(64)), "no release"),
    ] {
        let found = problems(&format!("{good}{bad}\n"), &steps(), &fixture_reader);
        assert!(
            found.iter().any(|message| message.contains("malformed")),
            "a line with {why} was not reported: {found:?}"
        );
        assert_fails(found, &["released.txt line"]);
    }
}

#[test]
fn a_step_listed_twice_fails() {
    let duplicated = format!("{}{}\n", manifest(), manifest().lines().nth(2).unwrap());
    assert_fails(problems(&duplicated, &steps(), &fixture_reader), &["listed twice"]);
}
