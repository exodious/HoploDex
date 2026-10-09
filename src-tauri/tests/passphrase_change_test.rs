//! Changing a database's passphrase by copy, verify and replace (FR-015,
//! FR-016, SC-004; research.md §3, §4). Real SQLCipher files in temp
//! directories, a manual clock, and machine settings in a temp config
//! directory.

mod support;

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::backups::ops as backups_ops;
use hoplodex_lib::commands::documents::ops as documents;
use hoplodex_lib::commands::firearms::ops as firearms;
use hoplodex_lib::commands::photos::ops as photos;
use hoplodex_lib::db;
use hoplodex_lib::models::database::{CloseReason, PassphraseChanged};
use hoplodex_lib::models::record::RecordRef;
use hoplodex_lib::services::backups;
use hoplodex_lib::services::disk_space;
use hoplodex_lib::services::file_swap;
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::services::passphrase::Passphrase;
use hoplodex_lib::session::{Session, lifecycle};
use rusqlite::Connection;
use serde_json::json;
use support::{ManualClock, TestEvents, passphrase, sample_png_bytes, test_session_at};
use tempfile::TempDir;

const START: &str = "2026-09-25T14:30:05+02:00";
const NEW_PASSPHRASE: &str = "a much longer passphrase of several words";
const PDF_BYTES: &[u8] = b"%PDF-1.4\n% a tiny document\n%%EOF\n";

struct World {
    dir: TempDir,
    _config: TempDir,
    scratch: TempDir,
    machine: MachineSettings,
    session: Session,
    events: Arc<TestEvents>,
    clock: Arc<ManualClock>,
}

impl World {
    /// "Mine", created with the test passphrase, holding a firearm with a
    /// photo and a document, and left open.
    fn new() -> Self {
        let config = TempDir::new().unwrap();
        let clock = ManualClock::at(START);
        let (session, events) =
            test_session_at(&config.path().join("opened-documents"), clock.clone());
        let world = Self {
            dir: TempDir::new().unwrap(),
            scratch: TempDir::new().unwrap(),
            machine: MachineSettings::load(config.path()).unwrap(),
            _config: config,
            session,
            events,
            clock,
        };
        lifecycle::create(&world.session, &world.machine, &world.path(), &passphrase()).unwrap();
        world.add_firearm("A1");
        world
    }

    fn path(&self) -> PathBuf {
        self.dir.path().join("Mine.hoplodex")
    }

    fn add_firearm(&self, serial: &str) {
        self.session
            .write(|conn| {
                let firearm = firearms::create_firearm(
                    conn,
                    &support::firearm("Colt", "Python", serial),
                    false,
                    None,
                )?;
                photos::add_photo(
                    conn,
                    RecordRef::Firearm(firearm.id),
                    &sample_png_bytes(),
                    "front.png",
                    "image/png",
                )?;
                documents::add_document(conn, RecordRef::Firearm(firearm.id), PDF_BYTES, "bill.pdf")
                    .map(|_| ())
            })
            .unwrap();
    }

    fn change(&self, current: &str, new: &str) -> Result<PassphraseChanged, CommandError> {
        backups_ops::change_passphrase(
            &self.session,
            &self.machine,
            self.scratch.path(),
            &Passphrase::from_input(current.to_owned()),
            &Passphrase::from_input(new.to_owned()),
        )
    }

    fn change_to_new(&self) -> Result<PassphraseChanged, CommandError> {
        self.change(support::TEST_PASSPHRASE, NEW_PASSPHRASE)
    }

    fn open_with(&self, text: &str) -> Result<(), CommandError> {
        let typed = Passphrase::from_input(text.to_owned());
        lifecycle::open(&self.session, &self.machine, &self.path(), &typed, false)
    }

    fn close(&self) {
        lifecycle::close_normal(&self.session, &self.machine, CloseReason::Closed).unwrap();
    }

    /// Every file beside the database whose name marks it as temporary.
    fn leftovers(&self) -> Vec<String> {
        let mut found: Vec<String> = fs::read_dir(self.dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains(".new") || n.ends_with(".old"))
            .collect();
        found.sort();
        found
    }

    fn collection(&self) -> Vec<String> {
        self.session.read(|conn| Ok(collection(conn))).unwrap()
    }

    fn row_counts(&self) -> BTreeMap<String, i64> {
        self.session.read(|conn| Ok(row_counts(conn))).unwrap()
    }
}

/// The collection as rows: every collection table, every column (blobs
/// included), in order. Housekeeping is left out.
fn collection(conn: &Connection) -> Vec<String> {
    let mut rows = Vec::new();
    for table in [
        "firearms",
        "photos",
        "document_attachments",
        "disposition_history",
        "insurance_policies",
        "firearm_types",
        "collection_settings",
    ] {
        let mut stmt = conn.prepare(&format!("SELECT * FROM {table} ORDER BY rowid")).unwrap();
        let columns = stmt.column_count();
        let found = stmt
            .query_map([], |row| {
                let values: Vec<String> =
                    (0..columns).map(|i| format!("{:?}", row.get_ref(i).unwrap())).collect();
                Ok(format!("{table}: {}", values.join(" | ")))
            })
            .unwrap();
        rows.extend(found.map(Result::unwrap));
    }
    rows
}

/// The row count of every table the file has, the search index's shadow
/// tables included.
fn row_counts(conn: &Connection) -> BTreeMap<String, i64> {
    let mut stmt = conn
        .prepare(
            "SELECT name FROM pragma_table_list
             WHERE schema = 'main' AND type IN ('table', 'shadow') ORDER BY name",
        )
        .unwrap();
    let tables: Vec<String> =
        stmt.query_map([], |row| row.get(0)).unwrap().map(Result::unwrap).collect();
    tables
        .into_iter()
        .map(|table| {
            let count = conn
                .query_row(&format!("SELECT count(*) FROM \"{table}\""), [], |row| row.get(0))
                .unwrap();
            (table, count)
        })
        .collect()
}

// --- A change that succeeds (FR-015, FR-016) --------------------------------

#[test]
fn afterwards_it_opens_with_the_new_passphrase_and_refuses_the_old() {
    let world = World::new();

    world.change_to_new().unwrap();

    world.close();
    let refused = world.open_with(support::TEST_PASSPHRASE).unwrap_err();
    assert_eq!(refused.code, "PASSPHRASE_INCORRECT");
    world.open_with(NEW_PASSPHRASE).unwrap();
}

#[test]
fn every_table_and_every_blob_is_the_same_afterwards() {
    let world = World::new();
    world.add_firearm("B2");
    let counts = world.row_counts();
    let rows = world.collection();

    world.change_to_new().unwrap();

    assert_eq!(world.row_counts(), counts, "SC-004");
    assert_eq!(world.collection(), rows);
    assert!(rows.iter().any(|row| row.starts_with("photos: ")), "photo blobs included");
    assert!(rows.iter().any(|row| row.starts_with("document_attachments: ")), "documents too");
    let found = world
        .session
        .read(|conn| {
            conn.query_row(
                "SELECT count(*) FROM firearms_fts WHERE firearms_fts MATCH 'Python'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(CommandError::from_db)
        })
        .unwrap();
    assert_eq!(found, 2, "the search index came across");
}

#[test]
fn the_previous_file_is_gone_and_no_temporary_file_is_left() {
    let world = World::new();

    let changed = world.change_to_new().unwrap();

    assert_eq!(
        changed,
        PassphraseChanged { old_file_removed: true, old_file_path: None, passphrase_saved: false }
    );
    assert!(world.leftovers().is_empty(), "{:?}", world.leftovers());
    assert!(!file_swap::old_path(&world.path()).exists());
    assert!(!file_swap::new_path(&world.path()).exists());
}

#[test]
fn the_change_counts_as_a_change_so_the_next_close_backs_it_up() {
    let world = World::new();
    // Backed up at this close, so nothing is waiting any more.
    world.close();
    world.open_with(support::TEST_PASSPHRASE).unwrap();
    let folder = world.dir.path().join("HoploDex backups");
    let id = world.session.inspect(|open| Ok(open.database_id.clone())).unwrap();
    assert_eq!(backups::list(&folder, &id).unwrap().len(), 1);

    world.change_to_new().unwrap();

    let waiting: bool = world
        .session
        .read(|conn| {
            conn.query_row("SELECT changes_waiting FROM app_state", [], |row| row.get(0))
                .map_err(CommandError::from_db)
        })
        .unwrap();
    assert!(waiting, "US3-9");
    // A day later, past the once-a-day limit.
    world.clock.advance(chrono::Duration::days(1));
    world.close();
    assert_eq!(backups::list(&folder, &id).unwrap().len(), 2);
}

#[test]
fn the_session_stays_open_and_takes_writes_afterwards() {
    let world = World::new();

    world.change_to_new().unwrap();

    assert!(world.session.is_open());
    world.add_firearm("C3");
    let firearms: i64 = world
        .session
        .read(|conn| {
            conn.query_row("SELECT count(*) FROM firearms", [], |row| row.get(0))
                .map_err(CommandError::from_db)
        })
        .unwrap();
    assert_eq!(firearms, 2, "the fingerprint was refreshed, so the write went through");
}

#[test]
fn progress_reports_copying_then_checking_then_replacing() {
    let world = World::new();

    world.change_to_new().unwrap();

    let reported = world.events.payloads("passphrase_change:progress");
    let phases: Vec<&str> = reported.iter().map(|p| p["phase"].as_str().unwrap()).collect();
    let copying = phases.iter().take_while(|phase| **phase == "copying").count();
    assert!(copying >= 2, "at the start and at the end at least: {phases:?}");
    assert_eq!(&phases[copying..], ["checking", "replacing"]);

    let processed: Vec<u64> =
        reported[..copying].iter().map(|p| p["processed"].as_u64().unwrap()).collect();
    let total = reported[0]["total"].as_u64().unwrap();
    assert!(total > 0);
    assert_eq!(processed[0], 0);
    assert!(processed.windows(2).all(|pair| pair[0] <= pair[1]), "growing: {processed:?}");
    assert_eq!(*processed.last().unwrap(), total);
    for indeterminate in &reported[copying..] {
        assert_eq!(indeterminate["total"], json!(0));
    }
}

// --- Refusals before anything is written ------------------------------------

#[test]
fn a_wrong_current_passphrase_is_refused_on_its_field_and_changes_nothing() {
    let world = World::new();
    let before = fs::read(world.path()).unwrap();

    let refused = world.change("not the passphrase at all", NEW_PASSPHRASE).unwrap_err();

    assert_eq!(refused.code, "PASSPHRASE_INCORRECT");
    assert!(refused.field_errors.as_ref().unwrap().contains_key("currentPassphrase"));
    assert_eq!(fs::read(world.path()).unwrap(), before, "the file is byte-identical");
    assert!(world.leftovers().is_empty());
    assert!(
        fs::read_dir(world.scratch.path()).unwrap().next().is_none(),
        "the page-1 probe is removed"
    );
    assert!(world.session.is_open());
}

#[test]
fn a_short_new_passphrase_is_refused_on_its_field() {
    let world = World::new();

    let refused = world.change(support::TEST_PASSPHRASE, "too short").unwrap_err();

    assert_eq!(refused.code, "VALIDATION_ERROR");
    assert_eq!(
        refused.field_errors.as_ref().unwrap().get("newPassphrase").map(String::as_str),
        Some("Use at least 12 characters.")
    );
    assert!(world.leftovers().is_empty());
}

#[test]
fn the_current_passphrase_is_refused_as_the_new_one_before_anything_is_written() {
    let world = World::new();
    let before = fs::read(world.path()).unwrap();

    let refused = world.change(support::TEST_PASSPHRASE, support::TEST_PASSPHRASE).unwrap_err();

    assert_eq!(refused.code, "VALIDATION_ERROR");
    assert_eq!(
        refused.field_errors.as_ref().unwrap().get("newPassphrase").map(String::as_str),
        Some("Choose a passphrase different from the current one.")
    );
    assert_eq!(fs::read(world.path()).unwrap(), before, "the file is byte-identical");
    assert!(world.leftovers().is_empty());
    assert!(world.events.payloads("passphrase_change:progress").is_empty());
}

#[test]
fn a_new_passphrase_differing_only_in_unicode_form_is_the_same_one() {
    let world = World::new();
    world.change(support::TEST_PASSPHRASE, "caf\u{e9} au lait, extra hot").unwrap();

    // "é" as one code point, then as "e" and a combining acute accent.
    let refused =
        world.change("caf\u{e9} au lait, extra hot", "cafe\u{301} au lait, extra hot").unwrap_err();

    assert_eq!(
        refused.field_errors.as_ref().unwrap().get("newPassphrase").map(String::as_str),
        Some("Choose a passphrase different from the current one.")
    );
}

#[test]
fn too_little_space_is_refused_before_anything_is_written() {
    let world = World::new();
    let before = fs::read(world.path()).unwrap();
    let _space = disk_space::testing::fake_available_space(|_| Some(4096));

    let refused = world.change_to_new().unwrap_err();

    assert_eq!(refused.code, "INSUFFICIENT_SPACE");
    let details = refused.details.as_deref().unwrap();
    assert!(details["bytesNeeded"].as_u64().unwrap() > before.len() as u64, "file size + 5%");
    assert_eq!(details["bytesAvailable"], json!(4096));
    assert_eq!(fs::read(world.path()).unwrap(), before);
    assert!(world.leftovers().is_empty());
    assert!(world.events.payloads("passphrase_change:progress").is_empty());
}

#[test]
fn unresolved_pending_changes_are_resolved_first() {
    let world = World::new();
    world
        .session
        .write(|conn| {
            conn.execute(
                "INSERT INTO pending_changes (id, kind, mode, target_id, label, form_version,
                                              values_json, saved_at)
                 VALUES (1, 'firearm', 'add', NULL, 'New firearm', 1, '{}', ?1)",
                [db::now_utc()],
            )
            .map_err(CommandError::from_db)
        })
        .unwrap();
    world.close();
    world.open_with(support::TEST_PASSPHRASE).unwrap();

    let refused = world.change_to_new().unwrap_err();

    assert_eq!(refused.code, "PENDING_CHANGES_UNRESOLVED", "the copy would drop them (FR-039)");
    assert!(world.leftovers().is_empty());
}

// --- Interruptions (SC-004) --------------------------------------------------

/// Adds about 48 MiB of document to the open database, so the copy runs
/// long enough to be stopped while it is under way.
fn make_large(world: &World) {
    world
        .session
        .write(|conn| {
            conn.execute(
                "INSERT INTO document_attachments
                     (firearm_id, file_bytes, original_filename, mime_type, created_at)
                 VALUES (1, randomblob(48 * 1024 * 1024), 'large.pdf', 'application/pdf',
                         datetime('now'))",
                [],
            )
            .map(|_| ())
            .map_err(CommandError::from_db)
        })
        .unwrap();
}

/// Stops the change at the first `passphrase_change:progress` that `when`
/// picks, then checks the database is open, unchanged, and still opens with
/// the old passphrase and all its content.
fn change_stopped_when(
    world: World,
    when: impl Fn(&serde_json::Value) -> bool + Send + Sync + 'static,
) {
    let rows = world.collection();
    let operations = world.session.operations_handle();
    world.events.on_event(move |event, payload| {
        if event == "passphrase_change:progress" && when(payload) {
            operations.stop_running();
        }
    });

    let stopped = world.change_to_new().unwrap_err();

    assert_eq!(stopped.code, "OPERATION_STOPPED");
    assert_eq!(stopped.details.as_deref().unwrap()["operation"], json!("passphraseChange"));
    assert!(world.leftovers().is_empty(), "no .new is left: {:?}", world.leftovers());
    assert!(world.session.is_open());
    assert_eq!(world.collection(), rows);
    world.add_firearm("D4");
    world.close();
    world.open_with(support::TEST_PASSPHRASE).unwrap();
    assert!(world.collection().len() > rows.len(), "the old passphrase still opens it");
    assert_eq!(world.open_with(NEW_PASSPHRASE).unwrap_err().code, "PASSPHRASE_INCORRECT");
}

#[test]
fn a_change_stopped_during_the_copy_changes_nothing() {
    let world = World::new();
    make_large(&world);
    change_stopped_when(world, |payload| {
        payload["phase"] == "copying"
            && payload["processed"].as_u64().unwrap() > 0
            && payload["processed"] != payload["total"]
    });
}

#[test]
fn a_change_stopped_during_the_check_changes_nothing() {
    change_stopped_when(World::new(), |payload| payload["phase"] == "checking");
}

#[test]
fn a_change_stopped_just_before_the_replacement_changes_nothing() {
    change_stopped_when(World::new(), |payload| payload["phase"] == "replacing");
}

#[test]
fn leftovers_beside_a_database_go_only_once_its_passphrase_has_opened_it() {
    // #63: recovery ran before the passphrase was checked.
    let world = World::new();
    world.close();
    let old = file_swap::old_path(&world.path());
    let new = file_swap::new_path(&world.path());
    fs::write(&old, b"left by a crash").unwrap();
    fs::write(&new, b"left by a crash").unwrap();

    let refused = world.open_with("not the passphrase at all").unwrap_err();

    assert_eq!(refused.code, "PASSPHRASE_INCORRECT");
    assert!(old.exists() && new.exists(), "a refused open changes nothing");
    world.open_with(support::TEST_PASSPHRASE).unwrap();
    assert!(!old.exists() && !new.exists(), "an open that succeeded tidies up");
}

#[test]
fn a_swap_interrupted_between_its_renames_is_completed_by_the_next_open() {
    let world = World::new();
    world.close();
    // The two-rename fallback stopped after the first rename: the old file
    // waits as `.old`, and the verified copy as `.new`.
    let old = file_swap::old_path(&world.path());
    let new = file_swap::new_path(&world.path());
    fs::copy(world.path(), &new).unwrap();
    fs::rename(world.path(), &old).unwrap();

    world.open_with(support::TEST_PASSPHRASE).unwrap();

    assert!(world.path().exists());
    assert!(!old.exists() && !new.exists());
}

#[cfg(unix)]
#[test]
fn a_symlink_planted_at_the_copys_name_is_not_followed_and_the_change_fails() {
    // #63: `.new` was truncated through a link, and then removed.
    let world = World::new();
    let rows = world.collection();
    let victim = world.scratch.path().join("victim.txt");
    fs::write(&victim, b"precious").unwrap();
    let planted = file_swap::new_path(&world.path());
    std::os::unix::fs::symlink(&victim, &planted).unwrap();

    let refused = world.change_to_new().unwrap_err();

    assert_eq!(refused.code, "INTERNAL_ERROR");
    assert_eq!(fs::read(&victim).unwrap(), b"precious", "the target is unchanged");
    assert!(fs::symlink_metadata(&planted).is_ok(), "the link is not ours to remove");
    assert!(world.session.is_open());
    assert_eq!(world.collection(), rows);
    world.close();
    world.open_with(support::TEST_PASSPHRASE).unwrap();
}

#[test]
fn a_file_planted_at_the_copys_name_is_left_alone_and_the_change_fails() {
    let world = World::new();
    let planted = file_swap::new_path(&world.path());
    fs::write(&planted, b"somebody else's file").unwrap();

    let refused = world.change_to_new().unwrap_err();

    assert_eq!(refused.code, "INTERNAL_ERROR");
    assert_eq!(fs::read(&planted).unwrap(), b"somebody else's file");
    assert!(world.session.is_open());
}

#[test]
fn a_refused_replacement_keeps_the_old_file_and_the_session_open() {
    let world = World::new();
    let rows = world.collection();
    let _refuse = file_swap::testing::fail_final_rename(std::time::Duration::from_millis(50));

    let refused = world.change_to_new().unwrap_err();

    assert_eq!(refused.code, "REPLACE_FAILED");
    assert!(world.leftovers().is_empty(), "{:?}", world.leftovers());
    assert!(world.session.is_open(), "reopened with the current passphrase");
    assert_eq!(world.collection(), rows);
    world.close();
    world.open_with(support::TEST_PASSPHRASE).unwrap();
}

// --- The previous file (US4-5) -----------------------------------------------

#[test]
fn an_old_file_that_cannot_be_deleted_is_reported_with_its_path() {
    let world = World::new();
    let _keep = file_swap::testing::keep_old_copy();

    let changed = world.change_to_new().unwrap();

    let old = file_swap::old_path(&world.path());
    assert!(!changed.old_file_removed);
    assert_eq!(changed.old_file_path.as_deref(), Some(old.to_str().unwrap()));
    assert!(old.exists());
    let still_old = support::peek(&old);
    assert!(
        still_old.query_row("SELECT count(*) FROM firearms", [], |r| r.get::<_, i64>(0)).is_ok(),
        "it opens with the old passphrase"
    );
}

// --- Serialization -----------------------------------------------------------

#[test]
fn the_answer_is_camel_case_and_leaves_out_a_path_it_does_not_have() {
    assert_eq!(
        serde_json::to_value(PassphraseChanged {
            old_file_removed: true,
            old_file_path: None,
            passphrase_saved: false,
        })
        .unwrap(),
        json!({ "oldFileRemoved": true, "passphraseSaved": false })
    );
    let kept = PassphraseChanged {
        old_file_removed: false,
        old_file_path: Some("/data/.Mine.hoplodex.old".into()),
        passphrase_saved: false,
    };
    assert_eq!(
        serde_json::to_value(kept).unwrap(),
        json!({
            "oldFileRemoved": false,
            "oldFilePath": "/data/.Mine.hoplodex.old",
            "passphraseSaved": false,
        })
    );
}
