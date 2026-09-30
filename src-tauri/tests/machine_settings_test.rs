//! `machine.json`, the machine-local settings file (research.md §6, §11;
//! data-model.md "Machine-local"). Every case uses a temp config directory,
//! never the real one.

use std::fs;
use std::path::{Path, PathBuf};

use hoplodex_lib::models::database::{ChooserNotice, OperationKind};
use hoplodex_lib::services::machine_settings::{
    MachineSettings, UnfinishedBackup, UnfinishedBackupMove,
};
use tempfile::TempDir;

fn names(settings: &MachineSettings) -> Vec<String> {
    settings.recent().into_iter().map(|r| r.name).collect()
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn the_first_load_makes_a_machine_id_that_stays_the_same() {
    let config = TempDir::new().unwrap();

    let first = MachineSettings::load(config.path()).unwrap().machine_id();
    let again = MachineSettings::load(config.path()).unwrap().machine_id();

    assert_eq!(first.len(), 32);
    assert!(first.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(first, again);
    assert!(config.path().join("machine.json").exists());
}

#[test]
fn a_missing_config_directory_is_created() {
    let config = TempDir::new().unwrap();
    let nested = config.path().join("a").join("com.hoplodex.inventory");

    let settings = MachineSettings::load(&nested).unwrap();

    assert_eq!(settings.machine_id().len(), 32);
    assert!(nested.join("machine.json").exists());
}

#[test]
fn the_identity_names_this_computer() {
    let config = TempDir::new().unwrap();
    let settings = MachineSettings::load(config.path()).unwrap();

    let identity = settings.identity();

    assert_eq!(identity.id, settings.machine_id());
    assert!(!identity.display_name.is_empty());
    assert!(!identity.display_name.ends_with(".local"));
}

#[test]
fn touch_recent_keeps_the_most_recent_first_and_refreshes_the_name() {
    let config = TempDir::new().unwrap();
    let settings = MachineSettings::load(config.path()).unwrap();
    let a = PathBuf::from("/data/A.hoplodex");
    let b = PathBuf::from("/data/B.hoplodex");

    settings.touch_recent(&a, "A", "a".repeat(32).as_str(), Path::new("/data/HoploDex backups"));
    settings.touch_recent(&b, "B", "b".repeat(32).as_str(), Path::new("/data/HoploDex backups"));
    assert_eq!(names(&settings), ["B", "A"]);

    settings.touch_recent(&a, "A renamed", "a".repeat(32).as_str(), Path::new("/elsewhere"));

    let recent = MachineSettings::load(config.path()).unwrap().recent();
    assert_eq!(recent.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["A renamed", "B"]);
    assert_eq!(recent[0].path, a);
    assert_eq!(recent[0].database_id.as_deref(), Some("a".repeat(32).as_str()));
    assert_eq!(recent[0].backup_folder.as_deref(), Some(Path::new("/elsewhere")));
    assert!(!recent[0].passphrase_saved);
    assert!(recent[0].last_opened_at >= recent[1].last_opened_at);
}

#[test]
fn writes_leave_no_temporary_file_behind() {
    let config = TempDir::new().unwrap();
    let settings = MachineSettings::load(config.path()).unwrap();

    settings.touch_recent(Path::new("/x.hoplodex"), "x", &"c".repeat(32), Path::new("/b"));
    settings.push_notice(ChooserNotice::PendingChangesLost { database_path: "/x.hoplodex".into() });

    assert_eq!(files_in(config.path()), ["machine.json"]);
}

#[test]
fn a_corrupt_file_is_set_aside_and_a_fresh_one_started() {
    let config = TempDir::new().unwrap();
    let path = config.path().join("machine.json");
    fs::write(&path, b"{ this is not json").unwrap();

    let settings = MachineSettings::load(config.path()).unwrap();

    assert!(settings.recent().is_empty());
    assert_eq!(settings.machine_id().len(), 32);
    assert_eq!(fs::read(config.path().join("machine.json.bad")).unwrap(), b"{ this is not json");
    let fresh: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(fresh["version"], 1);
}

#[test]
fn notices_are_returned_once() {
    let config = TempDir::new().unwrap();
    let settings = MachineSettings::load(config.path()).unwrap();
    let stopped = ChooserNotice::OperationStopped {
        database_path: "/x.hoplodex".into(),
        operation: OperationKind::Import,
        imported_count: Some(12),
        deleted_count: None,
        left_behind_count: None,
        folder: None,
    };
    settings.push_notice(stopped.clone());

    // Notices survive a restart until they are shown.
    let reloaded = MachineSettings::load(config.path()).unwrap();
    assert_eq!(reloaded.take_notices(), vec![stopped]);
    assert!(reloaded.take_notices().is_empty());
    assert!(MachineSettings::load(config.path()).unwrap().take_notices().is_empty());
}

#[test]
fn the_file_uses_the_documented_shape() {
    let config = TempDir::new().unwrap();
    let settings = MachineSettings::load(config.path()).unwrap();
    settings.touch_recent(Path::new("/x.hoplodex"), "x", &"d".repeat(32), Path::new("/b"));
    settings.push_notice(ChooserNotice::OperationStopped {
        database_path: "/x.hoplodex".into(),
        operation: OperationKind::Import,
        imported_count: None,
        deleted_count: None,
        left_behind_count: None,
        folder: None,
    });

    let json: serde_json::Value =
        serde_json::from_slice(&fs::read(config.path().join("machine.json")).unwrap()).unwrap();

    assert_eq!(json["version"], 1);
    assert!(json["machineId"].is_string());
    let entry = &json["recentDatabases"][0];
    assert_eq!(entry["path"], "/x.hoplodex");
    assert_eq!(entry["name"], "x");
    assert!(entry["lastOpenedAt"].is_string());
    assert_eq!(entry["databaseId"], "d".repeat(32));
    assert_eq!(entry["backupFolder"], "/b");
    assert_eq!(entry["passphraseSaved"], false);
    assert!(json["unfinishedBackup"].is_null());
    assert_eq!(
        json["notices"][0],
        serde_json::json!({
            "kind": "operationStopped", "databasePath": "/x.hoplodex", "operation": "import"
        })
    );
}

#[test]
fn an_unfinished_backup_survives_a_reload_until_cleared() {
    let config = TempDir::new().unwrap();
    let settings = MachineSettings::load(config.path()).unwrap();
    let record = UnfinishedBackup {
        database_path: "/data/A.hoplodex".into(),
        partial_path: "/data/HoploDex backups/A 2026-09-26 101500 abcd1234.hoplodex.partial".into(),
        started_at: "2026-09-26T10:15:00Z".into(),
    };

    settings.set_unfinished_backup(record.clone());
    assert_eq!(MachineSettings::load(config.path()).unwrap().unfinished_backup(), Some(record));

    settings.clear_unfinished_backup();
    assert_eq!(MachineSettings::load(config.path()).unwrap().unfinished_backup(), None);
}

#[test]
fn an_unfinished_backup_move_survives_a_reload_until_cleared() {
    let config = TempDir::new().unwrap();
    let settings = MachineSettings::load(config.path()).unwrap();
    let record = UnfinishedBackupMove {
        database_path: "/data/A.hoplodex".into(),
        database_id: "a".repeat(32),
        from_folder: "/data/HoploDex backups".into(),
        partial_path: None,
    };
    let partial = Path::new("/usb/backups/A 2026-09-26 101500 aaaaaaaa.hoplodex.partial");

    settings.set_unfinished_backup_move(record.clone());
    assert_eq!(
        MachineSettings::load(config.path()).unwrap().unfinished_backup_move(),
        Some(record.clone())
    );
    settings.set_unfinished_backup_move_partial(Some(partial));
    let json: serde_json::Value =
        serde_json::from_slice(&fs::read(config.path().join("machine.json")).unwrap()).unwrap();
    assert_eq!(
        json["unfinishedBackupMove"],
        serde_json::json!({
            "databasePath": "/data/A.hoplodex",
            "databaseId": "a".repeat(32),
            "fromFolder": "/data/HoploDex backups",
            "partialPath": partial,
        })
    );
    settings.set_unfinished_backup_move_partial(None);
    assert_eq!(
        MachineSettings::load(config.path()).unwrap().unfinished_backup_move(),
        Some(record)
    );

    settings.clear_unfinished_backup_move();
    assert_eq!(MachineSettings::load(config.path()).unwrap().unfinished_backup_move(), None);
}

#[test]
fn saving_a_backup_location_updates_only_the_cached_folder() {
    let config = TempDir::new().unwrap();
    let settings = MachineSettings::load(config.path()).unwrap();
    let a = Path::new("/data/A.hoplodex");
    settings.touch_recent(a, "A", &"a".repeat(32), Path::new("/data/HoploDex backups"));
    let before = settings.recent_entry(a).unwrap();

    settings.set_backup_folder(a, Path::new("/usb/backups"));
    settings.set_backup_folder(Path::new("/not/listed.hoplodex"), Path::new("/x"));

    let after = MachineSettings::load(config.path()).unwrap().recent();
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].backup_folder.as_deref(), Some(Path::new("/usb/backups")));
    assert_eq!(after[0].last_opened_at, before.last_opened_at);
}

// --- User Story 2 -----------------------------------------------------------

#[test]
fn locate_replaces_the_path_and_keeps_everything_else() {
    let config = TempDir::new().unwrap();
    let settings = MachineSettings::load(config.path()).unwrap();
    let a = PathBuf::from("/data/A.hoplodex");
    let b = PathBuf::from("/data/B.hoplodex");
    settings.touch_recent(&a, "A", &"a".repeat(32), Path::new("/data/HoploDex backups"));
    settings.touch_recent(&b, "B", &"b".repeat(32), Path::new("/data/HoploDex backups"));
    let before = settings.recent();

    let located = settings.locate_recent(&a, Path::new("/usb/A.hoplodex")).unwrap();

    let recent = MachineSettings::load(config.path()).unwrap().recent();
    assert_eq!(recent.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["B", "A"]);
    assert_eq!(recent[1], located);
    assert_eq!(located.path, Path::new("/usb/A.hoplodex"));
    assert_eq!(located.name, before[1].name);
    assert_eq!(located.last_opened_at, before[1].last_opened_at);
    assert_eq!(located.database_id, before[1].database_id);
    assert_eq!(located.backup_folder, before[1].backup_folder);
    assert_eq!(located.passphrase_saved, before[1].passphrase_saved);
    assert_eq!(settings.locate_recent(Path::new("/nowhere.hoplodex"), &a), None);
}

#[test]
fn locate_onto_a_path_already_listed_keeps_one_entry_for_it() {
    let config = TempDir::new().unwrap();
    let settings = MachineSettings::load(config.path()).unwrap();
    let a = PathBuf::from("/data/A.hoplodex");
    let b = PathBuf::from("/data/B.hoplodex");
    settings.touch_recent(&a, "A", &"a".repeat(32), Path::new("/b"));
    settings.touch_recent(&b, "B", &"b".repeat(32), Path::new("/b"));

    settings.locate_recent(&b, &a).unwrap();

    let recent = settings.recent();
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0].path, a);
    assert_eq!(recent[0].database_id.as_deref(), Some("b".repeat(32).as_str()));
}

#[test]
fn remove_drops_only_the_entry_and_never_the_file() {
    let config = TempDir::new().unwrap();
    let data = TempDir::new().unwrap();
    let settings = MachineSettings::load(config.path()).unwrap();
    let a = data.path().join("A.hoplodex");
    let b = data.path().join("B.hoplodex");
    fs::write(&a, b"database A").unwrap();
    settings.touch_recent(&a, "A", &"a".repeat(32), Path::new("/b"));
    settings.touch_recent(&b, "B", &"b".repeat(32), Path::new("/b"));

    assert!(settings.remove_recent(&a).is_some());
    assert!(settings.remove_recent(&a).is_none(), "already gone");

    assert_eq!(names(&MachineSettings::load(config.path()).unwrap()), ["B"]);
    assert_eq!(fs::read(&a).unwrap(), b"database A");
    assert_eq!(files_in(data.path()), ["A.hoplodex"]);
}
