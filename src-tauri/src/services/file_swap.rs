//! Replacing a database file in one step, for a passphrase change and a
//! restore (research.md §4; contracts/database-file.md "Replacing the
//! file"). The connection is closed first: Windows cannot replace an open
//! file. The verified copy waits at `.<file>.new` in the same folder, and the
//! previous contents at `.<file>.old` until they are securely deleted. The
//! two fixed names are the journal: [`finish_interrupted`], called before every
//! open of a path, finishes or undoes a replacement a crash interrupted, and
//! [`remove_leftovers`], after it opened, tidies what is left. Neither acts on
//! anything but a plain file of ours (#63; services/scratch.rs).

use std::cell::Cell;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use crate::commands::CommandError;
use crate::services::scratch;
use crate::services::secure_delete::{self, WipeControl};

/// How long a refused final rename is retried: a sync client or antivirus
/// may hold the file for a moment on Windows.
const RENAME_RETRY_FOR: Duration = Duration::from_secs(2);
const RENAME_RETRY_EVERY: Duration = Duration::from_millis(100);

/// What a finished replacement did with the previous contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replaced {
    /// The previous file was securely deleted (US4-5).
    pub old_removed: bool,
    /// Where it still is when it wasn't.
    pub old_path: PathBuf,
}

fn beside(original: &Path, suffix: &str) -> PathBuf {
    let name = original.file_name().unwrap_or_default().to_string_lossy();
    original.with_file_name(format!(".{name}.{suffix}"))
}

/// Where the verified copy waits: `.<file>.new` beside the original.
pub fn new_path(original: &Path) -> PathBuf {
    beside(original, "new")
}

/// Where the previous contents wait for secure deletion: `.<file>.old`.
pub fn old_path(original: &Path) -> PathBuf {
    beside(original, "old")
}

fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

/// Replaces `original` with the verified copy at [`new_path`], then securely
/// deletes the previous contents. The original's data is kept under a second
/// name (a hard link) while the new copy is renamed over it, which replaces
/// the directory entry atomically, so the old blocks can then be overwritten
/// rather than freed unwiped. Without hard links (FAT, some network shares)
/// it takes two renames, and [`recover`] covers the gap between them.
///
/// A final rename still refused after about two seconds gives
/// `REPLACE_FAILED`: the copy is removed and the original is unchanged.
pub fn replace(original: &Path) -> Result<Replaced, CommandError> {
    let new = new_path(original);
    let old = old_path(original);
    if !scratch::is_plain_file(&new) {
        // Not the copy this operation made (it was swapped for a link, say):
        // never move it into the database's place, and never touch it.
        log::error!("{} is not a plain file of ours: not replacing", new.display());
        return Err(CommandError::replace_failed(original));
    }
    remove_old(original, &old);
    let linked = !testing::hard_links_disabled() && fs::hard_link(original, &old).is_ok();
    if !linked && let Err(err) = fs::rename(original, &old) {
        log::error!("could not move {} aside: {err}", original.display());
        let _ = secure_delete::secure_delete_file(&new);
        return Err(CommandError::replace_failed(original));
    }
    if let Err(err) = rename_with_retry(&new, original) {
        log::error!("could not replace {}: {err}", original.display());
        let _ = secure_delete::secure_delete_file(&new);
        let put_back = if linked { fs::remove_file(&old) } else { fs::rename(&old, original) };
        if let Err(err) = put_back {
            log::error!("could not tidy up after replacing {}: {err}", original.display());
        }
        return Err(CommandError::replace_failed(original));
    }
    sync_folder(original);
    let old_removed = !testing::old_copy_kept()
        && secure_delete::secure_delete_whole_file(&old, WipeControl::default()).is_ok();
    Ok(Replaced { old_removed, old_path: old })
}

fn rename_with_retry(from: &Path, to: &Path) -> io::Result<()> {
    let retry_for = testing::final_rename_failing_for().unwrap_or(RENAME_RETRY_FOR);
    let started = Instant::now();
    loop {
        let attempt = if testing::final_rename_failing_for().is_some() {
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "refused for the test"))
        } else {
            fs::rename(from, to)
        };
        match attempt {
            Ok(()) => return Ok(()),
            Err(err) if started.elapsed() >= retry_for => return Err(err),
            Err(_) => thread::sleep(RENAME_RETRY_EVERY.min(retry_for)),
        }
    }
}

/// Flushes the rename to disk, where the OS lets a folder be flushed.
fn sync_folder(path: &Path) {
    #[cfg(unix)]
    if let Some(folder) = path.parent()
        && let Err(err) = fs::File::open(folder).and_then(|dir| dir.sync_all())
    {
        log::warn!("could not flush {}: {err}", folder.display());
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// Whether two names are the same file (hard links), without opening
/// either on Unix.
fn same_file(a: &Path, b: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        match (fs::metadata(a), fs::metadata(b)) {
            (Ok(a), Ok(b)) => (a.dev(), a.ino()) == (b.dev(), b.ino()),
            _ => false,
        }
    }
    #[cfg(windows)]
    {
        same_file::is_same_file(a, b).unwrap_or(false)
    }
}

/// Removes a leftover `.old`: a second name for the original is only
/// unlinked, since overwriting it would wipe the original; anything else is
/// previous contents, deleted securely, but only a plain file of ours (see
/// [`scratch::is_plain_file`]): a symlink, a folder or a file with other
/// names at that name is left alone.
fn remove_old(original: &Path, old: &Path) {
    if !exists(old) {
        return;
    }
    let removed = if is_regular(old) && same_file(original, old) {
        fs::remove_file(old)
    } else if scratch::is_plain_file(old) {
        secure_delete::secure_delete_whole_file(old, WipeControl::default()).map(|_| ())
    } else {
        log::warn!("{} is not a plain file of ours: leaving it alone", old.display());
        return;
    };
    if let Err(err) = removed {
        log::warn!("could not remove {}: {err}", old.display());
    }
}

fn is_regular(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_file())
}

/// Finishes or undoes a replacement of `path` that a crash interrupted
/// between its two renames, before `path` is opened (research.md §4), when
/// the database is missing:
/// - `.new` present: it is renamed into place, completing the swap;
/// - only `.old` present: it is renamed back.
///
/// Only a plain file of ours is renamed (#63); a symlink, a folder or a file
/// with other names at those names is left alone and the open that follows
/// finds no database. Everything that merely tidies up waits for
/// [`remove_leftovers`], after the passphrase has opened the database.
pub fn finish_interrupted(path: &Path) {
    if exists(path) {
        return;
    }
    let new = new_path(path);
    let old = old_path(path);
    let (from, what) = if scratch::is_plain_file(&new) {
        (new, "completing")
    } else if scratch::is_plain_file(&old) {
        (old, "undoing")
    } else {
        for leftover in [&new, &old] {
            if exists(leftover) {
                log::warn!("{} is not a plain file of ours: leaving it alone", leftover.display());
            }
        }
        return;
    };
    log::warn!("{what} an interrupted replacement of {}", path.display());
    if let Err(err) = fs::rename(&from, path) {
        log::error!("could not recover {}: {err}", path.display());
    }
}

/// Removes what a finished or abandoned replacement left beside `path`, once
/// the database has opened with its passphrase (so a wrong passphrase, or
/// anyone who can only reach the folder, can't make the app delete anything):
/// - `.old` present: the swap finished, or never got to its rename, and
///   `.old` goes (unlinked when it is the database under a second name);
/// - `.new` present: the swap never happened, and the copy goes.
///
/// Both only when they are plain files of ours (#63).
pub fn remove_leftovers(path: &Path) {
    if !exists(path) {
        return;
    }
    remove_old(path, &old_path(path));
    let new = new_path(path);
    if !exists(&new) {
        return;
    }
    if !scratch::is_plain_file(&new) {
        log::warn!("{} is not a plain file of ours: leaving it alone", new.display());
    } else if let Err(err) = secure_delete::secure_delete_file(&new) {
        log::warn!("could not remove {}: {err}", new.display());
    }
}

thread_local! {
    static NO_HARD_LINKS: Cell<bool> = const { Cell::new(false) };
    static KEEP_OLD_COPY: Cell<bool> = const { Cell::new(false) };
    static FAIL_FINAL_RENAME: Cell<Option<Duration>> = const { Cell::new(None) };
}

/// Lets the tests take the fallback path, refuse the final rename, or keep
/// the previous contents, on this thread.
#[doc(hidden)]
pub mod testing {
    use super::*;

    /// Undoes one of the hooks below when dropped.
    pub struct HookGuard(fn());

    impl Drop for HookGuard {
        fn drop(&mut self) {
            (self.0)();
        }
    }

    /// Until dropped, hard links are "not supported", as on FAT.
    pub fn disable_hard_links() -> HookGuard {
        NO_HARD_LINKS.with(|hook| hook.set(true));
        HookGuard(|| NO_HARD_LINKS.with(|hook| hook.set(false)))
    }

    /// Until dropped, the final rename is refused, and retried for `for_`
    /// before giving up.
    pub fn fail_final_rename(for_: Duration) -> HookGuard {
        FAIL_FINAL_RENAME.with(|hook| hook.set(Some(for_)));
        HookGuard(|| FAIL_FINAL_RENAME.with(|hook| hook.set(None)))
    }

    /// Until dropped, the previous contents can't be deleted.
    pub fn keep_old_copy() -> HookGuard {
        KEEP_OLD_COPY.with(|hook| hook.set(true));
        HookGuard(|| KEEP_OLD_COPY.with(|hook| hook.set(false)))
    }

    pub(super) fn hard_links_disabled() -> bool {
        NO_HARD_LINKS.with(|hook| hook.get())
    }

    pub(super) fn final_rename_failing_for() -> Option<Duration> {
        FAIL_FINAL_RENAME.with(|hook| hook.get())
    }

    pub(super) fn old_copy_kept() -> bool {
        KEEP_OLD_COPY.with(|hook| hook.get())
    }
}
