//! Pending changes (FR-039, research.md §16): a form's unsaved input,
//! staged in memory, kept inside the database by a lock or an OS shutdown,
//! and offered at the next open. Real SQLCipher files in temp directories.

mod support;

#[cfg(unix)]
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::accessories::ops as accessories;
use hoplodex_lib::commands::databases::ops as databases;
use hoplodex_lib::commands::firearms::ops as firearms;
use hoplodex_lib::commands::insurance::ops as insurance;
use hoplodex_lib::models::database::{
    BackupOutcome, CloseReason, Draft, DraftKind, DraftMode, PendingAction,
};
use hoplodex_lib::services::backups;
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::session::{Session, lifecycle};
#[cfg(unix)]
use serde_json::Value;
use serde_json::json;
use support::{TestEvents, passphrase, peek, test_session};
use tempfile::TempDir;

struct World {
    dir: TempDir,
    _config: TempDir,
    machine: MachineSettings,
    session: Session,
    // Read only by the Unix-only tests.
    #[cfg_attr(not(unix), allow(dead_code))]
    events: Arc<TestEvents>,
}

impl World {
    fn new() -> Self {
        let config = TempDir::new().unwrap();
        let (session, events) = test_session(&config.path().join("opened-documents"));
        Self {
            dir: TempDir::new().unwrap(),
            machine: MachineSettings::load(config.path()).unwrap(),
            _config: config,
            session,
            events,
        }
    }

    fn path(&self) -> PathBuf {
        self.dir.path().join("Mine.hoplodex")
    }

    fn path_text(&self) -> String {
        self.path().to_string_lossy().into_owned()
    }

    /// Creates "Mine" and leaves it open.
    fn create(&self) {
        lifecycle::create(&self.session, &self.machine, &self.path(), &passphrase()).unwrap();
    }

    fn open(&self) {
        lifecycle::open(&self.session, &self.machine, &self.path(), &passphrase(), false).unwrap();
    }

    fn add_firearm(&self, serial: &str) -> Result<i64, CommandError> {
        self.session.write(|conn| {
            firearms::create_firearm(conn, &support::firearm("Glock", "19", serial), false, None)
                .map(|created| created.id)
        })
    }

    /// Rows in `pending_changes`, looked at through the open connection.
    fn pending_rows(&self) -> i64 {
        self.session
            .inspect(|open| {
                open.conn
                    .query_row("SELECT count(*) FROM pending_changes", [], |row| row.get(0))
                    .map_err(CommandError::from_db)
            })
            .unwrap()
    }

    /// The closed file's pending changes and backup record, and whether its
    /// open marker is set.
    fn closed_file(&self) -> (i64, bool, bool) {
        let conn = peek(&self.path());
        conn.query_row(
            "SELECT (SELECT count(*) FROM pending_changes), changes_waiting,
                    open_machine_id IS NOT NULL
             FROM app_state",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap()
    }

    #[cfg(unix)]
    fn notices(&self) -> Vec<Value> {
        self.events.payloads("notice")
    }
}

fn draft_for(kind: DraftKind, mode: DraftMode, target_id: Option<i64>) -> Draft {
    Draft {
        form_version: 1,
        kind,
        mode,
        target_id,
        label: "Glock 19 (edit)".into(),
        values: json!({ "make": "Glock", "model": "19", "notes": "half typed, with “quotes” é" }),
    }
}

fn edit_draft(id: i64) -> Draft {
    draft_for(DraftKind::Firearm, DraftMode::Edit, Some(id))
}

fn validation_field(err: CommandError) -> String {
    assert_eq!(err.code, "VALIDATION_ERROR");
    err.field_errors.unwrap().into_keys().next().unwrap()
}

// --- Staging -----------------------------------------------------------------

#[test]
fn staging_keeps_the_draft_in_memory_only() {
    let world = World::new();
    world.create();
    let id = world.add_firearm("A1").unwrap();

    databases::stage_pending_changes(&world.session, Some(edit_draft(id))).unwrap();

    assert_eq!(world.pending_rows(), 0, "nothing is written until a lock");
    let staged = world.session.inspect(|open| Ok(open.staged_draft.clone())).unwrap();
    assert_eq!(staged, Some(edit_draft(id)));
    databases::stage_pending_changes(&world.session, None).unwrap();
    assert_eq!(world.session.inspect(|open| Ok(open.staged_draft.clone())).unwrap(), None);
}

#[test]
fn a_draft_over_one_mebibyte_is_refused() {
    let world = World::new();
    world.create();
    let mut draft = draft_for(DraftKind::Firearm, DraftMode::Add, None);
    draft.values = json!({ "notes": "x".repeat(1 << 20) });

    let refused = databases::stage_pending_changes(&world.session, Some(draft)).unwrap_err();

    assert_eq!(validation_field(refused), "values");
}

#[test]
fn a_draft_whose_kind_and_mode_dont_go_together_is_refused() {
    let world = World::new();
    world.create();
    for (kind, mode, target) in [
        (DraftKind::Policy, DraftMode::Coverage, Some(1)),
        (DraftKind::Policy, DraftMode::Dispose, Some(1)),
    ] {
        let refused =
            databases::stage_pending_changes(&world.session, Some(draft_for(kind, mode, target)))
                .unwrap_err();
        assert_eq!(validation_field(refused), "mode");
    }
    let no_target = draft_for(DraftKind::Firearm, DraftMode::Edit, None);
    assert_eq!(
        validation_field(
            databases::stage_pending_changes(&world.session, Some(no_target)).unwrap_err()
        ),
        "targetId"
    );
    let added_with_target = draft_for(DraftKind::Firearm, DraftMode::Add, Some(3));
    assert_eq!(
        validation_field(
            databases::stage_pending_changes(&world.session, Some(added_with_target)).unwrap_err()
        ),
        "targetId"
    );
}

// --- Keeping them at a lock, a sleep and a shutdown ------------------------

#[test]
fn a_lock_keeps_the_draft_and_leaves_the_backup_record_alone() {
    let world = World::new();
    world.create();
    let id = world.add_firearm("A1").unwrap();
    // Backups off, so the changes stay waiting and the record shows it.
    world
        .session
        .write(|conn| {
            conn.execute("UPDATE collection_settings SET backups_enabled = 0", [])
                .map_err(CommandError::from_db)
        })
        .unwrap();

    let outcome =
        databases::lock_database(&world.session, &world.machine, Some(edit_draft(id))).unwrap();

    assert_eq!(outcome.backup, BackupOutcome::Off);
    assert!(!world.session.is_open());
    let (pending, changes_waiting, marked) = world.closed_file();
    assert_eq!(pending, 1);
    assert!(changes_waiting, "pending changes are not a change to back up");
    assert!(!marked, "a lock clears the open marker");
}

#[test]
fn the_idle_lock_keeps_the_staged_draft() {
    let world = World::new();
    world.create();
    let id = world.add_firearm("A1").unwrap();
    databases::stage_pending_changes(&world.session, Some(edit_draft(id))).unwrap();

    lifecycle::lock_with_staged(&world.session, &world.machine, CloseReason::Idle).unwrap();

    assert_eq!(world.closed_file().0, 1);
}

#[test]
fn a_sleep_keeps_the_draft_and_clears_the_marker_in_the_same_write() {
    for reason in [CloseReason::Sleep, CloseReason::Shutdown] {
        let world = World::new();
        world.create();
        let id = world.add_firearm("A1").unwrap();
        databases::stage_pending_changes(&world.session, Some(edit_draft(id))).unwrap();

        lifecycle::close_immediate(&world.session, &world.machine, reason);

        assert!(!world.session.is_open());
        let (pending, changes_waiting, marked) = world.closed_file();
        assert_eq!(pending, 1, "{reason:?}");
        assert!(changes_waiting, "{reason:?} makes no backup");
        assert!(!marked, "{reason:?}");
    }
}

#[test]
fn a_sleep_with_nothing_staged_still_clears_the_marker() {
    let world = World::new();
    world.create();

    lifecycle::close_immediate(&world.session, &world.machine, CloseReason::Sleep);

    let (pending, _, marked) = world.closed_file();
    assert_eq!(pending, 0);
    assert!(!marked);
}

// --- Offering them at the next open ----------------------------------------

/// "Mine" with a firearm, locked with an edit of it pending. Returns the
/// firearm's id.
fn locked_while_editing(world: &World) -> i64 {
    world.create();
    let id = world.add_firearm("A1").unwrap();
    databases::lock_database(&world.session, &world.machine, Some(edit_draft(id))).unwrap();
    id
}

#[test]
fn the_next_open_reports_the_pending_changes() {
    let world = World::new();
    let id = locked_while_editing(&world);

    let status = databases::open_database(
        &world.session,
        &world.machine,
        &world.path_text(),
        databases::Unlock::typed(&passphrase()),
        false,
    )
    .unwrap();

    let pending = status.pending_changes.expect("reported");
    assert_eq!(pending.form_version, 1);
    assert_eq!(pending.kind, DraftKind::Firearm);
    assert_eq!(pending.mode, DraftMode::Edit);
    assert_eq!(pending.target_id, Some(id));
    assert_eq!(pending.label, "Glock 19 (edit)");
    assert!(pending.resumable);
    assert!(!pending.saved_at.is_empty());
}

#[test]
fn pending_changes_for_a_record_deleted_elsewhere_can_only_be_discarded() {
    let world = World::new();
    let id = locked_while_editing(&world);
    // Deleted on another computer while this one was locked.
    {
        let conn = hoplodex_lib::db::open_database(
            &world.path(),
            &passphrase(),
            &support::other_machine(),
            false,
        )
        .unwrap();
        conn.execute("DELETE FROM pending_changes", []).unwrap();
        firearms::delete_firearm(&conn, id, true).unwrap();
        conn.execute(
            "INSERT INTO pending_changes (id, kind, mode, target_id, label, form_version,
                                          values_json, saved_at)
             VALUES (1, 'firearm', 'edit', ?1, 'Glock 19 (edit)', 1, '{}', '2026-09-25T12:00:00Z')",
            [id],
        )
        .unwrap();
        conn.execute(
            "UPDATE app_state SET open_machine_id = NULL, open_machine_name = NULL,
                                  open_since = NULL",
            [],
        )
        .unwrap();
    }

    world.open();

    let status = databases::database_status(&world.session, &world.machine).unwrap();
    assert!(!status.pending_changes.unwrap().resumable);
}

#[test]
fn a_policy_draft_is_resumable_while_the_policy_exists() {
    let world = World::new();
    world.create();
    let policy = world
        .session
        .write(|conn| {
            insurance::create_policy(
                conn,
                &support::policy("Home", "2026-01-01", "2026-12-31", None),
            )
        })
        .unwrap();
    let draft = Draft {
        label: "Home (edit)".into(),
        ..draft_for(DraftKind::Policy, DraftMode::Edit, Some(policy.id))
    };
    databases::lock_database(&world.session, &world.machine, Some(draft)).unwrap();

    world.open();

    let pending = databases::database_status(&world.session, &world.machine)
        .unwrap()
        .pending_changes
        .unwrap();
    assert_eq!(pending.kind, DraftKind::Policy);
    assert!(pending.resumable);
}

#[test]
fn collection_commands_wait_until_the_pending_changes_are_resolved() {
    let world = World::new();
    locked_while_editing(&world);
    world.open();

    let refused = world.add_firearm("B2").unwrap_err();
    assert_eq!(refused.code, "PENDING_CHANGES_UNRESOLVED");
    let refused =
        world.session.read(|conn| firearms::list_firearms(conn, &Default::default())).unwrap_err();
    assert_eq!(refused.code, "PENDING_CHANGES_UNRESOLVED");
    // The session's own commands still answer.
    databases::database_status(&world.session, &world.machine).unwrap();

    databases::resolve_pending_changes(&world.session, PendingAction::Discard).unwrap();

    world.add_firearm("B2").unwrap();
}

#[test]
fn resuming_returns_the_exact_draft_and_removes_it() {
    let world = World::new();
    let id = locked_while_editing(&world);
    world.open();

    let resolved =
        databases::resolve_pending_changes(&world.session, PendingAction::Resume).unwrap();

    assert_eq!(resolved.draft, Some(edit_draft(id)));
    assert_eq!(world.pending_rows(), 0);
    assert!(
        databases::database_status(&world.session, &world.machine)
            .unwrap()
            .pending_changes
            .is_none()
    );
}

#[test]
fn discarding_removes_them() {
    let world = World::new();
    locked_while_editing(&world);
    world.open();

    let resolved =
        databases::resolve_pending_changes(&world.session, PendingAction::Discard).unwrap();

    assert_eq!(resolved.draft, None);
    assert_eq!(world.pending_rows(), 0);
}

#[test]
fn a_backup_made_at_a_lock_carries_no_pending_changes() {
    let world = World::new();
    world.create();
    let id = world.add_firearm("A1").unwrap();

    let outcome =
        databases::lock_database(&world.session, &world.machine, Some(edit_draft(id))).unwrap();

    assert_eq!(outcome.backup, BackupOutcome::Made);
    assert_eq!(world.closed_file().0, 1, "the database keeps them");
    let folder = world.dir.path().join("HoploDex backups");
    let id = peek(&world.path())
        .query_row("SELECT database_id FROM app_state", [], |row| row.get::<_, String>(0))
        .unwrap();
    let listed = backups::list(&folder, &id).unwrap();
    assert_eq!(listed.len(), 1);
    let in_backup: i64 = peek(std::path::Path::new(&listed[0].path))
        .query_row("SELECT count(*) FROM pending_changes", [], |row| row.get(0))
        .unwrap();
    assert_eq!(in_backup, 0);
}

// --- When they can't be kept -------------------------------------------------

#[cfg(unix)]
#[test]
fn pending_changes_that_cant_be_written_still_lock_and_are_reported_lost() {
    use std::os::unix::fs::PermissionsExt;

    let world = World::new();
    let folder = world.dir.path().join("share");
    fs::create_dir(&folder).unwrap();
    let path = folder.join("Mine.hoplodex");
    lifecycle::create(&world.session, &world.machine, &path, &passphrase()).unwrap();
    let id = world
        .session
        .write(|conn| {
            firearms::create_firearm(conn, &support::firearm("Glock", "19", "A1"), false, None)
                .map(|created| created.id)
        })
        .unwrap();

    // The drive or share holding it can't be reached any more.
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o000)).unwrap();
    let locked = databases::lock_database(&world.session, &world.machine, Some(edit_draft(id)));
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o700)).unwrap();

    locked.unwrap();
    assert!(!world.session.is_open(), "the lock goes ahead");
    assert!(world.notices().contains(
        &json!({ "kind": "pendingChangesLost", "databasePath": path.to_string_lossy() })
    ));
    let pending: i64 = peek(&path)
        .query_row("SELECT count(*) FROM pending_changes", [], |row| row.get(0))
        .unwrap();
    assert_eq!(pending, 0);
}

// Unix only: Windows refuses to rename over a file SQLite has open, so
// nothing can replace an open database there.
#[cfg(unix)]
#[test]
fn a_take_over_found_while_finishing_on_waking_loses_the_draft_and_says_so() {
    let world = World::new();
    world.create();
    let id = world.add_firearm("A1").unwrap();
    databases::stage_pending_changes(&world.session, Some(edit_draft(id))).unwrap();
    // The computer slept after the first step of the lock.
    assert!(lifecycle::begin_immediate(&world.session, CloseReason::Sleep));
    // Meanwhile another computer took the file over.
    let ours = world.dir.path().join("ours-link.hoplodex");
    fs::hard_link(world.path(), &ours).unwrap();
    let theirs = world.dir.path().join(".sync-incoming");
    fs::copy(world.path(), &theirs).unwrap();
    fs::rename(&theirs, world.path()).unwrap();

    lifecycle::finish_on_wake(&world.session, &world.machine);

    assert!(!world.session.is_open());
    assert!(
        world
            .notices()
            .contains(&json!({ "kind": "pendingChangesLost", "databasePath": world.path_text() }))
    );
    let pending: i64 = peek(&ours)
        .query_row("SELECT count(*) FROM pending_changes", [], |row| row.get(0))
        .unwrap();
    assert_eq!(pending, 0, "nothing is written to a file taken over");
}

// --- Accessory drafts (specs/006-accessory-links FR-027, research.md §19) ------
//
// `Draft.kind` gains "accessory" with the modes a firearm has. Accessory
// inputs are built from the IPC shape (`AccessoryInput`'s camelCase JSON), so
// these tests do not depend on how the struct is spelled.

const ACCESSORY_MODES: [DraftMode; 5] =
    [DraftMode::Add, DraftMode::Edit, DraftMode::Dispose, DraftMode::Restore, DraftMode::Coverage];

impl World {
    fn add_accessory(&self) -> Result<i64, CommandError> {
        let input = json!({
            "accessoryKindId": 1,
            "make": "Leupold",
            "model": "VX-5HD 3-15x44",
            "status": "active",
        });
        self.session.write(|conn| {
            accessories::create_accessory(
                conn,
                &serde_json::from_value(input.clone()).unwrap(),
                None,
            )
            .map(|created| created.id)
        })
    }
}

/// An accessory draft labelled with the accessory's name (FR-005, FR-027).
fn accessory_draft(mode: DraftMode, target_id: Option<i64>) -> Draft {
    Draft {
        label: format!("Leupold VX-5HD 3-15x44 · Optic — {}", mode.as_str()),
        values: json!({ "make": "Leupold", "notes": "half typed, with “quotes” é" }),
        ..draft_for(DraftKind::Accessory, mode, target_id)
    }
}

fn target_for(mode: DraftMode, id: i64) -> Option<i64> {
    (mode != DraftMode::Add).then_some(id)
}

#[test]
fn an_accessory_draft_is_accepted_in_every_mode_a_firearm_has() {
    let world = World::new();
    world.create();
    let id = world.add_accessory().unwrap();

    for mode in ACCESSORY_MODES {
        let draft = accessory_draft(mode, target_for(mode, id));
        databases::stage_pending_changes(&world.session, Some(draft.clone()))
            .unwrap_or_else(|err| panic!("{mode:?}: {err:?}"));
        assert_eq!(
            world.session.inspect(|open| Ok(open.staged_draft.clone())).unwrap(),
            Some(draft),
            "{mode:?}"
        );
    }
}

#[test]
fn an_accessory_drafts_target_follows_the_same_rules_as_a_firearms() {
    let world = World::new();
    world.create();
    let id = world.add_accessory().unwrap();

    for mode in [DraftMode::Edit, DraftMode::Dispose, DraftMode::Restore, DraftMode::Coverage] {
        let no_target = accessory_draft(mode, None);
        let refused =
            databases::stage_pending_changes(&world.session, Some(no_target)).unwrap_err();
        assert_eq!(validation_field(refused), "targetId", "{mode:?} needs a target");
    }
    let added_with_target = accessory_draft(DraftMode::Add, Some(id));
    let refused =
        databases::stage_pending_changes(&world.session, Some(added_with_target)).unwrap_err();
    assert_eq!(validation_field(refused), "targetId", "an added accessory has no target yet");
}

#[test]
fn a_policy_draft_in_coverage_mode_is_still_refused() {
    let world = World::new();
    world.create();
    world.add_accessory().unwrap();

    let refused = databases::stage_pending_changes(
        &world.session,
        Some(draft_for(DraftKind::Policy, DraftMode::Coverage, Some(1))),
    )
    .unwrap_err();

    assert_eq!(validation_field(refused), "mode");
}

#[test]
fn an_accessory_draft_in_each_mode_is_kept_by_a_lock_and_resumed_with_its_label() {
    for mode in ACCESSORY_MODES {
        let world = World::new();
        world.create();
        let id = world.add_accessory().unwrap();
        let draft = accessory_draft(mode, target_for(mode, id));
        databases::lock_database(&world.session, &world.machine, Some(draft.clone())).unwrap();
        assert_eq!(world.closed_file().0, 1, "{mode:?} is kept in the database");

        let status = databases::open_database(
            &world.session,
            &world.machine,
            &world.path_text(),
            databases::Unlock::typed(&passphrase()),
            false,
        )
        .unwrap();

        let pending = status.pending_changes.unwrap_or_else(|| panic!("{mode:?} is reported"));
        assert_eq!(pending.kind, DraftKind::Accessory, "{mode:?}");
        assert_eq!(pending.mode, mode);
        assert_eq!(pending.target_id, target_for(mode, id));
        assert_eq!(pending.label, draft.label, "named by the accessory's label");
        assert!(pending.resumable, "{mode:?}");

        let resolved =
            databases::resolve_pending_changes(&world.session, PendingAction::Resume).unwrap();
        assert_eq!(resolved.draft, Some(draft), "{mode:?}: the exact draft comes back");
        assert_eq!(world.pending_rows(), 0, "{mode:?}");
    }
}

#[test]
fn an_accessory_draft_is_kept_by_a_sleep_as_well() {
    let world = World::new();
    world.create();
    let id = world.add_accessory().unwrap();
    databases::stage_pending_changes(
        &world.session,
        Some(accessory_draft(DraftMode::Edit, Some(id))),
    )
    .unwrap();

    lifecycle::close_immediate(&world.session, &world.machine, CloseReason::Sleep);

    let (pending, _, marked) = world.closed_file();
    assert_eq!(pending, 1);
    assert!(!marked);
}

#[test]
fn a_resumed_edit_whose_accessory_was_deleted_elsewhere_can_only_be_discarded() {
    let world = World::new();
    world.create();
    let id = world.add_accessory().unwrap();
    databases::lock_database(
        &world.session,
        &world.machine,
        Some(accessory_draft(DraftMode::Edit, Some(id))),
    )
    .unwrap();
    // Deleted on another computer while this one was locked.
    {
        let conn = hoplodex_lib::db::open_database(
            &world.path(),
            &passphrase(),
            &support::other_machine(),
            false,
        )
        .unwrap();
        conn.execute("DELETE FROM pending_changes", []).unwrap();
        accessories::delete_accessory(&conn, id, true).unwrap();
        conn.execute(
            "INSERT INTO pending_changes (id, kind, mode, target_id, label, form_version,
                                          values_json, saved_at)
             VALUES (1, 'accessory', 'edit', ?1, 'Leupold VX-5HD 3-15x44 · Optic — edit', 1, '{}',
                     '2026-09-25T12:00:00Z')",
            [id],
        )
        .unwrap();
        conn.execute(
            "UPDATE app_state SET open_machine_id = NULL, open_machine_name = NULL,
                                  open_since = NULL",
            [],
        )
        .unwrap();
    }

    world.open();

    let pending =
        databases::database_status(&world.session, &world.machine).unwrap().pending_changes;
    let pending = pending.expect("the draft is still offered");
    assert_eq!(pending.kind, DraftKind::Accessory);
    assert!(!pending.resumable);
}

/// A firearm and an accessory can have the same numeric id: an accessory
/// draft is checked against `accessories`, never `firearms`.
#[test]
fn an_accessory_draft_is_not_resumable_just_because_a_firearm_has_its_target_id() {
    let world = World::new();
    world.create();
    let firearm = world.add_firearm("A1").unwrap();
    databases::lock_database(
        &world.session,
        &world.machine,
        Some(accessory_draft(DraftMode::Edit, Some(firearm))),
    )
    .unwrap();

    world.open();

    let pending = databases::database_status(&world.session, &world.machine)
        .unwrap()
        .pending_changes
        .unwrap();
    assert_eq!(pending.kind, DraftKind::Accessory);
    assert!(!pending.resumable, "there is no accessory {firearm}, only a firearm");
}

#[test]
fn collection_commands_wait_for_an_accessory_draft_too() {
    let world = World::new();
    world.create();
    let id = world.add_accessory().unwrap();
    databases::lock_database(
        &world.session,
        &world.machine,
        Some(accessory_draft(DraftMode::Edit, Some(id))),
    )
    .unwrap();
    world.open();

    let refused = world.add_accessory().unwrap_err();
    assert_eq!(refused.code, "PENDING_CHANGES_UNRESOLVED");

    databases::resolve_pending_changes(&world.session, PendingAction::Discard).unwrap();
    world.add_accessory().unwrap();
}
