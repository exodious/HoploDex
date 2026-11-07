//! The troubleshooting log's files (013 research.md §4): where they are, and
//! how they are created, rotated, aged and deleted. The folder is a parameter,
//! so tests use throwaway ones. Nothing here logs while the logger's lock is
//! held: only [`delete_all`] and [`delete_if_old`] can log, through
//! `secure_delete`, and the logger calls them with its lock released.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::services::{scratch, secure_delete};

/// The folder under the app's cache directory (013 FR-006).
pub const FOLDER: &str = "logs";
pub const CURRENT: &str = "hoplodex.log";
pub const ROTATED: &str = "hoplodex.log.1";
/// A file's cap, so both together never pass 4 MiB (013 FR-007).
pub const MAX_BYTES: u64 = 2 * 1024 * 1024;
/// How long a file lasts from when it was made, so a start's new lines can't
/// keep older ones past what the user is told (013 FR-008).
pub const MAX_AGE: Duration = Duration::from_secs(14 * 24 * 60 * 60);

/// The current log file, open for appending.
pub struct LogFile {
    folder: PathBuf,
    file: File,
}

impl LogFile {
    /// Opens the current file in `folder`, making both if they aren't there.
    /// Anything at the name that isn't a plain file of this user's is refused
    /// and left alone (013 FR-010).
    pub fn open(folder: &Path) -> io::Result<Self> {
        make_folder(folder)?;
        let file = open_current(&folder.join(CURRENT))?;
        Ok(Self { folder: folder.to_owned(), file })
    }

    /// One write per line, so a crash loses nothing already logged and two
    /// copies of the app don't split each other's lines. Rotates first if the
    /// line would take the open file past the cap, by the file's own length, so
    /// another copy's lines count too (013 FR-007).
    pub fn write_line(&mut self, line: &str) -> io::Result<()> {
        if self.file.metadata()?.len() + line.len() as u64 > MAX_BYTES {
            self.rotate()?;
        }
        self.file.write_all(line.as_bytes())
    }

    /// Renaming replaces whatever is at the rotated name, a symlink itself and
    /// not what it points at. A current file that has gone, deleted by the user
    /// or rotated by another copy, leaves nothing to rename. Any other failure
    /// stops this file rather than let it pass the cap.
    fn rotate(&mut self) -> io::Result<()> {
        match fs::rename(self.folder.join(CURRENT), self.folder.join(ROTATED)) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => return Err(err),
            _ => {}
        }
        self.file = open_current(&self.folder.join(CURRENT))?;
        Ok(())
    }
}

/// Only this user can list the folder on Unix (013 FR-010).
fn make_folder(folder: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(folder)
}

/// Exclusive, so a name that is already there is never followed: another copy's
/// file is appended to, and anything else is refused.
fn open_current(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.append(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(path) {
        Ok(file) => {
            // NTFS can give a file made soon after its name was freed the old file's
            // creation time, which would age it out at every start (013 FR-008).
            #[cfg(windows)]
            {
                use std::os::windows::fs::FileTimesExt;
                let _ = file.set_times(fs::FileTimes::new().set_created(SystemTime::now()));
            }
            return Ok(file);
        }
        Err(err) if err.kind() != io::ErrorKind::AlreadyExists => return Err(err),
        Err(_) => {}
    }
    if !scratch::is_plain_file(path) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a plain file of ours"));
    }
    let mut options = OpenOptions::new();
    options.append(true);
    scratch::open_no_follow(&mut options, path)
}

fn both(folder: &Path) -> [PathBuf; 2] {
    [folder.join(CURRENT), folder.join(ROTATED)]
}

/// Deletes both files, leaving anything that isn't a plain file of ours
/// (013 FR-009, FR-012).
pub fn delete_all(folder: &Path) {
    for path in both(folder) {
        if scratch::is_plain_file(&path)
            && let Err(err) = secure_delete::secure_delete_file(&path)
        {
            log::warn!(
                "could not delete a troubleshooting log in {}: {err}",
                crate::logging::place(folder)
            );
        }
    }
}

/// Deletes each file made longer than [`MAX_AGE`] before `now`, or last changed
/// that long ago where the file system doesn't record when it was made
/// (013 FR-008).
pub fn delete_if_old(folder: &Path, now: SystemTime) {
    for path in both(folder) {
        let old = fs::metadata(&path)
            .and_then(|meta| meta.created().or_else(|_| meta.modified()))
            .is_ok_and(|made| now.duration_since(made).is_ok_and(|age| age > MAX_AGE));
        if old
            && scratch::is_plain_file(&path)
            && let Err(err) = secure_delete::secure_delete_file(&path)
        {
            log::warn!(
                "could not delete an old troubleshooting log in {}: {err}",
                crate::logging::place(folder)
            );
        }
    }
}
