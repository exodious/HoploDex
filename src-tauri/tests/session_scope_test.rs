//! A session names its requests (008 FR-004; research.md §2): every accessor
//! of `Session` takes the id of the session the caller means, compares it
//! with the open database's under the session's mutex, and refuses
//! `DATABASE_CLOSED`, running nothing, when it isn't the open one. Real
//! SQLCipher files in temp directories; machine settings in a temp config
//! directory.

mod support;

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::backups::ops as backups_ops;
use hoplodex_lib::commands::databases::ops as databases;
use hoplodex_lib::commands::firearms::ops as firearms;
use hoplodex_lib::commands_list::{COMMANDS, UNSCOPED_COMMANDS};
use hoplodex_lib::models::database::{CloseReason, Draft, DraftKind, DraftMode};
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::services::passphrase::Passphrase;
use hoplodex_lib::session::scoped::{SESSION_HEADER, ScopedSession, Target};
use hoplodex_lib::session::{Session, SessionId, lifecycle};
use serde_json::json;
use support::{TestEvents, passphrase, test_session};
use tempfile::TempDir;

/// A throwaway world: a folder for databases, a config directory for
/// `machine.json`, and a session reporting to a recorder.
struct World {
    dir: TempDir,
    config: TempDir,
    machine: MachineSettings,
    session: Session,
    events: Arc<TestEvents>,
}

impl World {
    fn new() -> Self {
        let config = TempDir::new().unwrap();
        let (session, events) = test_session(&config.path().join("opened-documents"));
        Self {
            dir: TempDir::new().unwrap(),
            machine: MachineSettings::load(config.path()).unwrap(),
            config,
            session,
            events,
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(format!("{name}.hoplodex"))
    }

    /// Creates `name` and leaves it open. Returns its session.
    fn create(&self, name: &str) -> SessionId {
        lifecycle::create(&self.session, &self.machine, &self.path(name), &passphrase()).unwrap();
        self.session.current_id()
    }

    /// Opens `name`, closing any other database first. Returns its session.
    fn open(&self, name: &str) -> SessionId {
        lifecycle::open(&self.session, &self.machine, &self.path(name), &passphrase(), false)
            .unwrap();
        self.session.current_id()
    }

    fn close(&self, id: SessionId) {
        lifecycle::close_normal(&self.session, id, &self.machine, CloseReason::Closed).unwrap();
    }

    fn scoped(&self, id: SessionId) -> ScopedSession {
        ScopedSession { session: self.session.clone(), id }
    }

    fn add_firearm(&self, id: SessionId, serial: &str) -> Result<(), CommandError> {
        self.session
            .write(id, |conn| {
                firearms::create_firearm(
                    conn,
                    &support::firearm("Colt", "Python", serial),
                    false,
                    None,
                )
            })
            .map(|_| ())
    }

    fn count_pending(&self, id: SessionId) -> i64 {
        self.session
            .read(id, |conn| {
                conn.query_row("SELECT count(*) FROM pending_changes", [], |r| r.get(0))
                    .map_err(CommandError::from_db)
            })
            .unwrap()
    }
}

fn draft() -> Draft {
    Draft {
        form_version: 1,
        kind: DraftKind::Firearm,
        mode: DraftMode::Add,
        target_id: None,
        label: "Colt Python (new)".into(),
        values: json!({ "make": "Colt", "model": "Python" }),
    }
}

/// An id that was never issued in this run: far beyond the counter.
fn never_issued() -> SessionId {
    SessionId::from_request(u64::MAX - 7).unwrap()
}

/// Calls every accessor with `id` and says which ran its closure. A refusal
/// must be `DATABASE_CLOSED` with the closure not run, whatever the
/// accessor.
fn assert_every_accessor_refuses(world: &World, id: SessionId) {
    let ran = Cell::new(false);
    let refused = |name: &str, result: Result<(), CommandError>| {
        let err = result.expect_err(&format!("{name} served id {id}"));
        assert_eq!(err.code, "DATABASE_CLOSED", "{name} with id {id}");
        assert!(!ran.get(), "{name} ran its closure for id {id}");
    };
    let session = &world.session;
    refused(
        "read",
        session.read(id, |_| {
            ran.set(true);
            Ok(())
        }),
    );
    refused(
        "read_stamped",
        session
            .read_stamped(id, |_| {
                ran.set(true);
                Ok(())
            })
            .map(|_| ()),
    );
    refused(
        "write",
        session.write(id, |_| {
            ran.set(true);
            Ok(())
        }),
    );
    refused(
        "write_open",
        session.write_open(id, |_| {
            ran.set(true);
            Ok(())
        }),
    );
    refused(
        "write_housekeeping",
        session.write_housekeeping(id, |_| {
            ran.set(true);
            Ok(())
        }),
    );
    refused(
        "inspect",
        session.inspect(id, |_| {
            ran.set(true);
            Ok(())
        }),
    );
    refused(
        "inspect_mut",
        session.inspect_mut(id, |_| {
            ran.set(true);
            Ok(())
        }),
    );
    refused("hold", world.scoped(id).hold().map(|_| ()));
}

/// Calls every accessor with `id` and expects each to serve it.
fn assert_every_accessor_serves(world: &World, id: SessionId) {
    let session = &world.session;
    session.read(id, |_| Ok(())).unwrap();
    session.read_stamped(id, |_| Ok(())).unwrap();
    session.write(id, |_| Ok(())).unwrap();
    session.write_open(id, |_| Ok(())).unwrap();
    session.write_housekeeping(id, |_| Ok(())).unwrap();
    session.inspect(id, |_| Ok(())).unwrap();
    session.inspect_mut(id, |_| Ok(())).unwrap();
    drop(world.scoped(id).hold().unwrap());
}

// --- The ids that name no session ---------------------------------------------

#[test]
fn the_current_id_is_served_by_every_accessor() {
    let world = World::new();
    let id = world.create("Mine");
    assert_ne!(id, SessionId::NONE);

    assert_every_accessor_serves(&world, id);
}

#[test]
fn zero_and_a_never_issued_id_are_refused_by_every_accessor() {
    let world = World::new();
    let id = world.create("Mine");

    assert_every_accessor_refuses(&world, SessionId::NONE);
    assert_every_accessor_refuses(&world, never_issued());
    // The ones next to it, too.
    assert_every_accessor_refuses(&world, SessionId::from_request(id.get() + 1).unwrap());

    // And nothing was disturbed: the real one is still served.
    assert_every_accessor_serves(&world, id);
}

#[test]
fn every_id_is_refused_with_nothing_open() {
    let world = World::new();

    assert_every_accessor_refuses(&world, SessionId::NONE);
    assert_every_accessor_refuses(&world, never_issued());
}

// --- Sessions that have ended ----------------------------------------------------

#[test]
fn a_session_closed_is_refused() {
    let world = World::new();
    let ended = world.create("Mine");
    world.close(ended);

    assert_every_accessor_refuses(&world, ended);
}

#[test]
fn a_session_locked_is_refused() {
    let world = World::new();
    let ended = world.create("Mine");
    lifecycle::lock(&world.session, ended, &world.machine, CloseReason::LockedByUser, None)
        .unwrap();

    assert_every_accessor_refuses(&world, ended);
}

#[test]
fn a_session_locked_with_a_draft_is_refused() {
    let world = World::new();
    let ended = world.create("Mine");
    databases::lock_database(&world.session, ended, &world.machine, Some(draft())).unwrap();

    assert_every_accessor_refuses(&world, ended);
}

#[test]
fn the_same_file_reopened_is_a_new_session() {
    let world = World::new();
    let ended = world.create("Mine");
    world.close(ended);
    let current = world.open("Mine");

    assert_ne!(current, ended, "an id is never handed out twice");
    assert_every_accessor_refuses(&world, ended);
    assert_every_accessor_serves(&world, current);
}

#[test]
fn a_second_database_opened_over_the_first_is_a_new_session() {
    let world = World::new();
    let ended = world.create("Mine");
    // The switch closes "Mine" itself.
    lifecycle::create(&world.session, &world.machine, &world.path("Other"), &passphrase()).unwrap();
    let current = world.session.current_id();

    assert_ne!(current, ended);
    assert_every_accessor_refuses(&world, ended);
    assert_every_accessor_serves(&world, current);

    // Opening one that exists switches the same way.
    let third = world.open("Mine");
    assert_every_accessor_refuses(&world, current);
    assert_every_accessor_serves(&world, third);
}

#[test]
fn a_restore_of_the_open_database_is_a_new_session() {
    let world = World::new();
    let first = world.create("Mine");
    world.add_firearm(first, "A1").unwrap();
    world.close(first);
    let ended = world.open("Mine");
    let listed = backups_ops::list_backups(&world.session, &world.machine, Target::Open(ended))
        .unwrap()
        .backups;
    let backup = listed.first().expect("the close made a backup").path.clone();

    let status = backups_ops::restore_backup(
        &world.session,
        &world.machine,
        &backup,
        &passphrase(),
        Target::Open(ended),
    )
    .unwrap();

    let current = world.session.current_id();
    assert_ne!(current, ended);
    assert_eq!(status.session_id, current);
    assert_every_accessor_refuses(&world, ended);
    assert_every_accessor_serves(&world, current);

    // A restore that names the session that ended restores nothing.
    let late = backups_ops::restore_backup(
        &world.session,
        &world.machine,
        &backup,
        &passphrase(),
        Target::Open(ended),
    );
    assert_eq!(late.unwrap_err().code, "DATABASE_CLOSED");
    assert_eq!(world.session.current_id(), current, "the open session is left as it was");
}

#[test]
fn a_passphrase_change_keeps_the_session() {
    let world = World::new();
    let id = world.create("Mine");
    world.add_firearm(id, "A1").unwrap();
    let new = Passphrase::from_input("a different passphrase entirely".to_owned());
    let scratch = world.config.path().join("probes");

    backups_ops::change_passphrase(
        &world.session,
        id,
        &world.machine,
        &scratch,
        &passphrase(),
        &new,
    )
    .unwrap();

    // The same open, re-keyed: the id a web view holds is still its session
    // (research.md §1).
    assert_eq!(world.session.current_id(), id);
    assert_every_accessor_serves(&world, id);
    let status = databases::database_status(&world.session, &world.machine).unwrap();
    assert_eq!(status.session_id, id);

    // And a change that names another session changes nothing.
    let late = backups_ops::change_passphrase(
        &world.session,
        never_issued(),
        &world.machine,
        &scratch,
        &new,
        &passphrase(),
    );
    assert_eq!(late.unwrap_err().code, "DATABASE_CLOSED");
    assert_every_accessor_serves(&world, id);
}

// Unix only: Windows refuses to rename over a file SQLite has open, so
// nothing can replace an open database there.
#[cfg(unix)]
#[test]
fn a_session_taken_over_is_refused() {
    let world = World::new();
    let ended = world.create("Mine");
    world.add_firearm(ended, "A1").unwrap();
    // Another computer's copy of the file, renamed over the path.
    let theirs = world.dir.path().join(".sync-incoming");
    fs::copy(world.path("Mine"), &theirs).unwrap();
    fs::rename(&theirs, world.path("Mine")).unwrap();

    let taken = world.add_firearm(ended, "A2");
    assert_eq!(taken.unwrap_err().code, "DATABASE_TAKEN_OVER");

    assert!(!world.session.is_open());
    assert_every_accessor_refuses(&world, ended);
}

// --- Commands that take a database path (restore_backup, list_backups) -------

#[test]
fn a_path_names_a_database_and_no_path_names_the_open_session_by_header() {
    use tauri::http::{HeaderMap, HeaderValue};

    let none = HeaderMap::new();
    let mut named = HeaderMap::new();
    named.insert(SESSION_HEADER, HeaderValue::from_static("42"));

    // With a path, the chooser's case: no session, no header needed.
    assert_eq!(
        Target::from_request(Some("/x/Mine.hoplodex"), &none).unwrap(),
        Target::At("/x/Mine.hoplodex")
    );
    assert_eq!(
        Target::from_request(Some("/x/Mine.hoplodex"), &named).unwrap(),
        Target::At("/x/Mine.hoplodex")
    );
    // Without one, the open database: the header must name a session.
    let missing = Target::from_request(None, &none).unwrap_err();
    assert_eq!(missing.code, "DATABASE_CLOSED");
    assert_eq!(
        Target::from_request(None, &named).unwrap(),
        Target::Open(SessionId::from_request(42).unwrap())
    );
}

#[test]
fn listing_the_backups_of_the_open_database_needs_its_session_and_of_another_does_not() {
    let world = World::new();
    let first = world.create("Mine");
    world.add_firearm(first, "A1").unwrap();
    world.close(first);
    // Nothing open: a database that isn't open lists by its path.
    let found = backups_ops::list_backups(
        &world.session,
        &world.machine,
        Target::At(&world.path("Mine").to_string_lossy()),
    )
    .unwrap();
    assert!(!found.backups.is_empty());

    let current = world.open("Mine");
    let open =
        backups_ops::list_backups(&world.session, &world.machine, Target::Open(current)).unwrap();
    assert_eq!(open.backups, found.backups);
    for id in [first, SessionId::NONE, never_issued()] {
        let refused = backups_ops::list_backups(&world.session, &world.machine, Target::Open(id));
        assert_eq!(refused.unwrap_err().code, "DATABASE_CLOSED", "id {id}");
    }
    // A path still lists while another database is open.
    let by_path = backups_ops::list_backups(
        &world.session,
        &world.machine,
        Target::At(&world.path("Mine").to_string_lossy()),
    )
    .unwrap();
    assert_eq!(by_path.backups, found.backups);
}

// --- A late close or lock leaves the next session open -----------------------

/// Checks that a close or lock of the session `ended`, which another session
/// has replaced, changes nothing about the one open now.
fn assert_a_late_close_and_lock_leave_the_next_session(world: &World, ended: SessionId) {
    let next = world.session.current_id();
    assert_ne!(next, ended);

    let close =
        databases::close_database(&world.session, ended, &world.machine, CloseReason::Closed);
    assert_eq!(close.unwrap_err().code, "DATABASE_CLOSED");

    let lock_without = databases::lock_database(&world.session, ended, &world.machine, None);
    assert_eq!(lock_without.unwrap_err().code, "DATABASE_CLOSED");

    let lock_with = databases::lock_database(&world.session, ended, &world.machine, Some(draft()));
    assert_eq!(lock_with.unwrap_err().code, "DATABASE_CLOSED");

    let quit_lock =
        lifecycle::lock(&world.session, ended, &world.machine, CloseReason::Idle, Some(draft()));
    assert_eq!(quit_lock.unwrap_err().code, "DATABASE_CLOSED");

    assert!(world.session.is_open(), "the next session is still open");
    assert_eq!(world.session.current_id(), next);
    assert_eq!(world.count_pending(next), 0, "no draft was written to it");
    assert_every_accessor_serves(world, next);
    let status = databases::database_status(&world.session, &world.machine).unwrap();
    assert!(status.pending_changes.is_none());
    for event in ["session:closing", "session:closed"] {
        assert!(
            world.events.payloads(event).iter().all(|p| p["sessionId"] != json!(next.get())),
            "{event} announced the end of the next session"
        );
    }
}

#[test]
fn a_late_close_or_lock_of_the_same_file_reopened_leaves_it_open() {
    let world = World::new();
    let ended = world.create("Mine");
    world.close(ended);
    world.open("Mine");
    world.events.take();

    assert_a_late_close_and_lock_leave_the_next_session(&world, ended);
}

#[test]
fn a_late_close_or_lock_after_a_switch_leaves_the_other_database_open() {
    let world = World::new();
    let ended = world.create("Mine");
    lifecycle::create(&world.session, &world.machine, &world.path("Other"), &passphrase()).unwrap();
    world.events.take();

    assert_a_late_close_and_lock_leave_the_next_session(&world, ended);
}

#[test]
fn a_late_close_with_nothing_open_is_refused() {
    let world = World::new();
    let ended = world.create("Mine");
    world.close(ended);

    let close =
        databases::close_database(&world.session, ended, &world.machine, CloseReason::Closed);
    assert_eq!(close.unwrap_err().code, "DATABASE_CLOSED");
    let lock = databases::lock_database(&world.session, ended, &world.machine, Some(draft()));
    assert_eq!(lock.unwrap_err().code, "DATABASE_CLOSED");
}

#[test]
fn a_close_or_lock_of_the_current_session_still_works() {
    let world = World::new();
    let id = world.create("Mine");
    databases::lock_database(&world.session, id, &world.machine, Some(draft())).unwrap();
    assert!(!world.session.is_open());

    let id = world.open("Mine");
    databases::close_database(&world.session, id, &world.machine, CloseReason::Closed).unwrap();
    assert!(!world.session.is_open());
}

// --- Which commands name their session -----------------------------------------

/// The `#[tauri::command]` functions in `src/commands/*.rs`, each with its
/// parameter list.
fn commands_in_source() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands");
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap();
        // The attribute alone on its line (not a doc comment's mention of it).
        for (at, _) in text.match_indices("\n#[tauri::command]\n") {
            let at = at + 1;
            let rest = &text[at + "#[tauri::command]".len()..];
            let fn_at = rest.find("fn ").expect("a command is followed by its fn");
            let after = &rest[fn_at + "fn ".len()..];
            let name_end = after.find('(').unwrap();
            let name = after[..name_end].trim().to_owned();
            let signature_end = after.find(") ->").expect("a command returns a Result");
            found.push((name, after[name_end..signature_end].to_owned()));
        }
    }
    assert!(found.len() > 50, "found only {} commands: the scan is broken", found.len());
    found
}

/// Commands that take no session at all: they read nothing of an open
/// database.
const SESSION_FREE_COMMANDS: &[&str] = &["list_document_types"];

#[test]
fn unscoped_commands_are_all_commands() {
    for name in UNSCOPED_COMMANDS {
        assert!(COMMANDS.contains(name), "{name} is in UNSCOPED_COMMANDS but not in COMMANDS");
    }
}

#[test]
fn a_command_takes_the_state_session_only_if_unscoped() {
    let mut problems = Vec::new();
    for (name, parameters) in commands_in_source() {
        let unscoped = UNSCOPED_COMMANDS.contains(&name.as_str());
        let takes_state = parameters.contains("State<'_, Session>");
        let takes_scoped = parameters.contains("ScopedSession");
        if takes_state && !unscoped {
            problems.push(format!("{name} takes State<Session> but isn't in UNSCOPED_COMMANDS"));
        }
        if unscoped && takes_scoped {
            problems.push(format!("{name} is in UNSCOPED_COMMANDS but takes ScopedSession"));
        }
        if !unscoped && !takes_scoped && !SESSION_FREE_COMMANDS.contains(&name.as_str()) {
            problems.push(format!("{name} is scoped but takes no ScopedSession"));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
