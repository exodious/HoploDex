//! What the chooser says about a database before it is unlocked (FR-040):
//! its backups, found by file name, and whether the file changed after this
//! computer last closed it. Real SQLCipher files in temp directories;
//! machine settings in a temp config directory.

mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use chrono::DateTime;
use hoplodex_lib::commands::databases::ops;
use hoplodex_lib::db;
use hoplodex_lib::models::database::{BackupSummary, CloseReason, RecentDatabase};
use hoplodex_lib::services::backups;
use hoplodex_lib::services::machine_settings::{self, MachineSettings};
use hoplodex_lib::session::{Session, lifecycle};
use support::{other_machine, passphrase, test_session};
use tempfile::TempDir;

struct World {
    dir: TempDir,
    _config: TempDir,
    machine: MachineSettings,
    session: Session,
}

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

    fn create(&self) {
        lifecycle::create(&self.session, &self.machine, &self.path(), &passphrase()).unwrap();
    }

    fn open(&self) {
        lifecycle::open(&self.session, &self.machine, &self.path(), &passphrase(), false).unwrap();
    }

    fn close(&self) {
        lifecycle::close_normal(&self.session, &self.machine, CloseReason::Closed).unwrap();
    }

    fn listed(&self) -> RecentDatabase {
        let state = ops::chooser_state(&self.session, &self.machine, None, None);
        state.recent.into_iter().find(|r| Path::new(&r.path) == self.path()).unwrap()
    }

    fn left_modified_at(&self) -> Option<String> {
        self.machine.recent_entry(&self.path()).unwrap().left_modified_at
    }

    /// Another computer opens the database from a synced folder, changes it
    /// and closes it again. The pause lets the file's time move on.
    fn change_on_another_computer(&self) {
        thread::sleep(Duration::from_millis(50));
        let conn = db::open_database(&self.path(), &passphrase(), &other_machine(), false).unwrap();
        conn.execute("UPDATE collection_settings SET idle_lock_minutes = 12", []).unwrap();
        drop(conn);
    }

    /// An empty file named as a backup of this database made at `local`.
    fn backup_made_at(&self, local: &str) -> PathBuf {
        let entry = self.machine.recent_entry(&self.path()).unwrap();
        let id = entry.database_id.unwrap();
        let folder = entry.backup_folder.unwrap();
        fs::create_dir_all(&folder).unwrap();
        let when = DateTime::parse_from_rfc3339(local).unwrap();
        let file = folder.join(backups::backup_file_name("Mine", &when, &id));
        fs::write(&file, b"").unwrap();
        file
    }
}

#[test]
fn a_database_closed_here_is_not_reported_as_changed() {
    let world = World::new();
    world.create();
    world.close();
    world.open();
    world.close();

    // The close itself wrote to the file (the open marker, the backup
    // record), after the time the entry was opened.
    assert_eq!(world.left_modified_at(), machine_settings::modified_at(&world.path()));
    assert_eq!(world.listed().changed_since_left_at, None);
}

#[test]
fn a_change_made_on_another_computer_after_the_close_is_reported_with_its_time() {
    let world = World::new();
    world.create();
    world.close();

    world.change_on_another_computer();

    let listed = world.listed();
    assert!(listed.changed_since_left_at.is_some());
    assert_eq!(listed.changed_since_left_at, machine_settings::modified_at(&world.path()));
}

#[test]
fn nothing_is_said_while_the_time_the_file_was_left_is_unknown() {
    let world = World::new();
    // Open here and never closed, as a crash leaves it: the file has been
    // written since the entry was made.
    world.create();

    assert_eq!(world.left_modified_at(), None);
    assert_eq!(world.listed().changed_since_left_at, None);
}

#[test]
fn opening_forgets_the_time_the_file_was_left() {
    let world = World::new();
    world.create();
    world.close();
    assert!(world.left_modified_at().is_some());

    world.open();

    assert_eq!(world.left_modified_at(), None, "the open writes to the file here");
}

#[test]
fn a_lock_at_sleep_records_the_time_too() {
    let world = World::new();
    world.create();

    lifecycle::close_immediate(&world.session, &world.machine, CloseReason::Sleep);

    assert!(!world.session.is_open());
    assert_eq!(world.left_modified_at(), machine_settings::modified_at(&world.path()));
    assert_eq!(world.listed().changed_since_left_at, None);
}

#[test]
fn a_close_after_a_take_over_leaves_the_time_unknown() {
    let world = World::new();
    world.create();
    world.close();
    world.open();
    // A sync client delivers another computer's copy over the path.
    let theirs = world.dir.path().join(".sync-incoming");
    fs::copy(world.path(), &theirs).unwrap();
    fs::rename(&theirs, world.path()).unwrap();

    world.close();

    assert_eq!(world.left_modified_at(), None);
    assert_eq!(world.listed().changed_since_left_at, None);
}

#[test]
fn a_located_file_starts_with_the_time_unknown() {
    let world = World::new();
    world.create();
    world.close();
    let found = world.dir.path().join("found.hoplodex");
    fs::rename(world.path(), &found).unwrap();

    ops::locate_database(&world.machine, &world.path().to_string_lossy(), &found.to_string_lossy())
        .unwrap();

    assert_eq!(world.machine.recent_entry(&found).unwrap().left_modified_at, None);
}

#[test]
fn the_backups_are_counted_by_their_names_newest_and_oldest() {
    let world = World::new();
    world.create();
    world.close();
    assert_eq!(
        world.listed().backups,
        Some(BackupSummary { count: 0, latest_made_at: None, oldest_made_at: None })
    );

    world.backup_made_at("2026-09-14T20:45:00+00:00");
    world.backup_made_at("2026-09-28T17:40:12+00:00");
    world.backup_made_at("2026-09-22T19:30:00+00:00");
    // Another database's backup in the same folder, and a file being written.
    let folder = world.machine.recent_entry(&world.path()).unwrap().backup_folder.unwrap();
    fs::write(folder.join("Mine 2026-09-29 090000 0123abcd.hoplodex"), b"").unwrap();
    fs::write(folder.join("Mine 2026-09-29 091500 x.hoplodex.partial"), b"").unwrap();

    assert_eq!(
        world.listed().backups,
        Some(BackupSummary {
            count: 3,
            latest_made_at: Some("2026-09-28T17:40:12".to_owned()),
            oldest_made_at: Some("2026-09-14T20:45:00".to_owned()),
        })
    );
}

#[test]
fn an_unavailable_database_says_nothing_about_backups_or_changes() {
    let world = World::new();
    world.create();
    world.close();
    world.backup_made_at("2026-09-28T17:40:12+00:00");
    fs::rename(world.path(), world.dir.path().join("moved.hoplodex")).unwrap();

    let listed = world.listed();

    assert!(!listed.available);
    assert_eq!(listed.backups, None);
    assert_eq!(listed.changed_since_left_at, None);
}

#[cfg(unix)]
#[test]
fn a_backup_folder_that_cant_be_read_says_nothing_about_backups() {
    use std::os::unix::fs::PermissionsExt;

    let world = World::new();
    world.create();
    world.close();
    let backup = world.backup_made_at("2026-09-28T17:40:12+00:00");
    let folder = backup.parent().unwrap();
    fs::set_permissions(folder, fs::Permissions::from_mode(0o000)).unwrap();

    let backups = world.listed().backups;

    fs::set_permissions(folder, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(backups, None);
}

#[test]
fn a_chosen_backup_folder_that_isnt_there_says_nothing_about_backups() {
    let world = World::new();
    world.create();
    world.close();
    // A folder on a drive that isn't plugged in, or doesn't exist on this
    // computer: its backups may well be there, so "none" would be a guess.
    let unplugged = world.dir.path().join("Unplugged drive").join("Backups");
    world.machine.set_backup_folder(&world.path(), &unplugged);

    assert_eq!(world.listed().backups, None);
}

#[test]
fn a_default_backup_folder_not_yet_made_means_no_backups() {
    let world = World::new();
    world.create();
    world.close();
    let folder = world.machine.recent_entry(&world.path()).unwrap().backup_folder.unwrap();
    assert_eq!(folder, backups::resolve_folder(&world.path(), "default"));
    if folder.exists() {
        fs::remove_dir_all(&folder).unwrap();
    }

    assert_eq!(
        world.listed().backups,
        Some(BackupSummary { count: 0, latest_made_at: None, oldest_made_at: None })
    );
}
