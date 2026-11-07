//! The troubleshooting log's file and state, on real folders (013 US1, US3, US5).
//! Each test makes its own [`Output`], so it needs no process-wide logger.

use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use hoplodex_lib::logging::file::{self, CURRENT, LogFile, MAX_AGE, MAX_BYTES, ROTATED};
use hoplodex_lib::logging::{self, Output};
use hoplodex_lib::models::troubleshooting_log::{LogChoice, LogState};
use tempfile::TempDir;

fn age(path: &Path, by: Duration) {
    let file = fs::OpenOptions::new().write(true).open(path).unwrap();
    file.set_modified(SystemTime::now() - by).unwrap();
}

fn output(variable: Option<LogState>) -> (TempDir, std::path::PathBuf, Output) {
    let dir = TempDir::new().unwrap();
    let folder = dir.path().join("cache").join(file::FOLDER);
    (dir, folder, Output::new(None, variable))
}

/// 013 US5-1: the cap holds, the end of the run is kept, one earlier file stays.
#[test]
fn the_file_rotates_at_the_cap_and_keeps_one() {
    let dir = TempDir::new().unwrap();
    let mut log = LogFile::open(dir.path()).unwrap();
    let line = format!("{}\n", "x".repeat(1023));
    for _ in 0..(MAX_BYTES * 5 / 2 / 1024) {
        log.write_line(&line).unwrap();
    }
    log.write_line("the last line\n").unwrap();
    let current = fs::read_to_string(dir.path().join(CURRENT)).unwrap();
    assert!(current.ends_with("the last line\n"));
    assert!(fs::metadata(dir.path().join(CURRENT)).unwrap().len() <= MAX_BYTES);
    assert!(fs::metadata(dir.path().join(ROTATED)).unwrap().len() <= MAX_BYTES);
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2, "at most two files");
}

/// A later run appends, and still counts what is there towards the cap.
#[test]
fn a_later_run_appends_within_the_same_cap() {
    let dir = TempDir::new().unwrap();
    LogFile::open(dir.path()).unwrap().write_line("first run\n").unwrap();
    let mut log = LogFile::open(dir.path()).unwrap();
    log.write_line("second run\n").unwrap();
    assert_eq!(fs::read_to_string(dir.path().join(CURRENT)).unwrap(), "first run\nsecond run\n");
    fs::write(dir.path().join(CURRENT), "x".repeat(MAX_BYTES as usize)).unwrap();
    let mut log = LogFile::open(dir.path()).unwrap();
    log.write_line("over the cap\n").unwrap();
    assert!(dir.path().join(ROTATED).exists());
    assert_eq!(fs::read_to_string(dir.path().join(CURRENT)).unwrap(), "over the cap\n");
}

/// 013 US5-2.
#[test]
fn a_file_older_than_14_days_is_deleted_and_a_newer_one_kept() {
    let dir = TempDir::new().unwrap();
    let (current, rotated) = (dir.path().join(CURRENT), dir.path().join(ROTATED));
    fs::write(&current, "recent").unwrap();
    fs::write(&rotated, "old").unwrap();
    age(&current, MAX_AGE - Duration::from_secs(3600));
    age(&rotated, MAX_AGE + Duration::from_secs(3600));
    file::delete_if_old(dir.path(), SystemTime::now());
    assert!(current.exists());
    assert!(!rotated.exists());
}

/// 013 FR-010: on Unix, only this user can read it.
#[cfg(unix)]
#[test]
fn only_this_user_can_read_the_file() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new().unwrap();
    let folder = dir.path().join("logs");
    LogFile::open(&folder).unwrap();
    assert_eq!(fs::metadata(&folder).unwrap().permissions().mode() & 0o777, 0o700);
    let mode = fs::metadata(folder.join(CURRENT)).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
}

/// 013 US5-5: a planted name isn't followed or changed, and the run has no file.
#[cfg(unix)]
#[test]
fn a_symbolic_link_or_a_hard_link_at_the_name_is_left_alone() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("somebody else's file");
    fs::write(&target, "keep me").unwrap();
    let folder = dir.path().join("logs");
    fs::create_dir(&folder).unwrap();
    let link = folder.join(CURRENT);

    std::os::unix::fs::symlink(&target, &link).unwrap();
    assert!(LogFile::open(&folder).is_err());
    file::delete_all(&folder);
    assert!(fs::symlink_metadata(&link).is_ok(), "a symlink isn't ours to delete");
    assert_eq!(fs::read_to_string(&target).unwrap(), "keep me");

    fs::remove_file(&link).unwrap();
    fs::hard_link(&target, &link).unwrap();
    assert!(LogFile::open(&folder).is_err());
    assert_eq!(fs::read_to_string(&target).unwrap(), "keep me");
}

/// 013 FR-011: no file, no failure, and the state says so.
#[test]
fn a_folder_that_cannot_be_made_leaves_the_app_running_and_says_so() {
    let dir = TempDir::new().unwrap();
    let blocked = dir.path().join("a file, not a folder");
    fs::write(&blocked, "").unwrap();
    let output = Output::new(None, Some(LogState::On));
    output.start(Some(blocked.join(file::FOLDER)), None);
    let status = output.status();
    assert_eq!(status.state, LogState::On);
    assert!(status.failed);
    assert!(!status.exists);
}

/// 013 US1-3, US5-3 and 013 contracts/tauri-commands.md's table.
#[test]
fn the_state_changes_as_the_contract_says_and_saves_only_a_choice() {
    let (_dir, folder, output) = output(Some(LogState::Off));
    output.start(Some(folder.clone()), None);
    assert_eq!(output.status().state, LogState::Off);
    assert!(!output.status().exists);

    assert_eq!(output.set(LogState::Detailed), None, "a raise while off is refused");
    assert_eq!(output.status().state, LogState::Off);

    assert_eq!(output.set(LogState::On), Some(LogChoice::On), "turned on and saved");
    let status = output.status();
    assert_eq!(status.state, LogState::On);
    assert!(status.exists);
    assert_eq!(status.path.as_deref(), folder.join(CURRENT).to_str());

    assert_eq!(output.set(LogState::Detailed), None, "013 US1-1: a raise saves nothing");
    assert_eq!(output.status().state, LogState::Detailed);
    assert_eq!(output.set(LogState::On), None, "and neither does coming back");
    assert_eq!(output.set(LogState::On), None, "nor asking for what it is");

    fs::write(folder.join(ROTATED), "earlier").unwrap();
    assert_eq!(output.set(LogState::Off), Some(LogChoice::Off), "turned off and saved");
    assert_eq!(output.status().state, LogState::Off);
    assert!(!folder.join(CURRENT).exists(), "013 US5-3");
    assert!(!folder.join(ROTATED).exists(), "013 US5-3");
}

fn leftovers(folder: &Path) {
    fs::create_dir_all(folder).unwrap();
    fs::write(folder.join(CURRENT), "from an earlier run\n").unwrap();
    fs::write(folder.join(ROTATED), "from an earlier run\n").unwrap();
}

/// 013 US3-5, FR-012: a saved Off deletes what an earlier run left.
#[test]
fn a_start_with_a_saved_off_deletes_the_files() {
    let (_dir, folder, output) = output(None);
    leftovers(&folder);
    let decided = output.start(Some(folder.clone()), Some(LogChoice::Off));
    assert_eq!(decided.state, LogState::Off);
    assert!(!folder.join(CURRENT).exists());
    assert!(!folder.join(ROTATED).exists());
}

/// 013 US4-2: the variable turns one run off and leaves the files.
#[test]
fn the_variable_turning_a_run_off_leaves_the_files_alone() {
    let (_dir, folder, output) = output(Some(LogState::Off));
    leftovers(&folder);
    let decided = output.start(Some(folder.clone()), Some(LogChoice::On));
    assert_eq!(decided.state, LogState::Off);
    assert_eq!(fs::read_to_string(folder.join(CURRENT)).unwrap(), "from an earlier run\n");
}

/// 013 US4-3: the variable turns one run on over a saved Off.
#[test]
fn the_variable_turning_a_run_on_over_a_saved_off_writes_for_that_run() {
    let (_dir, folder, output) = output(Some(LogState::On));
    let decided = output.start(Some(folder.clone()), Some(LogChoice::Off));
    assert_eq!(decided.state, LogState::On);
    assert!(!decided.notice);
    assert!(fs::read_to_string(folder.join(CURRENT)).unwrap().contains("--- HoploDex "));
}

/// 013 FR-018: each run begins with its header; 013 US3-1's notice follows only
/// the version's default, which this build's version is.
#[test]
fn a_run_following_the_default_is_told_and_starts_with_a_header() {
    let (_dir, folder, output) = output(None);
    let decided = output.start(Some(folder.clone()), None);
    assert_eq!(decided.notice, logging::default_on(env!("CARGO_PKG_VERSION")));
    if decided.state != LogState::Off {
        let text = fs::read_to_string(folder.join(CURRENT)).unwrap();
        assert!(text.starts_with(&format!("--- HoploDex {} on ", env!("CARGO_PKG_VERSION"))));
    }
}
