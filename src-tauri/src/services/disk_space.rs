//! Free space before a copy (003 FR-016, research.md §4): a copy of a database
//! file is only started when its folder has room for the file plus 5%, so
//! it does not fill the disk halfway through.

use std::cell::RefCell;
use std::io;
use std::path::{Path, PathBuf};

use crate::commands::CommandError;

/// A folder without room for a copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InsufficientSpace {
    pub bytes_needed: u64,
    pub bytes_available: u64,
    pub path: PathBuf,
}

impl From<InsufficientSpace> for CommandError {
    fn from(short: InsufficientSpace) -> Self {
        CommandError::insufficient_space(short.bytes_needed, short.bytes_available, &short.path)
    }
}

/// Whether `err` is the OS saying the disk is full or the user's quota is
/// used up: `ENOSPC` and `EDQUOT` on Linux and macOS, `ERROR_DISK_FULL` (112),
/// `ERROR_HANDLE_DISK_FULL` (39) and `ERROR_DISK_QUOTA_EXCEEDED` on Windows
/// (issue #110). The pre-check above can't see a disk that fills while a
/// file is being written, nor a quota.
pub fn is_disk_full(err: &io::Error) -> bool {
    matches!(err.kind(), io::ErrorKind::StorageFull | io::ErrorKind::QuotaExceeded)
}

/// [`is_disk_full`] for `err` or any error it wraps, for a writer that
/// reports its I/O errors inside its own type.
pub fn chain_is_disk_full(err: &(dyn std::error::Error + 'static)) -> bool {
    let mut current = Some(err);
    while let Some(err) = current {
        if err.downcast_ref::<io::Error>().is_some_and(is_disk_full) {
            return true;
        }
        current = err.source();
    }
    false
}

/// The bytes a copy of a `len`-byte file needs: its size plus 5%, rounded
/// up.
pub fn room_for_copy(len: u64) -> u64 {
    len + (len * 5).div_ceil(100)
}

/// The free space available to this user in the folder `dir`, or where it
/// would be made when it doesn't exist yet.
pub fn available_space(dir: &Path) -> io::Result<u64> {
    if let Some(faked) = testing::faked(dir) {
        return Ok(faked);
    }
    fs4::available_space(existing_ancestor(dir))
}

/// `dir`, or its nearest ancestor that exists: a backup folder is made at
/// the first backup, on the volume of the folder it goes in.
fn existing_ancestor(dir: &Path) -> &Path {
    dir.ancestors().find(|ancestor| ancestor.exists()).unwrap_or(dir)
}

/// Checks `dir` has room for a copy of a `file_len`-byte file. Space that
/// can't be measured is not a reason to refuse: the copy itself then fails
/// if the disk fills.
pub fn check_room_for_copy(file_len: u64, dir: &Path) -> Result<(), InsufficientSpace> {
    check_room(&[(dir, file_len)])
}

/// Checks there is room for several copies at once, each `(folder, file
/// length)`. Copies to folders on the same volume need their room summed
/// (003 research.md §8). The first volume short of room is reported.
pub fn check_room(copies: &[(&Path, u64)]) -> Result<(), InsufficientSpace> {
    let mut volumes: Vec<(Option<Volume>, &Path, u64)> = Vec::new();
    for &(dir, len) in copies {
        let volume = volume_of(dir);
        match volumes.iter_mut().find(|(v, _, _)| volume.is_some() && *v == volume) {
            Some((_, _, needed)) => *needed += room_for_copy(len),
            None => volumes.push((volume, dir, room_for_copy(len))),
        }
    }
    for (_, dir, needed) in volumes {
        match available_space(dir) {
            Ok(available) if available < needed => {
                return Err(InsufficientSpace {
                    bytes_needed: needed,
                    bytes_available: available,
                    path: dir.to_owned(),
                });
            }
            Ok(_) => {}
            Err(err) => log::warn!(
                "could not measure the free space in {}: {err}",
                crate::logging::place(dir)
            ),
        }
    }
    Ok(())
}

#[cfg(unix)]
type Volume = u64;
#[cfg(windows)]
type Volume = std::ffi::OsString;

/// Which volume `dir` is on: its device number on Unix, its drive or share
/// on Windows.
#[cfg(unix)]
fn volume_of(dir: &Path) -> Option<Volume> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(existing_ancestor(dir)).ok().map(|meta| meta.dev())
}

#[cfg(windows)]
fn volume_of(dir: &Path) -> Option<Volume> {
    match dir.components().next()? {
        std::path::Component::Prefix(prefix) => Some(prefix.as_os_str().to_ascii_uppercase()),
        _ => None,
    }
}

/// Whether the folder `dir` may be on another computer: a network share. On
/// Windows a `\\server\share` path or a drive letter mapped to one; on Linux
/// an NFS, SMB, AFS, Ceph, Coda or 9P mount, or any FUSE mount, since that is
/// how gvfs, kio-fuse, sshfs and the Flatpak document portal reach a share;
/// on macOS a volume not marked local. A folder that can't be looked at is
/// not.
pub fn is_on_network(dir: &Path) -> bool {
    // The E2E build's stand-in for a share, which no test machine mounts.
    // Never in a release build (`scripts/check-no-webdriver.mjs`).
    #[cfg(feature = "e2e")]
    if let Some(share) = std::env::var_os("HOPLODEX_E2E_NETWORK_FOLDER")
        && dir.starts_with(share)
    {
        return true;
    }
    on_network(dir)
}

/// Whether a Linux filesystem of type `kind` (`statfs`'s `f_type`) may be on
/// another computer.
#[cfg(target_os = "linux")]
fn network_kind(kind: u32) -> bool {
    const NETWORK: [u32; 10] = [
        0x6969,      // NFS
        0x517B,      // SMB
        0xFF53_4D42, // CIFS
        0xFE53_4D42, // SMB2
        0x5346_414F, // AFS
        0x6B41_4653, // kAFS
        0x00C3_6400, // Ceph
        0x7375_7245, // Coda
        0x0102_1997, // 9P
        0x6573_5546, // FUSE
    ];
    NETWORK.contains(&kind)
}

#[cfg(target_os = "linux")]
fn on_network(dir: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let Ok(path) = std::ffi::CString::new(dir.as_os_str().as_bytes()) else { return false };
    let mut stat = std::mem::MaybeUninit::<libc::statfs>::uninit();
    // SAFETY: `path` is NUL-terminated and `stat` is written by a call that
    // returns 0, and only read then.
    if unsafe { libc::statfs(path.as_ptr(), stat.as_mut_ptr()) } != 0 {
        return false;
    }
    let kind = unsafe { stat.assume_init() }.f_type;
    #[allow(clippy::unnecessary_cast)] // `f_type`'s type differs between targets.
    network_kind(kind as u32)
}

#[cfg(target_os = "macos")]
fn on_network(dir: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let Ok(path) = std::ffi::CString::new(dir.as_os_str().as_bytes()) else { return false };
    let mut stat = std::mem::MaybeUninit::<libc::statfs>::uninit();
    // SAFETY: as on Linux.
    if unsafe { libc::statfs(path.as_ptr(), stat.as_mut_ptr()) } != 0 {
        return false;
    }
    unsafe { stat.assume_init() }.f_flags & libc::MNT_LOCAL as u32 == 0
}

#[cfg(windows)]
fn on_network(dir: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use std::path::{Component, Prefix};
    use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
    const DRIVE_REMOTE: u32 = 4;
    let letter = match dir.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::UNC(..) | Prefix::VerbatimUNC(..) => return true,
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => letter,
            _ => return false,
        },
        _ => return false,
    };
    let root: Vec<u16> = std::ffi::OsString::from(format!("{}:\\", letter as char))
        .encode_wide()
        .chain([0])
        .collect();
    // SAFETY: `root` is a NUL-terminated wide string that outlives the call.
    unsafe { GetDriveTypeW(root.as_ptr()) == DRIVE_REMOTE }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn on_network(_dir: &Path) -> bool {
    false
}

type FakeSpace = Box<dyn Fn(&Path) -> Option<u64>>;

thread_local! {
    static FAKE_SPACE: RefCell<Option<FakeSpace>> = const { RefCell::new(None) };
}

/// Lets the tests set how much free space this thread's copies see, so a
/// full disk can be tested without filling one.
#[doc(hidden)]
pub mod testing {
    use super::*;

    /// Undoes [`fake_available_space`] when dropped.
    pub struct FakeSpaceGuard(());

    impl Drop for FakeSpaceGuard {
        fn drop(&mut self) {
            FAKE_SPACE.with(|fake| fake.borrow_mut().take());
        }
    }

    /// Until the guard is dropped, `space(dir)` is the free space of `dir`
    /// on this thread; `None` measures it for real.
    pub fn fake_available_space(space: impl Fn(&Path) -> Option<u64> + 'static) -> FakeSpaceGuard {
        FAKE_SPACE.with(|fake| *fake.borrow_mut() = Some(Box::new(space)));
        FakeSpaceGuard(())
    }

    pub(super) fn faked(dir: &Path) -> Option<u64> {
        FAKE_SPACE.with(|fake| fake.borrow().as_ref().and_then(|space| space(dir)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_on_this_computers_own_disk_is_not_on_the_network() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_on_network(dir.path()));
        assert!(!is_on_network(&dir.path().join("not made")));
    }

    /// The magic numbers are the kernel's (`include/uapi/linux/magic.h`).
    #[cfg(target_os = "linux")]
    #[test]
    fn a_fuse_or_network_mount_may_be_on_another_computer_and_a_local_disk_is_not() {
        for remote in [0x6573_5546, 0x6969, 0xFF53_4D42] {
            assert!(network_kind(remote), "{remote:#x}");
        }
        for local in [0xEF53, 0x0102_1994, 0x9123_683E, 0x5846_5342, 0x794C_7630] {
            assert!(!network_kind(local), "{local:#x}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn a_share_is_on_the_network() {
        assert!(is_on_network(Path::new(r"\\server\share\Exports")));
        assert!(is_on_network(Path::new(r"\\?\UNC\server\share\Exports")));
    }
}
