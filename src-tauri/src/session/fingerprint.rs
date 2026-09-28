//! Noticing that another computer took the database over (FR-032, research.md
//! §6). On a cloud-synced folder another computer's changes arrive as the
//! sync client replacing the file at the path, and our open file descriptor
//! keeps pointing at the old one, so the connection itself never sees it.
//! The session instead records the file's fingerprint after each of its own
//! commits, and compares it before the next write and at close.

use std::fs;
use std::io;
use std::path::Path;
use std::time::SystemTime;

/// Which file is at the path, whatever its name.
///
/// On Unix it comes from `stat`, never from opening the file: closing any
/// descriptor on a file drops every POSIX lock this process holds on it,
/// including SQLite's exclusive lock, so a check must not open and close
/// one.
#[cfg(unix)]
type FileIdentity = (u64, u64);

/// On Windows, the volume serial number and file index, through a handle.
/// Windows locks belong to the handle that took them, so opening and
/// closing another one leaves SQLite's lock alone.
#[cfg(windows)]
type FileIdentity = same_file::Handle;

#[cfg(unix)]
fn identity(_path: &Path, meta: &fs::Metadata) -> io::Result<FileIdentity> {
    use std::os::unix::fs::MetadataExt;
    Ok((meta.dev(), meta.ino()))
}

#[cfg(windows)]
fn identity(path: &Path, _meta: &fs::Metadata) -> io::Result<FileIdentity> {
    same_file::Handle::from_path(path)
}

/// The file at a path, as the session last wrote it: its identity, length
/// and modification time.
#[derive(Debug, PartialEq, Eq)]
pub struct FileFingerprint {
    identity: FileIdentity,
    len: u64,
    modified: Option<SystemTime>,
}

/// What [`FileFingerprint::check`] found at the path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FingerprintCheck {
    /// The file is as the session left it.
    Same,
    /// A different file is at the path, or the file was changed by someone
    /// else: another computer took the database over.
    Replaced,
    /// The path is missing or can't be examined: the drive or network share
    /// has gone. This is not a take-over (research.md §6).
    Unreachable,
}

impl FileFingerprint {
    /// The fingerprint of the file at `path` now. Fails when the path is
    /// missing or can't be examined.
    pub fn capture(path: &Path) -> io::Result<Self> {
        let meta = fs::metadata(path)?;
        Ok(Self {
            identity: identity(path, &meta)?,
            len: meta.len(),
            modified: meta.modified().ok(),
        })
    }

    /// Compares the file at `path` now with this fingerprint. One `stat`
    /// (plus a handle on Windows).
    pub fn check(&self, path: &Path) -> FingerprintCheck {
        match Self::capture(path) {
            Ok(now) if now == *self => FingerprintCheck::Same,
            Ok(_) => FingerprintCheck::Replaced,
            Err(_) => FingerprintCheck::Unreachable,
        }
    }
}
