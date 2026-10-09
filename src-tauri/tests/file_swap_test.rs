//! Replacing a database file in one step, and recovering a replacement
//! that was interrupted (research.md §4; contracts/database-file.md
//! "Replacing the file"). Plain files stand in for databases: the swap never
//! looks inside them.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use hoplodex_lib::commands::CommandError;
use hoplodex_lib::services::disk_space;
use hoplodex_lib::services::file_swap::{self, testing};
use tempfile::TempDir;

/// What `lifecycle::open` does around opening the database: finish an
/// interrupted swap first, tidy what is left once the passphrase has opened it.
fn recover(path: &std::path::Path) {
    file_swap::finish_interrupted(path);
    file_swap::remove_leftovers(path);
}

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
        self.dir.path().join(".Mine.hoplodex.new")
    }

    fn old_copy(&self) -> PathBuf {
        self.dir.path().join(".Mine.hoplodex.old")
    }

    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// The original and its verified replacement, as a passphrase change or
    /// a restore leaves them just before the swap.
    fn ready(&self) {
        fs::write(self.original(), b"old contents").unwrap();
        fs::write(self.new_copy(), b"new contents").unwrap();
    }
}

#[test]
fn the_temporary_names_sit_beside_the_original() {
    let folder = Folder::new();
    assert_eq!(file_swap::new_path(&folder.original()), folder.new_copy());
    assert_eq!(file_swap::old_path(&folder.original()), folder.old_copy());
}

#[test]
fn replacing_puts_the_new_copy_in_place_and_securely_deletes_the_old_one() {
    let folder = Folder::new();
    folder.ready();

    let replaced = file_swap::replace(&folder.original()).unwrap();

    assert!(replaced.old_removed);
    assert_eq!(fs::read(folder.original()).unwrap(), b"new contents");
    assert_eq!(folder.names(), vec!["Mine.hoplodex"]);
}

#[test]
fn an_old_copy_that_cannot_be_deleted_is_reported() {
    let folder = Folder::new();
    folder.ready();
    let _stuck = testing::keep_old_copy();

    let replaced = file_swap::replace(&folder.original()).unwrap();

    assert!(!replaced.old_removed);
    assert_eq!(replaced.old_path, folder.old_copy());
    assert_eq!(fs::read(folder.old_copy()).unwrap(), b"old contents");
    assert_eq!(fs::read(folder.original()).unwrap(), b"new contents");
}

#[test]
fn without_hard_links_the_swap_uses_two_renames() {
    let folder = Folder::new();
    folder.ready();
    let _no_links = testing::disable_hard_links();

    let replaced = file_swap::replace(&folder.original()).unwrap();

    assert!(replaced.old_removed);
    assert_eq!(fs::read(folder.original()).unwrap(), b"new contents");
    assert_eq!(folder.names(), vec!["Mine.hoplodex"]);
}

#[test]
fn recovery_completes_a_swap_stopped_between_the_two_renames() {
    // The original was renamed to .old and the crash came before .new took
    // its place.
    let folder = Folder::new();
    fs::write(folder.old_copy(), b"old contents").unwrap();
    fs::write(folder.new_copy(), b"new contents").unwrap();

    recover(&folder.original());

    assert_eq!(fs::read(folder.original()).unwrap(), b"new contents");
    assert_eq!(folder.names(), vec!["Mine.hoplodex"]);
}

#[test]
fn recovery_puts_back_an_original_left_only_as_old() {
    let folder = Folder::new();
    fs::write(folder.old_copy(), b"old contents").unwrap();

    recover(&folder.original());

    assert_eq!(fs::read(folder.original()).unwrap(), b"old contents");
    assert_eq!(folder.names(), vec!["Mine.hoplodex"]);
}

#[test]
fn recovery_undoes_a_swap_stopped_before_its_rename() {
    // Hard link made, rename not yet done: .old is the original under a
    // second name, and must be unlinked, never overwritten.
    let folder = Folder::new();
    folder.ready();
    fs::hard_link(folder.original(), folder.old_copy()).unwrap();

    recover(&folder.original());

    assert_eq!(fs::read(folder.original()).unwrap(), b"old contents");
    assert_eq!(folder.names(), vec!["Mine.hoplodex"]);
}

#[test]
fn recovery_finishes_deleting_an_old_copy_after_a_completed_swap() {
    let folder = Folder::new();
    fs::write(folder.original(), b"new contents").unwrap();
    fs::write(folder.old_copy(), b"old contents").unwrap();

    recover(&folder.original());

    assert_eq!(fs::read(folder.original()).unwrap(), b"new contents");
    assert_eq!(folder.names(), vec!["Mine.hoplodex"]);
}

#[test]
fn recovery_removes_a_copy_that_never_got_as_far_as_the_swap() {
    let folder = Folder::new();
    folder.ready();

    recover(&folder.original());

    assert_eq!(fs::read(folder.original()).unwrap(), b"old contents");
    assert_eq!(folder.names(), vec!["Mine.hoplodex"]);
}

#[test]
fn recovery_with_nothing_to_recover_changes_nothing() {
    let folder = Folder::new();
    fs::write(folder.original(), b"contents").unwrap();

    recover(&folder.original());
    recover(&folder.dir.path().join("Missing.hoplodex"));

    assert_eq!(folder.names(), vec!["Mine.hoplodex"]);
}

fn assert_replace_failed(folder: &Folder, result: Result<file_swap::Replaced, CommandError>) {
    let error = result.unwrap_err();
    assert_eq!(error.code, "REPLACE_FAILED");
    assert_eq!(
        error.details.as_deref().unwrap()["path"],
        folder.original().to_string_lossy().as_ref()
    );
    assert_eq!(fs::read(folder.original()).unwrap(), b"old contents", "the original is unchanged");
    assert_eq!(folder.names(), vec!["Mine.hoplodex"], "the new copy is removed");
}

#[test]
fn a_final_rename_that_keeps_failing_leaves_the_original_and_removes_the_copy() {
    let folder = Folder::new();
    folder.ready();
    let _refused = testing::fail_final_rename(Duration::from_millis(200));

    let started = std::time::Instant::now();
    let result = file_swap::replace(&folder.original());

    assert!(started.elapsed() >= Duration::from_millis(200), "it was retried");
    assert_replace_failed(&folder, result);
}

#[test]
fn a_final_rename_that_keeps_failing_without_hard_links_puts_the_original_back() {
    let folder = Folder::new();
    folder.ready();
    let _no_links = testing::disable_hard_links();
    let _refused = testing::fail_final_rename(Duration::from_millis(50));

    let result = file_swap::replace(&folder.original());

    assert_replace_failed(&folder, result);
}

#[test]
fn the_free_space_check_asks_for_the_file_size_plus_five_percent() {
    let folder = Folder::new();
    let _space = disk_space::testing::fake_available_space(|_| Some(1_000));

    assert_eq!(disk_space::room_for_copy(1_000), 1_050);
    assert_eq!(disk_space::room_for_copy(1_001), 1_052, "rounded up");
    let short = disk_space::check_room_for_copy(1_000, folder.dir.path()).unwrap_err();
    assert_eq!(short.bytes_needed, 1_050);
    assert_eq!(short.bytes_available, 1_000);
    assert_eq!(short.path, folder.dir.path());
    let error = CommandError::from(short);
    assert_eq!(error.code, "INSUFFICIENT_SPACE");
    assert!(disk_space::check_room_for_copy(952, folder.dir.path()).is_ok());
}

#[test]
fn the_real_free_space_is_measured() {
    let folder = Folder::new();
    assert!(disk_space::available_space(folder.dir.path()).unwrap() > 0);
    assert!(disk_space::check_room_for_copy(1, folder.dir.path()).is_ok());
}
