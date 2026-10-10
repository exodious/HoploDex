//! Best-effort secure deletion (FR-035, FR-016; research.md §12): overwrite
//! a file's contents, flush, ask the storage to discard the blocks, then
//! unlink it, so a decrypted copy of an attachment, or an old database file
//! or backup, doesn't linger in freed disk blocks. One zero-fill pass is all
//! that can be promised (journaling and copy-on-write filesystems and SSD
//! wear-levelling may keep older blocks), which is why callers treat this
//! as "where the OS supports it".

use std::cell::Cell;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::services::scratch;

/// The chunk small files (decrypted document copies) are overwritten in.
const SMALL_CHUNK: u64 = 64 * 1024;
/// The chunk files over 1 MiB (databases, backups) are overwritten in.
const LARGE_CHUNK: u64 = 1024 * 1024;

/// Opens `path` for the overwrite, or says it must only be unlinked
/// (`None`): something that is not a regular file (a symlink would be
/// written through to its target) or a file with other names (a hard link
/// someone planted would carry the overwrite to its referent, and the
/// overwrite would destroy the other name's contents, #63). The checks are
/// made on the open handle, which does not follow a symlink, so nothing can
/// swap the path in between.
fn open_for_wipe(path: &Path) -> io::Result<Option<File>> {
    if !fs::symlink_metadata(path)?.file_type().is_file() {
        return Ok(None);
    }
    let mut options = OpenOptions::new();
    options.write(true);
    let file = scratch::open_no_follow(&mut options, path)?;
    if !file.metadata()?.file_type().is_file() || scratch::link_count(&file)? > 1 {
        log::warn!("{} has other names: unlinking it without overwriting", path.display());
        return Ok(None);
    }
    Ok(Some(file))
}

/// Overwrites `path` with zeros (same length), flushes that to disk, asks
/// the storage to discard the blocks, and removes it. A failed overwrite
/// doesn't stop the unlink (the file is deleted either way) and only a
/// failure to unlink is returned. A symlink, or a file with more than one
/// hard link, is just unlinked: overwriting would write through to the
/// target, or destroy the other name's contents.
pub fn secure_delete_file(path: &Path) -> io::Result<()> {
    if let Ok(Some(file)) = open_for_wipe(path) {
        let _ = overwrite_with_zeros(&file, &mut WipeControl::default());
    }
    fs::remove_file(path)
}

/// What a whole-file deletion reports to, and asks whether to stop.
#[derive(Default)]
pub struct WipeControl<'a> {
    /// Called after each chunk with the bytes overwritten so far and the
    /// file's length.
    pub progress: Option<&'a mut dyn FnMut(u64, u64)>,
    /// Asked before each chunk: `true` stops the overwrite.
    pub cancel: Option<&'a dyn Fn() -> bool>,
}

/// How a whole-file deletion ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wiped {
    Deleted,
    /// Stopped part way through the overwrite. The file is removed anyway,
    /// so nothing half overwritten is left to look like a backup (FR-037).
    Stopped,
}

/// Securely deletes a database file or backup, reporting progress and
/// stoppable between chunks. Unlike [`secure_delete_file`], a file that
/// can't be opened for the overwrite (read-only, say) is left in place and
/// the error returned, because unlinking it would leave its data unwiped.
pub fn secure_delete_whole_file(path: &Path, mut control: WipeControl) -> io::Result<Wiped> {
    let outcome = match open_for_wipe(path)? {
        Some(file) => overwrite_with_zeros(&file, &mut control)?,
        None => Wiped::Deleted,
    };
    fs::remove_file(path)?;
    Ok(outcome)
}

fn overwrite_with_zeros(file: &File, control: &mut WipeControl) -> io::Result<Wiped> {
    let len = file.metadata()?.len();
    let chunk = if len > LARGE_CHUNK { LARGE_CHUNK } else { SMALL_CHUNK };
    let zeros = vec![0u8; chunk as usize];
    let mut out = file;
    let mut done = 0;
    while done < len {
        if control.cancel.is_some_and(|stop| stop()) {
            let _ = file.sync_all();
            return Ok(Wiped::Stopped);
        }
        let n = (len - done).min(chunk);
        out.write_all(&zeros[..n as usize])?;
        done += n;
        if let Some(progress) = control.progress.as_mut() {
            progress(done, len);
        }
    }
    file.sync_all()?;
    if let Err(err) = discard_blocks(file, len) {
        log::debug!("the storage did not discard a deleted file's blocks: {err}");
    }
    Ok(Wiped::Deleted)
}

/// Asks the storage to deallocate the file's blocks (research.md §12), so
/// on a filesystem mounted with discard, or trimmed periodically, the freed
/// blocks reach an SSD sooner. Best effort: callers ignore a failure.
fn discard_blocks(file: &File, len: u64) -> io::Result<()> {
    if testing::discard_fails() {
        return Err(io::Error::other("discard refused for the test"));
    }
    if len == 0 {
        return Ok(());
    }
    platform_discard(file, len)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn platform_discard(file: &File, len: u64) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    let len = libc::off_t::try_from(len).map_err(io::Error::other)?;
    // SAFETY: the descriptor is open for writing for the duration of the call.
    let rc = unsafe {
        libc::fallocate(
            file.as_raw_fd(),
            libc::FALLOC_FL_PUNCH_HOLE | libc::FALLOC_FL_KEEP_SIZE,
            0,
            len,
        )
    };
    if rc == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}

#[cfg(target_os = "macos")]
fn platform_discard(file: &File, len: u64) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    let hole = libc::fpunchhole_t {
        fp_flags: 0,
        reserved: 0,
        fp_offset: 0,
        fp_length: libc::off_t::try_from(len).map_err(io::Error::other)?,
    };
    // SAFETY: the descriptor is open for writing, and `hole` outlives the call.
    let rc = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_PUNCHHOLE, &hole) };
    if rc == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}

#[cfg(windows)]
fn platform_discard(file: &File, len: u64) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::IO::DeviceIoControl;
    use windows_sys::Win32::System::Ioctl::{FILE_ZERO_DATA_INFORMATION, FSCTL_SET_ZERO_DATA};

    let range = FILE_ZERO_DATA_INFORMATION {
        FileOffset: 0,
        BeyondFinalZero: i64::try_from(len).map_err(io::Error::other)?,
    };
    let mut returned = 0u32;
    // SAFETY: the handle is open for writing, and `range` and `returned`
    // outlive the synchronous call.
    let ok = unsafe {
        DeviceIoControl(
            file.as_raw_handle(),
            FSCTL_SET_ZERO_DATA,
            (&range as *const FILE_ZERO_DATA_INFORMATION).cast(),
            std::mem::size_of::<FILE_ZERO_DATA_INFORMATION>() as u32,
            std::ptr::null_mut(),
            0,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    if ok != 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}

#[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos", windows)))]
fn platform_discard(_file: &File, _len: u64) -> io::Result<()> {
    Err(io::Error::from(io::ErrorKind::Unsupported))
}

/// Securely deletes every file under `dir`, then `dir` itself. Returns the
/// files that could not be deleted (their folders are left in place) so the
/// caller can retry later; a missing `dir` has nothing to clear.
pub fn secure_delete_dir(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut leftovers = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => leftovers.extend(secure_delete_dir(&path)),
            _ => {
                if secure_delete_file(&path).is_err() {
                    leftovers.push(path);
                }
            }
        }
    }
    if leftovers.is_empty() {
        let _ = fs::remove_dir(dir);
    }
    leftovers
}

thread_local! {
    static DISCARD_FAILS: Cell<bool> = const { Cell::new(false) };
}

/// Lets the tests make the discard step fail on this thread.
#[doc(hidden)]
pub mod testing {
    use super::DISCARD_FAILS;

    /// Undoes [`fail_discard`] when dropped.
    pub struct FailDiscardGuard(());

    impl Drop for FailDiscardGuard {
        fn drop(&mut self) {
            DISCARD_FAILS.with(|fails| fails.set(false));
        }
    }

    /// Until the guard is dropped, the storage refuses every discard.
    pub fn fail_discard() -> FailDiscardGuard {
        DISCARD_FAILS.with(|fails| fails.set(true));
        FailDiscardGuard(())
    }

    pub(super) fn discard_fails() -> bool {
        DISCARD_FAILS.with(|fails| fails.get())
    }
}
