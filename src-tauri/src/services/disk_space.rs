//! Free space before a copy (FR-016, research.md §4): a copy of a database
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
/// (research.md §8). The first volume short of room is reported.
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
            Err(err) => log::warn!("could not measure the free space in {}: {err}", dir.display()),
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
