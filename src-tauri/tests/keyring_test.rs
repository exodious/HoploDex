//! Remembering a database's passphrase on this computer (FR-017–FR-019,
//! SC-008; research.md §10). Real SQLCipher files in temp directories and
//! keyring-core's in-memory store, never the OS keyring: run with
//! `--features mock-keyring`.
#![cfg(feature = "mock-keyring")]

mod support;

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::commands::backups::ops as backups_ops;
use hoplodex_lib::commands::databases::ops::{self, Unlock};
use hoplodex_lib::commands::firearms::ops as firearms;
use hoplodex_lib::models::database::{CloseReason, DatabaseStatus};
use hoplodex_lib::services::keyring::Keyring;
use hoplodex_lib::services::machine_settings::MachineSettings;
use hoplodex_lib::services::passphrase::Passphrase;
use hoplodex_lib::session::{Session, lifecycle};
use support::{TEST_PASSPHRASE, TestEvents, passphrase, test_session};
use tempfile::TempDir;

const SERVICE: &str = "io.github.exodious.HoploDex";
const NEW_PASSPHRASE: &str = "a much longer passphrase of several words";

/// keyring-core's in-memory store is one per process, shared by the tests
/// running beside each other. The E2E keyring file is written from all of it
/// and loaded back into it, which could bring back a passphrase another test
/// had forgotten in between, so that test has the store to itself.
static STORE: RwLock<()> = RwLock::new(());

fn sharing_the_store() -> RwLockReadGuard<'static, ()> {
    STORE.read().unwrap_or_else(PoisonError::into_inner)
}

fn store_to_itself() -> RwLockWriteGuard<'static, ()> {
    STORE.write().unwrap_or_else(PoisonError::into_inner)
}

struct World {
    folder: TempDir,
    _config: TempDir,
    scratch: TempDir,
    machine: MachineSettings,
    session: Session,
    _events: Arc<TestEvents>,
    _store: Option<RwLockReadGuard<'static, ()>>,
}

impl World {
    /// A computer whose keyring works.
    fn new() -> Self {
        Self::with_keyring(Keyring::mock(None, None))
    }

    fn with_keyring(keyring: Keyring) -> Self {
        Self { _store: Some(sharing_the_store()), ..Self::with_the_store_to_itself(keyring) }
    }

    /// For a test that holds [`store_to_itself`].
    fn with_the_store_to_itself(keyring: Keyring) -> Self {
        let config = TempDir::new().unwrap();
        let (session, events) = test_session(&config.path().join("opened-documents"));
        Self {
            folder: TempDir::new().unwrap(),
            scratch: TempDir::new().unwrap(),
            machine: MachineSettings::load(config.path()).unwrap().with_keyring(keyring),
            _config: config,
            session,
            _events: events,
            _store: None,
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.folder.path().join(format!("{name}.hoplodex"))
    }

    fn path_text(&self, name: &str) -> String {
        self.path(name).to_string_lossy().into_owned()
    }

    /// Creates `name` with the test passphrase and leaves it open.
    fn create(&self, name: &str) {
        lifecycle::create(&self.session, &self.machine, &self.path(name), &passphrase()).unwrap();
    }

    fn close(&self) {
        lifecycle::close_normal(&self.session, &self.machine, CloseReason::Closed).unwrap();
    }

    fn database_id(&self) -> String {
        self.session.inspect(|open| Ok(open.database_id.clone())).unwrap()
    }

    fn open(&self, name: &str, unlock: Unlock<'_>) -> Result<DatabaseStatus, CommandError> {
        ops::open_database(&self.session, &self.machine, &self.path_text(name), unlock, false)
    }

    fn open_typed(&self, name: &str, text: &str) -> Result<DatabaseStatus, CommandError> {
        self.open(name, Unlock::typed(&Passphrase::from_input(text.to_owned())))
    }

    fn open_remembering(&self, name: &str) -> DatabaseStatus {
        self.open(name, Unlock::Typed { passphrase: &passphrase(), remember: true }).unwrap()
    }

    fn save(&self, text: &str) -> Result<(), CommandError> {
        let typed = Passphrase::from_input(text.to_owned());
        ops::save_passphrase(&self.session, &self.machine, self.scratch.path(), &typed).map(|_| ())
    }

    fn saved_flag(&self, name: &str) -> bool {
        self.machine.recent_entry(&self.path(name)).is_some_and(|entry| entry.passphrase_saved)
    }

    fn add_firearm(&self, serial: &str) {
        self.session
            .write(|conn| {
                firearms::create_firearm(
                    conn,
                    &support::firearm("Colt", "Python", serial),
                    false,
                    None,
                )
                .map(|_| ())
            })
            .unwrap();
    }
}

/// What the keyring holds for `database_id`, read straight from the store.
fn stored(database_id: &str) -> Option<String> {
    let entry = keyring_core::Entry::new(SERVICE, &format!("passphrase:{database_id}")).unwrap();
    match entry.get_password() {
        Ok(saved) => Some(saved),
        Err(keyring_core::Error::NoEntry) => None,
        Err(err) => panic!("the keyring failed: {err}"),
    }
}

fn saved_failed(err: &CommandError) -> bool {
    err.code == "PASSPHRASE_INCORRECT"
        && err.details.as_deref().and_then(|d| d["savedPassphraseFailed"].as_bool()) == Some(true)
}

#[test]
fn saving_a_checked_passphrase_makes_one_entry_holding_it_normalized() {
    let world = World::new();
    // Typed decomposed, as some keyboards send it: e + combining acute.
    let decomposed = "caf\u{65}\u{301} passphrase for the safe";
    lifecycle::create(
        &world.session,
        &world.machine,
        &world.path("Main"),
        &Passphrase::from_input(decomposed.to_owned()),
    )
    .unwrap();
    let id = world.database_id();
    assert_eq!(stored(&id), None, "nothing is saved until asked (SC-002)");

    world.save(decomposed).unwrap();

    assert_eq!(stored(&id).as_deref(), Some("caf\u{e9} passphrase for the safe"));
    assert!(world.saved_flag("Main"));
    let status = ops::database_status(&world.session, &world.machine).unwrap();
    assert!(status.passphrase_saved);
    assert!(status.keyring_available);
}

#[test]
fn a_saved_passphrase_opens_its_database_while_another_still_asks() {
    let world = World::new();
    world.create("Saved");
    world.save(TEST_PASSPHRASE).unwrap();
    world.close();
    world.create("Other");
    let other_id = world.database_id();
    world.close();

    let status = world.open("Saved", Unlock::Saved).unwrap();
    assert_eq!(status.name, "Saved");
    assert!(status.passphrase_saved);
    world.close();

    let err = world.open("Other", Unlock::Saved).unwrap_err();
    assert!(saved_failed(&err), "{err:?}");
    assert!(!world.session.is_open());
    assert_eq!(stored(&other_id), None);

    let state = ops::chooser_state(&world.session, &world.machine, None, None);
    let saved: Vec<(&str, bool)> =
        state.recent.iter().map(|r| (r.name.as_str(), r.passphrase_saved)).collect();
    assert_eq!(saved, [("Saved", true), ("Other", false)], "most recent first");
    assert!(state.keyring_available);
}

#[test]
fn remembering_at_a_typed_open_writes_the_entry() {
    let world = World::new();
    world.create("Main");
    let id = world.database_id();
    world.close();

    // An ordinary open saves nothing.
    world.open_typed("Main", TEST_PASSPHRASE).unwrap();
    assert_eq!(stored(&id), None);
    world.close();

    let status = world.open_remembering("Main");
    assert!(status.passphrase_saved);
    assert_eq!(stored(&id).as_deref(), Some(TEST_PASSPHRASE));
    world.close();
    world.open("Main", Unlock::Saved).unwrap();
}

#[test]
fn forgetting_removes_the_entry_for_the_open_database_or_a_listed_one() {
    let world = World::new();
    world.create("Main");
    let id = world.database_id();
    world.save(TEST_PASSPHRASE).unwrap();

    let forgotten = ops::forget_saved_passphrase(&world.session, &world.machine, None).unwrap();
    assert!(!forgotten.passphrase_saved);
    assert_eq!(stored(&id), None);
    assert!(!world.saved_flag("Main"));
    assert!(!ops::database_status(&world.session, &world.machine).unwrap().passphrase_saved);

    // From the chooser, with nothing open.
    world.save(TEST_PASSPHRASE).unwrap();
    world.close();
    ops::forget_saved_passphrase(&world.session, &world.machine, Some(&world.path_text("Main")))
        .unwrap();
    assert_eq!(stored(&id), None);
    assert!(!world.saved_flag("Main"));
    assert!(saved_failed(&world.open("Main", Unlock::Saved).unwrap_err()));
}

#[test]
fn removing_a_database_from_the_list_forgets_its_passphrase() {
    let world = World::new();
    world.create("Main");
    let id = world.database_id();
    world.save(TEST_PASSPHRASE).unwrap();
    world.close();

    ops::remove_recent_database(&world.machine, &world.path_text("Main"));

    assert_eq!(stored(&id), None);
    assert!(world.machine.recent().is_empty());
    assert!(world.path("Main").is_file(), "the file itself is never touched");
}

#[test]
fn a_stale_saved_passphrase_is_reported_then_replaced_by_the_next_typed_one() {
    let world = World::new();
    world.create("Main");
    let id = world.database_id();
    world.save(TEST_PASSPHRASE).unwrap();
    world.close();
    // The passphrase was changed on another computer.
    keyring_core::Entry::new(SERVICE, &format!("passphrase:{id}"))
        .unwrap()
        .set_password("what it used to be, long ago")
        .unwrap();

    let err = world.open("Main", Unlock::Saved).unwrap_err();
    assert!(saved_failed(&err), "{err:?}");
    assert!(!world.session.is_open());

    let status = world.open_typed("Main", TEST_PASSPHRASE).unwrap();
    assert!(status.passphrase_saved);
    assert_eq!(stored(&id).as_deref(), Some(TEST_PASSPHRASE));
    world.close();
    world.open("Main", Unlock::Saved).unwrap();
}

#[test]
fn a_passphrase_change_updates_the_saved_one_and_a_restore_sets_the_backups() {
    let world = World::new();
    world.create("Main");
    let id = world.database_id();
    world.add_firearm("A1");
    world.save(TEST_PASSPHRASE).unwrap();
    // The close makes a backup, with the test passphrase.
    world.close();
    world.open("Main", Unlock::Saved).unwrap();

    let changed = backups_ops::change_passphrase(
        &world.session,
        &world.machine,
        world.scratch.path(),
        &passphrase(),
        &Passphrase::from_input(NEW_PASSPHRASE.to_owned()),
    )
    .unwrap();
    assert!(changed.passphrase_saved);
    assert_eq!(stored(&id).as_deref(), Some(NEW_PASSPHRASE));
    world.close();
    world.open("Main", Unlock::Saved).unwrap();

    let listed = backups_ops::list_backups(&world.session, &world.machine, None).unwrap();
    let backup = &listed.backups.last().expect("the first close made a backup").path;
    let restored =
        backups_ops::restore_backup(&world.session, &world.machine, backup, &passphrase(), None)
            .unwrap();
    assert!(restored.passphrase_saved);
    assert_eq!(stored(&id).as_deref(), Some(TEST_PASSPHRASE), "the backup's passphrase");
    world.close();
    world.open("Main", Unlock::Saved).unwrap();
}

#[test]
fn a_change_without_a_saved_passphrase_saves_nothing() {
    let world = World::new();
    world.create("Main");
    let id = world.database_id();
    let changed = backups_ops::change_passphrase(
        &world.session,
        &world.machine,
        world.scratch.path(),
        &passphrase(),
        &Passphrase::from_input(NEW_PASSPHRASE.to_owned()),
    )
    .unwrap();
    assert!(!changed.passphrase_saved);
    assert_eq!(stored(&id), None);
}

#[test]
fn a_backup_opened_directly_does_not_inherit_the_saved_passphrase() {
    let world = World::new();
    world.create("Main");
    let id = world.database_id();
    world.add_firearm("A1");
    world.save(TEST_PASSPHRASE).unwrap();
    world.close();
    world.open("Main", Unlock::Saved).unwrap();
    let listed = backups_ops::list_backups(&world.session, &world.machine, None).unwrap();
    let backup = PathBuf::from(&listed.backups[0].path);
    world.close();

    // It opens like any database, with its own new identity (research.md §9).
    ops::open_database(
        &world.session,
        &world.machine,
        &backup.to_string_lossy(),
        Unlock::typed(&passphrase()),
        false,
    )
    .unwrap();
    let backup_id = world.database_id();
    assert_ne!(backup_id, id);
    assert!(!ops::database_status(&world.session, &world.machine).unwrap().passphrase_saved);
    assert_eq!(stored(&backup_id), None);
    world.close();

    let err = ops::open_database(
        &world.session,
        &world.machine,
        &backup.to_string_lossy(),
        Unlock::Saved,
        false,
    )
    .unwrap_err();
    assert!(saved_failed(&err), "{err:?}");
    assert_eq!(stored(&id).as_deref(), Some(TEST_PASSPHRASE), "the original's is kept");
}

#[test]
fn saving_a_wrong_passphrase_is_refused_by_the_first_page_check() {
    let world = World::new();
    world.create("Main");
    let id = world.database_id();

    let err = world.save("not the passphrase of this one").unwrap_err();

    assert_eq!(err.code, "PASSPHRASE_INCORRECT");
    assert!(err.field_errors.as_ref().is_some_and(|f| f.contains_key("passphrase")));
    assert_eq!(stored(&id), None);
    assert!(!world.saved_flag("Main"));
    // The database stays open and usable.
    world.add_firearm("A1");
}

#[test]
fn without_a_keyring_nothing_is_saved_and_everything_else_works() {
    let world = World::with_keyring(Keyring::mock(Some("unavailable"), None));
    let state = ops::chooser_state(&world.session, &world.machine, None, None);
    assert!(!state.keyring_available);

    world.create("Main");
    let id = world.database_id();
    let err = world.save(TEST_PASSPHRASE).unwrap_err();
    assert_eq!(err.code, "KEYRING_UNAVAILABLE");
    assert!(!ops::database_status(&world.session, &world.machine).unwrap().keyring_available);
    world.close();

    // Asking to remember at open still opens it, without saving.
    let status = world.open_remembering("Main");
    assert!(!status.passphrase_saved);
    assert_eq!(stored(&id), None);
    world.add_firearm("A1");
    world.close();
    assert!(saved_failed(&world.open("Main", Unlock::Saved).unwrap_err()));
    world.open_typed("Main", TEST_PASSPHRASE).unwrap();
}

/// The pre-feature entry keys the developer's real database (research.md
/// §10). A mock error is put on it: any read, write or delete would consume
/// the error, so finding it still there proves nothing touched the entry.
#[test]
fn the_pre_feature_sqlcipher_key_entry_is_never_touched() {
    let world = World::new();
    let old = keyring_core::Entry::new(SERVICE, "sqlcipher-key").unwrap();
    old.set_password("the old random key").unwrap();
    let mock: &keyring_core::mock::Cred = old.as_any().downcast_ref().unwrap();
    mock.set_error(keyring_core::Error::Invalid("untouched".into(), "sentinel".into()));

    world.create("Main");
    world.save(TEST_PASSPHRASE).unwrap();
    world.close();
    ops::chooser_state(&world.session, &world.machine, None, None);
    world.open("Main", Unlock::Saved).unwrap();
    backups_ops::change_passphrase(
        &world.session,
        &world.machine,
        world.scratch.path(),
        &passphrase(),
        &Passphrase::from_input(NEW_PASSPHRASE.to_owned()),
    )
    .unwrap();
    ops::forget_saved_passphrase(&world.session, &world.machine, None).unwrap();
    world.close();
    world
        .open(
            "Main",
            Unlock::Typed {
                passphrase: &Passphrase::from_input(NEW_PASSPHRASE.to_owned()),
                remember: true,
            },
        )
        .unwrap();
    world.close();
    ops::remove_recent_database(&world.machine, &world.path_text("Main"));

    let untouched = old.get_password().unwrap_err();
    assert!(matches!(untouched, keyring_core::Error::Invalid(ref what, _) if what == "untouched"));
    assert_eq!(old.get_password().unwrap(), "the old random key");
}

/// E2E builds keep the in-memory keyring in a file between launches, so a
/// saved passphrase outlives a relaunch there.
#[test]
fn an_e2e_keyring_file_keeps_saved_passphrases_between_launches() {
    let _alone = store_to_itself();
    let files = TempDir::new().unwrap();
    let file = files.path().join("keyring.json");
    let world = World::with_the_store_to_itself(Keyring::mock(None, Some(file.clone())));
    world.create("Main");
    let id = world.database_id();
    world.save(TEST_PASSPHRASE).unwrap();
    let written = fs::read_to_string(&file).unwrap();
    assert!(written.contains(&format!("passphrase:{id}")), "{written}");
    assert!(!written.contains("sqlcipher-key"));

    // The next launch starts from the file.
    keyring_core::Entry::new(SERVICE, &format!("passphrase:{id}"))
        .unwrap()
        .delete_credential()
        .unwrap();
    let relaunched = Keyring::mock(None, Some(file.clone()));
    assert_eq!(
        relaunched.load(&id).map(|p| p.as_str().to_owned()).as_deref(),
        Some(TEST_PASSPHRASE)
    );

    relaunched.forget(&id).unwrap();
    assert!(!fs::read_to_string(&file).unwrap().contains(&id));
}
