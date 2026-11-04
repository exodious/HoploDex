//! Passphrases typed in the native dialog, never in the page (amended
//! 2026-11-04 to 003 FR-007; 003 contracts/ui-databases.md §0): what each
//! flow asks, how a wrong passphrase is asked for again under its field, what
//! Cancel returns, the backup check that keeps its passphrase for the restore,
//! and the dialogs a screen lock, a sleep or a close takes down. Real
//! SQLCipher files in temp directories, through a scripted dialog.

use crate::support;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use hoplodex_lib::commands::backups::ops as backups_ops;
use hoplodex_lib::commands::databases::ops;
use hoplodex_lib::commands::firearms::ops as firearms;
use hoplodex_lib::db;
use hoplodex_lib::models::database::{CloseReason, IdlePauseReason};
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::services::passphrase::Passphrase;
use hoplodex_lib::services::passphrase_dialog::{
    Answer, DialogWindow, Event, Field, PassphraseDialog, Request, Retry, Shape,
};
use hoplodex_lib::services::sandbox::Reach;
use hoplodex_lib::session::scoped::Target;
use hoplodex_lib::session::{Session, SessionId, lifecycle};
use support::{
    ManualClock, TEST_PASSPHRASE, TestEvents, other_machine, passphrase, test_session_at,
};
use tempfile::TempDir;

/// One press of the dialog's primary button: the passphrase and, for a
/// change, the current one. `None` is Cancel.
type Press = Option<(&'static str, Option<&'static str>)>;

/// A dialog that answers from a script, records what it was asked and the
/// retries it showed, and cancels once the script runs out.
#[derive(Default)]
struct Scripted {
    script: Mutex<VecDeque<Press>>,
    asked: Mutex<Vec<Request>>,
    retries: Mutex<Vec<Retry>>,
    /// Sends nothing when shown, so a test can take it down.
    silent: bool,
}

impl Scripted {
    fn answering(script: impl IntoIterator<Item = Press>) -> Arc<Self> {
        Arc::new(Self { script: Mutex::new(script.into_iter().collect()), ..Self::default() })
    }

    fn silent() -> Arc<Self> {
        Arc::new(Self { silent: true, ..Self::default() })
    }

    fn next(&self) -> Event {
        match self.script.lock().unwrap().pop_front().flatten() {
            Some((typed, current)) => Event::Submitted(Answer {
                passphrase: Passphrase::from_typed(typed),
                current: current.map(Passphrase::from_typed),
            }),
            None => Event::Cancelled,
        }
    }

    fn asked(&self) -> Vec<Request> {
        self.asked.lock().unwrap().clone()
    }

    fn retries(&self) -> Vec<Retry> {
        self.retries.lock().unwrap().clone()
    }
}

struct ScriptedDialog(Arc<Scripted>);

impl PassphraseDialog for ScriptedDialog {
    fn open(
        &self,
        request: &Request,
        events: Sender<Event>,
    ) -> Result<Box<dyn DialogWindow>, String> {
        self.0.asked.lock().unwrap().push(request.clone());
        if !self.0.silent {
            let _ = events.send(self.0.next());
        }
        Ok(Box::new(ScriptedWindow { script: Arc::clone(&self.0), events }))
    }
}

struct ScriptedWindow {
    script: Arc<Scripted>,
    events: Sender<Event>,
}

impl DialogWindow for ScriptedWindow {
    fn retry(&self, retry: &Retry) {
        self.script.retries.lock().unwrap().push(retry.clone());
        let _ = self.events.send(self.script.next());
    }

    fn close(&self) {}
}

const WRONG: &str = "not the passphrase at all";
const NEW: &str = "a brand new passphrase";

struct World {
    dir: TempDir,
    _config: TempDir,
    machine: Arc<MachineSettings>,
    session: Session,
    _events: Arc<TestEvents>,
    clock: Arc<ManualClock>,
}

impl World {
    fn new() -> Self {
        let config = TempDir::new().unwrap();
        let clock = ManualClock::at("2026-11-04T09:00:00-05:00");
        let (session, events) =
            test_session_at(&config.path().join("opened-documents"), clock.clone());
        Self {
            dir: TempDir::new().unwrap(),
            machine: Arc::new(MachineSettings::load(config.path()).unwrap()),
            _config: config,
            session,
            _events: events,
            clock,
        }
    }

    fn path(&self) -> PathBuf {
        self.dir.path().join("Mine.hoplodex")
    }

    fn path_text(&self) -> String {
        self.path().to_string_lossy().into_owned()
    }

    /// Creates "Mine" with the test passphrase and closes it.
    fn create_closed(&self) {
        lifecycle::create(&self.session, &self.machine, &self.path(), &passphrase()).unwrap();
        self.close();
    }

    fn close(&self) {
        lifecycle::close_normal(
            &self.session,
            self.session.current_id(),
            &self.machine,
            CloseReason::Closed,
        )
        .unwrap();
    }

    fn open_typed(
        &self,
        script: &Arc<Scripted>,
        take_over: bool,
    ) -> Result<(), hoplodex_lib::commands::CommandError> {
        let dialog = ScriptedDialog(Arc::clone(script));
        ops::open_database_typed(
            &self.session,
            &self.machine,
            &dialog,
            &self.path_text(),
            false,
            take_over,
        )
        .map(|_| ())
    }
}

#[test]
fn a_wrong_passphrase_is_asked_for_again_under_the_field_until_the_right_one() {
    let world = World::new();
    world.create_closed();
    let script = Scripted::answering([Some((WRONG, None)), Some((TEST_PASSPHRASE, None))]);

    world.open_typed(&script, false).unwrap();

    assert!(world.session.is_open());
    let asked = script.asked();
    assert_eq!(asked.len(), 1, "one dialog, asked again in place");
    assert_eq!(asked[0].title, "Open \u{201C}Mine\u{201D}");
    assert_eq!(asked[0].shape, Shape::Enter);
    assert_eq!(asked[0].primary, "Open");
    let retries = script.retries();
    assert_eq!(retries.len(), 1);
    assert_eq!(retries[0].field, Field::Passphrase);
    assert!(retries[0].message.starts_with("That passphrase didn't open \u{201C}Mine\u{201D}."));
}

#[test]
fn cancel_before_any_try_changes_nothing_and_says_so() {
    let world = World::new();
    world.create_closed();
    let script = Scripted::answering([None]);

    let err = world.open_typed(&script, false).unwrap_err();

    assert_eq!(err.code, "PASSPHRASE_CANCELLED");
    assert!(!world.session.is_open());
}

/// 003 US3-6: the chooser offers a restore after a passphrase that didn't
/// open the database, so Cancel after one returns that refusal.
#[test]
fn cancel_after_a_wrong_passphrase_returns_the_refusal() {
    let world = World::new();
    world.create_closed();
    let script = Scripted::answering([Some((WRONG, None)), None]);

    let err = world.open_typed(&script, false).unwrap_err();

    assert_eq!(err.code, "PASSPHRASE_INCORRECT");
    assert!(!world.session.is_open());
}

/// 003 FR-032: typing the passphrase again is the take-over's confirmation,
/// and the dialog names the computer the open found it open on.
#[test]
fn a_take_over_asks_again_naming_the_other_computer() {
    let world = World::new();
    {
        let conn = db::create_database(&world.path(), &passphrase(), &other_machine()).unwrap();
        conn.execute(
            "UPDATE app_state SET open_machine_id = ?1, open_machine_name = ?2, open_since = ?3",
            rusqlite::params![other_machine().id, "Workshop PC", "2026-11-03T14:30:05Z"],
        )
        .unwrap();
    }
    let first = Scripted::answering([Some((TEST_PASSPHRASE, None))]);

    let refused = world.open_typed(&first, false).unwrap_err();
    assert_eq!(refused.code, "DATABASE_OPEN_ELSEWHERE");
    assert!(!world.session.is_open());

    let again = Scripted::answering([Some((TEST_PASSPHRASE, None))]);
    world.open_typed(&again, true).unwrap();

    assert!(world.session.is_open());
    let asked = &again.asked()[0];
    assert_eq!(asked.title, "Take over \u{201C}Mine\u{201D}?");
    assert_eq!(asked.primary, "Take over");
    assert!(
        asked.message.contains("Only do this if Workshop PC no longer has"),
        "{}",
        asked.message
    );
}

/// The name and folder are checked before the passphrase is asked for, so a
/// refusal there costs no typing.
#[test]
fn creating_refuses_the_name_or_folder_before_asking_for_a_passphrase() {
    let world = World::new();
    world.create_closed();
    let script = Scripted::answering([Some((NEW, None))]);
    let dialog = ScriptedDialog(Arc::clone(&script));
    let folder = world.dir.path().to_string_lossy().into_owned();

    let exists = ops::create_database_typed(
        &world.session,
        &world.machine,
        &dialog,
        &folder,
        "Mine",
        &Reach::Unconfined,
    )
    .unwrap_err();
    let unnamed = ops::create_database_typed(
        &world.session,
        &world.machine,
        &dialog,
        &folder,
        "",
        &Reach::Unconfined,
    )
    .unwrap_err();

    assert_eq!(exists.code, "DATABASE_EXISTS");
    assert_eq!(unnamed.code, "VALIDATION_ERROR");
    assert!(script.asked().is_empty());
}

/// 003 FR-003, FR-004: the dialog asks for the new passphrase twice, with the
/// strength hint and the acknowledgement, and the database opens with it.
#[test]
fn creating_asks_for_a_new_passphrase_and_opens_with_it() {
    let world = World::new();
    let script = Scripted::answering([Some((NEW, None))]);
    let dialog = ScriptedDialog(Arc::clone(&script));
    let folder = world.dir.path().join("New folder").to_string_lossy().into_owned();

    ops::create_database_typed(
        &world.session,
        &world.machine,
        &dialog,
        &folder,
        "Fresh",
        &Reach::Unconfined,
    )
    .unwrap();

    let asked = &script.asked()[0];
    assert_eq!(asked.shape, Shape::Choose { current: false, acknowledgement: true });
    assert_eq!(asked.primary, "Create database");
    let path = world.dir.path().join("New folder").join("Fresh.hoplodex");
    world.close();
    lifecycle::open(&world.session, &world.machine, &path, &Passphrase::from_typed(NEW), false)
        .unwrap();
}

#[test]
fn cancelling_the_create_dialog_makes_nothing() {
    let world = World::new();
    let script = Scripted::answering([None]);
    let dialog = ScriptedDialog(Arc::clone(&script));
    let folder = world.dir.path().to_string_lossy().into_owned();

    let err = ops::create_database_typed(
        &world.session,
        &world.machine,
        &dialog,
        &folder,
        "Fresh",
        &Reach::Unconfined,
    )
    .unwrap_err();

    assert_eq!(err.code, "PASSPHRASE_CANCELLED");
    assert!(!world.dir.path().join("Fresh.hoplodex").exists());
}

/// 003 FR-015: a wrong current passphrase is asked for again under its own
/// field, and the change goes ahead once it is right.
#[test]
fn a_change_asks_again_for_a_wrong_current_passphrase() {
    let world = World::new();
    lifecycle::create(&world.session, &world.machine, &world.path(), &passphrase()).unwrap();
    let scratch = TempDir::new().unwrap();
    let script =
        Scripted::answering([Some((NEW, Some(WRONG))), Some((NEW, Some(TEST_PASSPHRASE)))]);
    let dialog = ScriptedDialog(Arc::clone(&script));

    backups_ops::change_passphrase_typed(
        &world.session,
        world.session.current_id(),
        &world.machine,
        scratch.path(),
        &dialog,
    )
    .unwrap();

    assert_eq!(script.retries()[0].field, Field::Current);
    assert_eq!(script.retries()[0].message, "That isn't the current passphrase.");
    assert_eq!(script.asked()[0].shape, Shape::Choose { current: true, acknowledgement: false });
    world.close();
    lifecycle::open(
        &world.session,
        &world.machine,
        &world.path(),
        &Passphrase::from_typed(NEW),
        false,
    )
    .unwrap();
}

#[test]
fn a_change_to_the_same_passphrase_is_asked_for_again() {
    let world = World::new();
    lifecycle::create(&world.session, &world.machine, &world.path(), &passphrase()).unwrap();
    let scratch = TempDir::new().unwrap();
    let script = Scripted::answering([Some((TEST_PASSPHRASE, Some(TEST_PASSPHRASE))), None]);
    let dialog = ScriptedDialog(Arc::clone(&script));

    let err = backups_ops::change_passphrase_typed(
        &world.session,
        world.session.current_id(),
        &world.machine,
        scratch.path(),
        &dialog,
    )
    .unwrap_err();

    assert_eq!(err.code, "PASSPHRASE_CANCELLED");
    assert_eq!(script.retries()[0].field, Field::Passphrase);
}

/// Makes a backup of the open "Mine" at its next close, reopens it, and
/// returns the backup's path.
fn backed_up(world: &World) -> String {
    lifecycle::create(&world.session, &world.machine, &world.path(), &passphrase()).unwrap();
    world
        .session
        .write(world.session.current_id(), |conn| {
            firearms::create_firearm(conn, &support::firearm("Colt", "Python", "A1"), false, None)
                .map(|_| ())
        })
        .unwrap();
    world.close();
    world.clock.advance(chrono::Duration::hours(1));
    lifecycle::open(&world.session, &world.machine, &world.path(), &passphrase(), false).unwrap();
    let id = world
        .session
        .inspect(world.session.current_id(), |open| Ok(open.database_id.clone()))
        .unwrap();
    let folder = world.dir.path().join("HoploDex backups");
    hoplodex_lib::services::backups::list(&folder, &id).unwrap()[0].path.clone()
}

/// 009 FR-019: the backup is checked in the dialog, and the passphrase that
/// opened it is kept under the check's id for the one restore that follows.
#[test]
fn a_checked_backup_keeps_its_passphrase_for_one_restore() {
    let world = World::new();
    let backup = backed_up(&world);
    let id = world.session.current_id();
    let script = Scripted::answering([Some((WRONG, None)), Some((TEST_PASSPHRASE, None))]);
    let dialog = ScriptedDialog(Arc::clone(&script));

    let checked = backups_ops::check_backup_typed(
        &world.session,
        &world.machine,
        &dialog,
        &backup,
        Target::Open(id),
    )
    .unwrap();

    assert_eq!(checked.upgrade_to, None);
    assert_eq!(script.asked()[0].title, "Restore from a backup");
    assert!(
        script.asked()[0].message.contains("the database had on November 4, 2026"),
        "{}",
        script.asked()[0].message
    );
    assert!(script.retries()[0].message.starts_with("That passphrase didn't open the backup"));
    backups_ops::restore_checked_backup(
        &world.session,
        &world.machine,
        checked.check_id,
        Target::Open(id),
        false,
        &Reach::Unconfined,
    )
    .unwrap();
    let again = backups_ops::restore_checked_backup(
        &world.session,
        &world.machine,
        checked.check_id,
        Target::Open(world.session.current_id()),
        false,
        &Reach::Unconfined,
    )
    .unwrap_err();
    assert_eq!(again.code, "OPERATION_STOPPED");
}

/// 003 FR-007: a screen lock wipes a passphrase kept for a restore's
/// confirmation, and the restore then changes nothing.
#[test]
fn a_screen_lock_drops_a_checked_backup() {
    let world = World::new();
    let backup = backed_up(&world);
    let id = world.session.current_id();
    let script = Scripted::answering([Some((TEST_PASSPHRASE, None))]);
    let dialog = ScriptedDialog(Arc::clone(&script));
    let checked = backups_ops::check_backup_typed(
        &world.session,
        &world.machine,
        &dialog,
        &backup,
        Target::Open(id),
    )
    .unwrap();

    lifecycle::screen_locked(&world.session, &world.machine);

    let err = backups_ops::restore_checked_backup(
        &world.session,
        &world.machine,
        checked.check_id,
        Target::Open(id),
        false,
        &Reach::Unconfined,
    )
    .unwrap_err();
    assert_eq!(err.code, "OPERATION_STOPPED");
}

/// A dialog stays up until answered: run on a thread of its own, as the
/// commands run it, while the test takes it down.
fn open_in_background(world: &World, script: &Arc<Scripted>) -> std::thread::JoinHandle<String> {
    let (session, machine, path) =
        (world.session.clone(), Arc::clone(&world.machine), world.path_text());
    let dialog = ScriptedDialog(Arc::clone(script));
    std::thread::spawn(move || {
        ops::open_database_typed(&session, &machine, &dialog, &path, false, false).unwrap_err().code
    })
}

fn wait_until_up(world: &World) {
    let start = std::time::Instant::now();
    while world.session.prompts().count() == 0 {
        assert!(start.elapsed() < std::time::Duration::from_secs(10), "the dialog never came up");
        std::thread::yield_now();
    }
}

/// 003 FR-007: a screen lock or a sleep empties and takes down a passphrase
/// dialog, whatever it was for, and the idle lock waits while one is up.
#[test]
fn a_screen_lock_or_a_sleep_takes_a_dialog_down() {
    let world = World::new();
    world.create_closed();
    for event in [lifecycle::screen_locked, lifecycle::will_sleep] {
        let script = Scripted::silent();
        let asking = open_in_background(&world, &script);
        wait_until_up(&world);
        assert!(world.session.idle().is_paused(IdlePauseReason::NativeDialog));

        event(&world.session, &world.machine);

        assert_eq!(asking.join().unwrap(), "PASSPHRASE_CANCELLED");
        assert_eq!(world.session.prompts().count(), 0);
        assert!(!world.session.idle().is_paused(IdlePauseReason::NativeDialog));
    }
}

/// A dialog for the open database (a change) goes when it closes; one that
/// isn't for it stays.
#[test]
fn closing_a_database_takes_down_only_the_dialogs_for_it() {
    let world = World::new();
    lifecycle::create(&world.session, &world.machine, &world.path(), &passphrase()).unwrap();
    let id = world.session.current_id();
    let scratch = TempDir::new().unwrap();
    let script = Scripted::silent();
    let changing = {
        let (session, machine) = (world.session.clone(), Arc::clone(&world.machine));
        let dialog = ScriptedDialog(Arc::clone(&script));
        let scratch = scratch.path().to_owned();
        std::thread::spawn(move || {
            backups_ops::change_passphrase_typed(&session, id, &machine, &scratch, &dialog)
                .unwrap_err()
                .code
        })
    };
    wait_until_up(&world);
    world.session.prompts().dismiss_for(SessionId::NONE);
    assert_eq!(world.session.prompts().count(), 1);

    world.close();

    assert_eq!(changing.join().unwrap(), "PASSPHRASE_CANCELLED");
}
