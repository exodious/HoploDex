//! Scratch files are only acted on when they are ours (#63): recovery and
//! cleanup leave a symlink, a folder or a file with other names alone, secure
//! deletion never writes through a link, and nothing is created by
//! truncating what is already at a predictable name. The operations that
//! make scratch files (a passphrase change, a restore, a backup) have their
//! collision tests beside their other tests.
//!
//! Real files in temp folders. Hard links need no privilege on NTFS, so the
//! hard-link cases run on every OS; the symlink cases are Unix-only.

use std::fs;
use std::path::{Path, PathBuf};

use hoplodex_lib::services::file_swap;
use hoplodex_lib::services::scratch;
use hoplodex_lib::services::secure_delete::{self, WipeControl};
use tempfile::TempDir;

struct Folder {
    dir: TempDir,
}

impl Folder {
    fn new() -> Self {
        Self { dir: TempDir::new().unwrap() }
    }

    fn original(&self) -> PathBuf {
        self.dir.path().join("Mine.hoplodex")
    }

    fn new_copy(&self) -> PathBuf {
        file_swap::new_path(&self.original())
    }

    fn old_copy(&self) -> PathBuf {
        file_swap::old_path(&self.original())
    }

    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

/// A file outside the folder that a planted link points at.
fn victim() -> (TempDir, PathBuf) {
    let elsewhere = TempDir::new().unwrap();
    let victim = elsewhere.path().join("victim.txt");
    fs::write(&victim, b"precious").unwrap();
    (elsewhere, victim)
}

fn recover(path: &Path) {
    file_swap::finish_interrupted(path);
    file_swap::remove_leftovers(path);
}

#[test]
fn a_scratch_file_is_created_exclusively() {
    let folder = Folder::new();
    let taken = folder.dir.path().join("taken");
    fs::write(&taken, b"already here").unwrap();

    let err = scratch::create(&taken).unwrap_err();

    assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read(&taken).unwrap(), b"already here", "not truncated");
}

#[test]
fn recovery_leaves_a_folder_at_the_scratch_names_alone() {
    let folder = Folder::new();
    fs::write(folder.original(), b"contents").unwrap();
    fs::create_dir(folder.new_copy()).unwrap();
    fs::write(folder.new_copy().join("inside"), b"kept").unwrap();
    fs::create_dir(folder.old_copy()).unwrap();

    recover(&folder.original());

    assert_eq!(fs::read(folder.new_copy().join("inside")).unwrap(), b"kept");
    assert!(folder.old_copy().is_dir());
}

#[test]
fn recovery_leaves_a_file_with_other_names_alone() {
    let folder = Folder::new();
    let (_elsewhere, victim) = victim();
    fs::write(folder.original(), b"contents").unwrap();
    fs::hard_link(&victim, folder.new_copy()).unwrap();
    fs::hard_link(&victim, folder.old_copy()).unwrap();

    recover(&folder.original());

    assert_eq!(fs::read(&victim).unwrap(), b"precious", "not overwritten through the link");
    assert!(folder.new_copy().exists() && folder.old_copy().exists(), "and not removed");
}

#[test]
fn a_missing_database_is_not_replaced_by_a_file_with_other_names() {
    let folder = Folder::new();
    let (_elsewhere, victim) = victim();
    fs::hard_link(&victim, folder.new_copy()).unwrap();

    recover(&folder.original());

    assert!(fs::symlink_metadata(folder.original()).is_err());
    assert_eq!(folder.names(), vec![".Mine.hoplodex.new"]);
    assert_eq!(fs::read(&victim).unwrap(), b"precious");
}

#[test]
fn securely_deleting_a_hard_linked_file_leaves_the_other_name_intact() {
    let folder = Folder::new();
    let kept = folder.dir.path().join("kept.txt");
    let planted = folder.dir.path().join("planted.txt");
    fs::write(&kept, b"precious").unwrap();
    fs::hard_link(&kept, &planted).unwrap();

    secure_delete::secure_delete_file(&planted).unwrap();

    assert!(!planted.exists());
    assert_eq!(fs::read(&kept).unwrap(), b"precious", "the other name's contents survive");
}

#[test]
fn securely_deleting_a_whole_hard_linked_file_leaves_the_other_name_intact() {
    let folder = Folder::new();
    let kept = folder.dir.path().join("kept.txt");
    let planted = folder.dir.path().join("planted.txt");
    fs::write(&kept, b"precious").unwrap();
    fs::hard_link(&kept, &planted).unwrap();

    secure_delete::secure_delete_whole_file(&planted, WipeControl::default()).unwrap();

    assert!(!planted.exists());
    assert_eq!(fs::read(&kept).unwrap(), b"precious");
}

#[cfg(unix)]
mod unix {
    use super::*;
    use hoplodex_lib::services::machine_settings::MachineSettings;
    use std::os::unix::fs::symlink;

    #[test]
    fn creating_over_a_symlink_fails_and_does_not_follow_it() {
        let folder = Folder::new();
        let (_elsewhere, victim) = victim();
        let link = folder.dir.path().join("link");
        symlink(&victim, &link).unwrap();

        assert!(scratch::create(&link).is_err());

        assert_eq!(fs::read(&victim).unwrap(), b"precious");
        assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
    }

    #[test]
    fn recovery_leaves_a_symlink_at_the_scratch_names_alone() {
        let folder = Folder::new();
        let (_elsewhere, victim) = victim();
        fs::write(folder.original(), b"contents").unwrap();
        symlink(&victim, folder.new_copy()).unwrap();
        symlink(&victim, folder.old_copy()).unwrap();

        recover(&folder.original());

        assert_eq!(fs::read(&victim).unwrap(), b"precious");
        assert!(fs::symlink_metadata(folder.new_copy()).unwrap().file_type().is_symlink());
        assert!(fs::symlink_metadata(folder.old_copy()).unwrap().file_type().is_symlink());
    }

    #[test]
    fn recovery_does_not_move_a_symlink_into_a_missing_databases_place() {
        let folder = Folder::new();
        let (_elsewhere, victim) = victim();
        symlink(&victim, folder.new_copy()).unwrap();
        symlink(&victim, folder.old_copy()).unwrap();

        recover(&folder.original());

        assert!(fs::symlink_metadata(folder.original()).is_err());
        assert_eq!(fs::read(&victim).unwrap(), b"precious");
        assert_eq!(folder.names(), vec![".Mine.hoplodex.new", ".Mine.hoplodex.old"]);
    }

    #[test]
    fn replacing_refuses_a_new_copy_that_has_become_a_symlink() {
        let folder = Folder::new();
        let (_elsewhere, victim) = victim();
        fs::write(folder.original(), b"old contents").unwrap();
        symlink(&victim, folder.new_copy()).unwrap();

        let result = file_swap::replace(&folder.original());

        assert_eq!(result.unwrap_err().code, "REPLACE_FAILED");
        assert_eq!(fs::read(folder.original()).unwrap(), b"old contents");
        assert_eq!(fs::read(&victim).unwrap(), b"precious");
        assert!(fs::symlink_metadata(folder.new_copy()).is_ok(), "the link is not ours to remove");
    }

    #[test]
    fn securely_deleting_a_symlink_leaves_its_target_intact() {
        let folder = Folder::new();
        let (_elsewhere, victim) = victim();
        let link = folder.dir.path().join("link");
        symlink(&victim, &link).unwrap();

        secure_delete::secure_delete_file(&link).unwrap();
        assert_eq!(fs::read(&victim).unwrap(), b"precious");
        symlink(&victim, &link).unwrap();
        secure_delete::secure_delete_whole_file(&link, WipeControl::default()).unwrap();

        assert!(fs::symlink_metadata(&link).is_err());
        assert_eq!(fs::read(&victim).unwrap(), b"precious");
    }

    #[test]
    fn a_planted_symlink_at_the_settings_temporary_name_is_not_followed() {
        let config = TempDir::new().unwrap();
        let (_elsewhere, victim) = victim();
        // The name the temporary file used to have.
        symlink(&victim, config.path().join("machine.json.tmp")).unwrap();

        // The first load writes the file, and so does every change after it.
        let settings = MachineSettings::load(config.path()).unwrap();
        assert_eq!(fs::read(&victim).unwrap(), b"precious");
        settings.touch_recent(Path::new("/data/Mine.hoplodex"), "Mine", "abcd", Path::new("/data"));

        assert_eq!(fs::read(&victim).unwrap(), b"precious");
    }
}

#[test]
fn securely_deleting_an_ordinary_file_still_overwrites_and_removes_it() {
    let folder = Folder::new();
    let path = folder.dir.path().join("plain.txt");
    fs::write(&path, b"contents").unwrap();

    secure_delete::secure_delete_file(&path).unwrap();

    assert!(!path.exists());
}
