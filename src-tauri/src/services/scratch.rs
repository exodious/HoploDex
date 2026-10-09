//! Scratch files and who owns them (#63). A scratch file is one this
//! operation made beside a database or a backup: the copy a passphrase
//! change or a restore builds, a backup's `.partial`, `machine.json`'s
//! temporary file. Some names must stay predictable so a crash can be
//! recovered, and a predictable name proves nothing about who put a file
//! there, so:
//! - a scratch file is only ever created exclusively ([`create`]), never by
//!   truncating whatever is at the name, and a collision is an error that
//!   leaves the existing object alone;
//! - an operation removes only what it created, so its cleanup starts after
//!   its creation succeeded;
//! - recovery and cleanup act on an object only if [`is_plain_file`] says it
//!   is a regular file with one name, and on Unix one this user owns.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::Path;

/// Creates `path` for writing, failing with `AlreadyExists` when anything is
/// there, a symlink included (which is not followed). The caller owns the
/// file from here, and only then may remove it.
pub fn create(path: &Path) -> io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

/// How many names (hard links) the open file has.
pub fn link_count(file: &File) -> io::Result<u64> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(file.metadata()?.nlink())
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
        };
        // SAFETY: the handle is open for the duration of the call, and the
        // structure is plain data that the call fills.
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        let ok = unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(u64::from(info.nNumberOfLinks))
    }
}

/// Opens `path` without following a symlink (a reparse point on Windows):
/// the handle is the link itself, or the link is refused.
pub fn open_no_follow(options: &mut OpenOptions, path: &Path) -> io::Result<File> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_FLAG_OPEN_REPARSE_POINT
        options.custom_flags(0x0020_0000);
    }
    options.open(path)
}

/// Whether `path` is a plain file this process may recover or remove on the
/// strength of its name alone: a regular file (not a symlink, a folder or
/// anything else, none of which is followed or entered), with no other name
/// (a planted hard link would otherwise carry our overwrite to its
/// referent), and on Unix owned by the user running the app. Anything else
/// is left alone and the caller says so in the log.
pub fn is_plain_file(path: &Path) -> bool {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return false;
    };
    if !meta.file_type().is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // SAFETY: `geteuid` has no preconditions and cannot fail.
        meta.nlink() == 1 && meta.uid() == unsafe { libc::geteuid() }
    }
    #[cfg(windows)]
    {
        let mut options = OpenOptions::new();
        options.read(true);
        open_no_follow(&mut options, path).is_ok_and(|file| {
            file.metadata().is_ok_and(|meta| meta.file_type().is_file())
                && link_count(&file).is_ok_and(|links| links == 1)
        })
    }
}
