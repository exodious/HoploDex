//! Best-effort secure deletion (FR-035): overwrite a file's contents, then
//! unlink it, so a decrypted copy of an attachment doesn't linger in freed
//! disk blocks. One zero-fill pass is all that can be promised — journaling
//! and copy-on-write filesystems and SSD wear-levelling may keep older
//! blocks — which is why callers treat this as "where the OS supports it".

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const CHUNK: usize = 64 * 1024;

/// Overwrites `path` with zeros (same length), flushes that to disk, and
/// removes it. A failed overwrite doesn't stop the unlink — the file is
/// deleted either way — and only a failure to unlink is returned. A symlink
/// is just unlinked: overwriting would write through it to its target.
pub fn secure_delete_file(path: &Path) -> io::Result<()> {
    let is_symlink = fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink());
    if !is_symlink {
        let _ = overwrite_with_zeros(path);
    }
    fs::remove_file(path)
}

fn overwrite_with_zeros(path: &Path) -> io::Result<()> {
    let mut remaining = fs::metadata(path)?.len();
    let mut file = OpenOptions::new().write(true).open(path)?;
    let zeros = [0u8; CHUNK];
    while remaining > 0 {
        let len = remaining.min(CHUNK as u64) as usize;
        file.write_all(&zeros[..len])?;
        remaining -= len as u64;
    }
    file.sync_all()
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
