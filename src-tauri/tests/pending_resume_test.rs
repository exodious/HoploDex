//! Pending changes until their form opens (008 FR-003, FR-017; research.md
//! §6; data-model.md "PendingState"; contracts/tauri-commands.md
//! `resolve_pending_changes`): the four actions in each state, what each
//! state lets through, and a row that stays until the form has opened.
//! Real SQLCipher files in temp directories.

use crate::support;

use std::path::PathBuf;

use hoplodex_lib::APP_VERSION;
use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::databases::ops as databases;
use hoplodex_lib::commands::firearms::ops as firearms;
use hoplodex_lib::models::database::{
    CloseReason, Draft, DraftKind, DraftMode, PendingAction, PendingResolved,
};
use hoplodex_lib::services::backups;
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::session::fingerprint::FileFingerprint;
use hoplodex_lib::session::{Session, lifecycle};
use serde_json::json;
use support::{passphrase, peek, test_session};
use tempfile::TempDir;

struct World {
    dir: TempDir,
    _config: TempDir,
    machine: MachineSettings,
    session: Session,
}

/// Where a database's pending changes stand, as the data model names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    None,
    Unresolved,
    Resuming,
}

const ACTIONS: [PendingAction; 4] = [
    PendingAction::Resume,
    PendingAction::Opened,
    PendingAction::NotOpened,
    PendingAction::Discard,
];

impl World {
    fn new() -> Self {
        let config = TempDir::new().unwrap();
        let (session, _events) = test_session(&config.path().join("opened-documents"));
        Self {
            dir: TempDir::new().unwrap(),
            machine: MachineSettings::load(config.path()).unwrap(),
            _config: config,
            session,
        }
    }

    fn path(&self) -> PathBuf {
        self.dir.path().join("Mine.hoplodex")
    }

    fn path_text(&self) -> String {
        self.path().to_string_lossy().into_owned()
    }

    fn id(&self) -> hoplodex_lib::session::SessionId {
        self.session.current_id()
    }

    /// Creates "Mine" with a firearm, and leaves it open with nothing
    /// pending. Returns the firearm's id.
    fn create(&self) -> i64 {
        lifecycle::create(&self.session, &self.machine, &self.path(), &passphrase()).unwrap();
        self.session
            .write(self.id(), |conn| {
                firearms::create_firearm(conn, &support::firearm("Glock", "19", "R1"), false, None)
                    .map(|created| created.id)
            })
            .unwrap()
    }

    /// Opens "Mine" (a switch from itself if it is open), as the chooser does.
    fn open(&self) {
        databases::open_database(
            &self.session,
            &self.machine,
            &self.path_text(),
            databases::Unlock::typed(&passphrase()),
            false,
        )
        .unwrap();
    }

    /// Creates "Mine", locks it while the firearm's edit is pending, and
    /// opens it again: `Unresolved`. Returns the firearm's id.
    fn unresolved(&self) -> i64 {
        let firearm = self.create();
        databases::lock_database(
            &self.session,
            self.id(),
            &self.machine,
            Some(edit_draft(firearm)),
        )
        .unwrap();
        self.open();
        firearm
    }

    fn resolve(&self, action: PendingAction) -> Result<PendingResolved, CommandError> {
        databases::resolve_pending_changes(&self.session, self.id(), action)
    }

    /// A world in `state`. Returns the firearm's id.
    fn reach(&self, state: State) -> i64 {
        match state {
            State::None => self.create(),
            State::Unresolved => self.unresolved(),
            State::Resuming => {
                let firearm = self.unresolved();
                self.resolve(PendingAction::Resume).unwrap();
                firearm
            }
        }
    }

    /// What the collection does now: `Ok` when it is served, otherwise the
    /// refusal's code.
    fn collection(&self) -> Result<(), String> {
        self.session
            .read(self.id(), |conn| firearms::list_firearms(conn, &Default::default()).map(|_| ()))
            .map_err(|err| err.code)
    }

    /// The state the collection's behaviour shows, with the rows alongside.
    fn observed(&self) -> (Result<(), String>, i64) {
        (self.collection(), self.pending_rows())
    }

    fn pending_rows(&self) -> i64 {
        self.session
            .inspect(self.id(), |open| {
                open.conn
                    .query_row("SELECT count(*) FROM pending_changes", [], |row| row.get(0))
                    .map_err(CommandError::from_db)
            })
            .unwrap()
    }

    fn staged(&self) -> Option<Draft> {
        self.session.inspect(self.id(), |open| Ok(open.staged_draft.clone())).unwrap()
    }

    /// Writes `version` as the recorded one, bypassing every rule, and
    /// refreshes the fingerprint as a write of the session's own would.
    fn seed_saved_version(&self, version: &str) {
        self.session
            .inspect_mut(self.id(), |open| {
                open.conn
                    .execute("UPDATE app_state SET last_saved_version = ?1", [version])
                    .map_err(CommandError::from_db)?;
                open.fingerprint = FileFingerprint::capture(&open.path).unwrap();
                Ok(())
            })
            .unwrap();
    }

    /// The recorded version, in the open database.
    fn saved_version(&self) -> Option<String> {
        self.session
            .inspect(self.id(), |open| {
                open.conn
                    .query_row("SELECT last_saved_version FROM app_state", [], |row| row.get(0))
                    .map_err(CommandError::from_db)
            })
            .unwrap()
    }

    /// The database's id, from the open session.
    fn database_id(&self) -> String {
        self.session.inspect(self.id(), |open| Ok(open.database_id.clone())).unwrap()
    }

    /// Rows in the closed file's `pending_changes`.
    fn closed_rows(&self) -> i64 {
        peek(&self.path())
            .query_row("SELECT count(*) FROM pending_changes", [], |row| row.get(0))
            .unwrap()
    }
}

fn edit_draft(id: i64) -> Draft {
    Draft {
        form_version: 1,
        kind: DraftKind::Firearm,
        mode: DraftMode::Edit,
        target_id: Some(id),
        label: "Glock 19 (edit)".into(),
        values: json!({ "make": "Glock", "model": "19", "notes": "half typed, with “quotes” é" }),
    }
}

/// What each action does in each state: the state it leads to, or `None`
/// for a `VALIDATION_ERROR` (data-model.md "PendingState").
fn expected(state: State, action: PendingAction) -> Option<State> {
    match (state, action) {
        (State::Unresolved, PendingAction::Resume) => Some(State::Resuming),
        (State::Unresolved, PendingAction::Discard) => Some(State::None),
        (State::Resuming, PendingAction::Opened) => Some(State::None),
        (State::Resuming, PendingAction::NotOpened) => Some(State::Unresolved),
        _ => None,
    }
}

/// What the collection and the row look like in `state`.
fn shows(state: State) -> (Result<(), String>, i64) {
    match state {
        State::None => (Ok(()), 0),
        State::Unresolved => (Err("PENDING_CHANGES_UNRESOLVED".into()), 1),
        State::Resuming => (Ok(()), 1),
    }
}

// --- The transitions ----------------------------------------------------------

#[test]
fn each_action_in_each_state_moves_as_the_data_model_says_or_changes_nothing() {
    for state in [State::None, State::Unresolved, State::Resuming] {
        for action in ACTIONS {
            let world = World::new();
            world.reach(state);
            assert_eq!(world.observed(), shows(state), "{state:?} to begin with");
            let staged_before = world.staged();

            let result = world.resolve(action);

            match expected(state, action) {
                Some(next) => {
                    result.unwrap_or_else(|e| panic!("{action:?} in {state:?}: {e:?}"));
                    assert_eq!(world.observed(), shows(next), "{action:?} in {state:?}");
                }
                None => {
                    let err = result.expect_err(&format!("{action:?} in {state:?} was accepted"));
                    assert_eq!(err.code, "VALIDATION_ERROR", "{action:?} in {state:?}");
                    assert_eq!(world.observed(), shows(state), "{action:?} in {state:?} changed");
                    assert_eq!(world.staged(), staged_before, "{action:?} in {state:?} staged");
                }
            }
        }
    }
}

#[test]
fn resume_returns_the_exact_draft_keeps_the_row_and_serves_the_collection() {
    let world = World::new();
    let firearm = world.unresolved();

    let resolved = world.resolve(PendingAction::Resume).unwrap();

    assert_eq!(resolved.draft, Some(edit_draft(firearm)));
    assert_eq!(resolved.pending, None);
    assert_eq!(world.pending_rows(), 1, "the row stays until the form has opened");
    assert_eq!(world.staged(), None, "nothing is staged yet");
    // The record can load: reads and writes are served.
    world.collection().unwrap();
    world
        .session
        .write(world.id(), |conn| {
            firearms::create_firearm(conn, &support::firearm("Colt", "Python", "R2"), false, None)
        })
        .unwrap();
}

#[test]
fn opened_deletes_the_row_and_stages_the_draft() {
    let world = World::new();
    let firearm = world.unresolved();
    let resumed = world.resolve(PendingAction::Resume).unwrap().draft.unwrap();

    let resolved = world.resolve(PendingAction::Opened).unwrap();

    assert_eq!(resolved, PendingResolved { draft: None, pending: None });
    assert_eq!(world.pending_rows(), 0);
    assert_eq!(world.staged(), Some(resumed));
    assert_eq!(world.staged(), Some(edit_draft(firearm)));
    world.collection().unwrap();
    assert!(
        databases::database_status(&world.session, &world.machine)
            .unwrap()
            .pending_changes
            .is_none()
    );
}

#[test]
fn not_opened_returns_the_summary_for_the_dialog_and_blocks_again() {
    let world = World::new();
    let firearm = world.unresolved();
    world.resolve(PendingAction::Resume).unwrap();

    let resolved = world.resolve(PendingAction::NotOpened).unwrap();

    assert_eq!(resolved.draft, None);
    let pending = resolved.pending.expect("the dialog needs the summary");
    assert_eq!(pending.label, "Glock 19 (edit)");
    assert_eq!(pending.kind, DraftKind::Firearm);
    assert_eq!(pending.mode, DraftMode::Edit);
    assert_eq!(pending.target_id, Some(firearm));
    assert!(pending.resumable);
    assert_eq!(world.collection(), Err("PENDING_CHANGES_UNRESOLVED".into()));
    assert_eq!(world.pending_rows(), 1);
    assert_eq!(world.staged(), None);

    // Back where it began: the draft can be resumed again, exactly.
    let again = world.resolve(PendingAction::Resume).unwrap();
    assert_eq!(again.draft, Some(edit_draft(firearm)));
}

// 003 US6 Scenario 6: discarding.
#[test]
fn discard_deletes_the_row_and_serves_the_collection() {
    let world = World::new();
    world.unresolved();

    let resolved = world.resolve(PendingAction::Discard).unwrap();

    assert_eq!(resolved, PendingResolved { draft: None, pending: None });
    assert_eq!(world.observed(), (Ok(()), 0));
    assert_eq!(world.staged(), None);
}

#[test]
fn an_action_names_its_session() {
    let world = World::new();
    world.unresolved();
    let ended = world.id();
    world.open();

    let late = databases::resolve_pending_changes(&world.session, ended, PendingAction::Discard);

    assert_eq!(late.unwrap_err().code, "DATABASE_CLOSED");
    assert_eq!(world.observed(), shows(State::Unresolved), "the new session is untouched");
}

// --- A session that ends before the form has opened ---------------------------------

#[test]
fn a_lock_while_resuming_with_nothing_staged_leaves_the_row_and_the_next_open_starts_unresolved() {
    let world = World::new();
    let firearm = world.unresolved();
    world.resolve(PendingAction::Resume).unwrap();

    lifecycle::lock_with_staged(&world.session, &world.machine, CloseReason::Idle).unwrap();

    assert!(!world.session.is_open());
    assert_eq!(world.closed_rows(), 1, "the row is still there");
    let status = databases::open_database(
        &world.session,
        &world.machine,
        &world.path_text(),
        databases::Unlock::typed(&passphrase()),
        false,
    )
    .unwrap();
    let pending = status.pending_changes.expect("the next open reports the changes");
    assert_eq!(pending.target_id, Some(firearm));
    assert_eq!(world.observed(), shows(State::Unresolved));
    // And they are intact.
    let resumed = world.resolve(PendingAction::Resume).unwrap();
    assert_eq!(resumed.draft, Some(edit_draft(firearm)));
}

#[test]
fn a_user_lock_a_sleep_and_a_close_while_resuming_leave_the_row() {
    for how in ["lock", "sleep", "close"] {
        let world = World::new();
        world.unresolved();
        world.resolve(PendingAction::Resume).unwrap();

        match how {
            "lock" => {
                databases::lock_database(&world.session, world.id(), &world.machine, None).unwrap();
            }
            "sleep" => {
                lifecycle::close_immediate(&world.session, &world.machine, CloseReason::Sleep)
            }
            _ => {
                lifecycle::close_normal(
                    &world.session,
                    world.id(),
                    &world.machine,
                    CloseReason::Closed,
                )
                .unwrap();
            }
        }

        assert!(!world.session.is_open(), "{how}");
        assert_eq!(world.closed_rows(), 1, "{how}");
        world.open();
        assert_eq!(world.observed(), shows(State::Unresolved), "{how}");
    }
}

// 008 US4 Scenarios 2 and 5: closing from the dialog.
#[test]
fn close_database_is_served_in_unresolved_and_in_resuming() {
    for state in [State::Unresolved, State::Resuming] {
        let world = World::new();
        world.reach(state);
        let id = world.id();

        lifecycle::close_normal(&world.session, id, &world.machine, CloseReason::Closed)
            .unwrap_or_else(|e| panic!("closing in {state:?}: {e:?}"));

        assert!(!world.session.is_open(), "{state:?}");
        assert_eq!(world.closed_rows(), 1, "{state:?} leaves the row exactly as it was");
    }
}

// --- `opened` stages the draft -------------------------------------------------------

#[test]
fn after_opened_a_lock_with_nothing_staged_by_a_form_keeps_the_changes() {
    let world = World::new();
    let firearm = world.unresolved();
    world.resolve(PendingAction::Resume).unwrap();
    world.resolve(PendingAction::Opened).unwrap();
    assert_eq!(world.pending_rows(), 0, "the row went when the form opened");

    lifecycle::lock_with_staged(&world.session, &world.machine, CloseReason::Idle).unwrap();

    assert_eq!(world.closed_rows(), 1, "the staged draft was written back");
    world.open();
    assert_eq!(world.resolve(PendingAction::Resume).unwrap().draft, Some(edit_draft(firearm)));
}

#[test]
fn after_opened_a_sleep_or_shutdown_keeps_the_changes() {
    for reason in [CloseReason::Sleep, CloseReason::Shutdown] {
        let world = World::new();
        let firearm = world.unresolved();
        world.resolve(PendingAction::Resume).unwrap();
        world.resolve(PendingAction::Opened).unwrap();

        lifecycle::close_immediate(&world.session, &world.machine, reason);

        assert_eq!(world.closed_rows(), 1, "{reason:?}");
        world.open();
        let resumed = world.resolve(PendingAction::Resume).unwrap();
        assert_eq!(resumed.draft, Some(edit_draft(firearm)), "{reason:?}");
    }
}

#[test]
fn the_forms_own_staging_replaces_the_one_opened_made() {
    let world = World::new();
    let firearm = world.unresolved();
    world.resolve(PendingAction::Resume).unwrap();
    world.resolve(PendingAction::Opened).unwrap();
    let mut typed_on = edit_draft(firearm);
    typed_on.values = json!({ "make": "Glock", "model": "19", "notes": "and some more" });

    databases::stage_pending_changes(&world.session, world.id(), Some(typed_on.clone())).unwrap();
    lifecycle::lock_with_staged(&world.session, &world.machine, CloseReason::Idle).unwrap();

    world.open();
    assert_eq!(world.resolve(PendingAction::Resume).unwrap().draft, Some(typed_on));
}

// --- Closing from the dialog (US4-2, US4-3, US4-5) --------------------------------

// 008 US4 Scenarios 2 and 5; 003 US6 Scenario 6 (a backup carries no pending changes).
#[test]
fn a_close_leaves_the_row_and_its_backup_has_no_pending_changes() {
    for state in [State::Unresolved, State::Resuming] {
        let world = World::new();
        world.reach(state);
        // Something to back up: a saved change (the form's record can load
        // while resuming; while unresolved only the creation's remains).
        if state == State::Resuming {
            world
                .session
                .write(world.id(), |conn| {
                    firearms::create_firearm(
                        conn,
                        &support::firearm("Colt", "Python", "R2"),
                        false,
                        None,
                    )
                })
                .unwrap();
        }
        let id = world.database_id();
        let folder = world.dir.path().join("HoploDex backups");

        lifecycle::close_normal(&world.session, world.id(), &world.machine, CloseReason::Closed)
            .unwrap();

        assert_eq!(world.closed_rows(), 1, "{state:?}: the row is untouched");
        let made = backups::list(&folder, &id).unwrap();
        if state == State::Resuming {
            assert!(!made.is_empty(), "{state:?}: a change waiting makes the normal close back up");
        }
        for backup in &made {
            let rows: i64 = peek(std::path::Path::new(&backup.path))
                .query_row("SELECT count(*) FROM pending_changes", [], |row| row.get(0))
                .unwrap();
            assert_eq!(rows, 0, "{state:?}: a backup never carries pending changes");
        }
        // The next open reports them again.
        world.open();
        assert_eq!(world.observed(), shows(State::Unresolved), "{state:?}");
    }
}

// 008 US4 Scenario 3; 003 US6 Scenario 6.
#[test]
fn resume_after_a_close_returns_every_field_as_written() {
    let world = World::new();
    let firearm = world.unresolved();
    lifecycle::close_normal(&world.session, world.id(), &world.machine, CloseReason::Closed)
        .unwrap();

    world.open();
    let status = databases::database_status(&world.session, &world.machine).unwrap();
    assert_eq!(status.pending_changes.map(|p| p.target_id), Some(Some(firearm)));
    let resumed = world.resolve(PendingAction::Resume).unwrap();

    assert_eq!(resumed.draft, Some(edit_draft(firearm)));
}

// --- The version that kept them (FR-015) ------------------------------------------

#[test]
fn opened_and_discard_record_this_version_and_resume_and_not_opened_do_not() {
    // `opened`, after `resume`.
    let world = World::new();
    world.unresolved();
    world.seed_saved_version("1.2.0");
    world.resolve(PendingAction::Resume).unwrap();
    assert_eq!(world.saved_version().as_deref(), Some("1.2.0"), "resume changes no rows");
    world.resolve(PendingAction::NotOpened).unwrap();
    assert_eq!(world.saved_version().as_deref(), Some("1.2.0"), "notOpened changes no rows");
    world.resolve(PendingAction::Resume).unwrap();
    world.resolve(PendingAction::Opened).unwrap();
    assert_eq!(world.saved_version().as_deref(), Some(APP_VERSION), "opened deletes the row");

    // `discard`.
    let world = World::new();
    world.unresolved();
    world.seed_saved_version("1.2.0");
    world.resolve(PendingAction::Discard).unwrap();
    assert_eq!(world.saved_version().as_deref(), Some(APP_VERSION), "discard deletes the row");
}
