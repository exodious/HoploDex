//! Automatic backups (003 FR-024–FR-029; 003 research.md §7, §9; 003 data-model.md
//! "Backup files"): when one is due, where it goes, what it is called, how
//! it is copied and stamped, and how many are kept. The close in
//! `session::lifecycle` and the restore in `commands::backups` make them
//! through [`make_backup`]. [`move_backups`] takes them to a new location
//! when it changes (003 FR-026, research.md §22).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, Utc};
use rusqlite::Connection;

use crate::db::random_hex;
use crate::db::raw_file::RawFile;
use crate::db::upgrade::reach;
use crate::models::database::{
    BackupFailureReason, BackupInfo, ChooserNotice, DATABASE_EXTENSION, LeftBehindReason,
};
use crate::services::disk_space::{self, InsufficientSpace};
use crate::services::machine_settings::{MachineSettings, UnfinishedBackup, UnfinishedBackupMove};
use crate::services::scratch;
use crate::services::secure_delete::{self, WipeControl};

/// The default backup folder's name, in the database's own folder.
pub const DEFAULT_FOLDER_NAME: &str = "HoploDex backups";
/// What a backup being written ends in, so no listing or open dialog takes
/// it for a backup.
const PARTIAL_SUFFIX: &str = ".partial";
/// Copies go in chunks of this size, stoppable between them.
const COPY_CHUNK: u64 = 1024 * 1024;
/// A deliberately slow copy speed, allowing for hard disks and network
/// folders, to decide whether the progress bar shows at once (003 research.md
/// §7). The development machine copies at about 190 MiB/s.
const ASSUMED_BYTES_PER_SECOND: u64 = 50 * 1024 * 1024;

/// The folder a database's backups go to on this computer (003 research.md
/// §7): `default` is a "HoploDex backups" folder next to the database, and
/// anything else is the absolute path the user chose.
pub fn resolve_folder(database_path: &Path, location: &str) -> PathBuf {
    if location == "default" {
        database_path.parent().unwrap_or(Path::new("")).join(DEFAULT_FOLDER_NAME)
    } else {
        PathBuf::from(location)
    }
}

/// `now` as the database stores times: UTC ISO-8601 to the second.
pub fn utc_text(now: &DateTime<FixedOffset>) -> String {
    now.with_timezone(&Utc).format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Whether a close should make a backup (003 FR-025).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Due {
    Yes,
    /// Automatic backups are turned off.
    Off,
    /// Nothing has changed since the last backup.
    NothingChanged,
    /// There was a backup today already, in local time, and it is still
    /// at the backup location.
    AlreadyToday,
}

/// Backups on, changes waiting, and no backup yet today on this computer's
/// calendar: `last_backup_at` (UTC) is compared by its local date with
/// `now`'s. A record saying today is not enough on its own: when
/// `kept_from(today)` finds none of today's backups left at the location
/// (all deleted, 003 FR-029, or left behind at an old location, 003 FR-026), the
/// changes would otherwise be in no backup at all, so one is due. It is
/// asked only when the record says today.
pub fn is_due(
    enabled: bool,
    changes_waiting: bool,
    last_backup_at: Option<&str>,
    now: &DateTime<FixedOffset>,
    kept_from: impl FnOnce(NaiveDate) -> bool,
) -> Due {
    if !enabled {
        return Due::Off;
    }
    if !changes_waiting {
        return Due::NothingChanged;
    }
    let today = now.date_naive();
    let last_local_date = last_backup_at
        .and_then(|last| DateTime::parse_from_rfc3339(last).ok())
        .map(|last| last.with_timezone(now.offset()).date_naive());
    if last_local_date == Some(today) && kept_from(today) { Due::AlreadyToday } else { Due::Yes }
}

/// Whether `folder` holds a regular backup of the database `database_id`
/// made on `date`, by the local date in its name. A backup made before an
/// upgrade is not one: it is never the day's backup (009 research.md §6). A folder that can't be read might
/// still hold one, so it counts as holding one: the once-a-day limit then
/// stands, as it did before the folder became unreadable.
pub fn has_backup_made_on(folder: &Path, database_id: &str, date: NaiveDate) -> bool {
    match list(folder, database_id) {
        Ok(listed) => {
            listed.iter().filter(|backup| backup.before_upgrade_to.is_none()).any(|backup| {
                NaiveDateTime::parse_from_str(&backup.made_at, "%Y-%m-%dT%H:%M:%S")
                    .is_ok_and(|made_at| made_at.date() == date)
            })
        }
        Err(err) => {
            log::warn!("could not list the backups in {}: {err}", folder.display());
            true
        }
    }
}

/// What a backup is for (009 research.md §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupKind<'a> {
    /// A close's, or the one before a restore: subject to the settings.
    Regular,
    /// Made before the database is upgraded to `to_version` (009 FR-011).
    /// It is made whether automatic backups are on or off and whatever the
    /// once-a-day limit says. Its name ends in ` before <to_version>`, its
    /// stamp also says so, its unfinished record is swept without a chooser
    /// notice, and it is never the day's backup. It is for the caller to
    /// leave `changes_waiting` and `last_backup_at` alone, as it does.
    BeforeUpgrade { to_version: &'a str },
}

impl BackupKind<'_> {
    /// The version a backup of this kind was made before upgrading to.
    pub fn before_upgrade_to(&self) -> Option<&str> {
        match self {
            Self::Regular => None,
            Self::BeforeUpgrade { to_version } => Some(to_version),
        }
    }

    fn is_before_upgrade(&self) -> bool {
        matches!(self, Self::BeforeUpgrade { .. })
    }
}

/// `<name> <YYYY-MM-DD HHMMSS> <id8>.hoplodex`, in local time (003 research.md
/// §7), with ` before <version>` after the id for a backup made before an
/// upgrade (009 research.md §6). The first 8 hex digits of the database's id
/// say whose backup it is.
pub fn backup_file_name(
    name: &str,
    now: &DateTime<FixedOffset>,
    database_id: &str,
    kind: BackupKind,
) -> String {
    let before = kind.before_upgrade_to().map(|v| format!(" before {v}")).unwrap_or_default();
    format!(
        "{name} {} {}{before}.{DATABASE_EXTENSION}",
        now.format("%Y-%m-%d %H%M%S"),
        id8(database_id)
    )
}

fn id8(database_id: &str) -> &str {
    database_id.get(..8).unwrap_or(database_id)
}

/// Whether `text` is a version a backup's name may end in:
/// `[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?` (009 data-model.md "Backup files").
pub fn is_backup_version(text: &str) -> bool {
    let (core, suffix) = match text.find(['-', '+']) {
        Some(at) => (&text[..at], Some(&text[at + 1..])),
        None => (text, None),
    };
    let numeric = |part: Option<&str>| {
        part.is_some_and(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
    };
    let mut numbers = core.split('.');
    numeric(numbers.next())
        && numeric(numbers.next())
        && numeric(numbers.next())
        && numbers.next().is_none()
        && suffix.is_none_or(|suffix| {
            !suffix.is_empty()
                && suffix.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        })
}

/// When a file named like a backup of the database `id8` was made, by its
/// name: a name, the local date and time, then the id, and for a backup made
/// before an upgrade ` before <version>` after that, which is returned too.
/// Anything else is not one of its backups. The suffix is removed first, but
/// only when what follows ` before ` is a version, and an ordinary backup's
/// stem ends in its id, so no ordinary name is read as having the suffix,
/// whatever the database is called (009 research.md §6).
fn made_at_by_name(file_name: &str, id8: &str) -> Option<(NaiveDateTime, Option<String>)> {
    let stem = file_name.strip_suffix(&format!(".{DATABASE_EXTENSION}"))?;
    let (stem, before_upgrade_to) = match stem.rsplit_once(" before ") {
        Some((rest, version)) if is_backup_version(version) => (rest, Some(version.to_owned())),
        _ => (stem, None),
    };
    let mut parts = stem.rsplitn(4, ' ');
    let (id, time, date, name) = (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    let digits = |text: &str| text.bytes().all(|b| b.is_ascii_digit() || b == b'-');
    if id != id8 || name.is_empty() || date.len() != 10 || time.len() != 6 {
        return None;
    }
    if !digits(date) || !digits(time) {
        return None;
    }
    let made_at =
        NaiveDateTime::parse_from_str(&format!("{date} {time}"), "%Y-%m-%d %H%M%S").ok()?;
    Some((made_at, before_upgrade_to))
}

/// The backups of the database `database_id` in `folder`, newest first
/// (003 FR-028). They are chosen by the id and the timestamp in their names,
/// never by the name alone, so a folder shared by two databases, or a
/// database renamed in the file manager, never mixes up whose are whose.
/// Files being written (`.partial`) are never listed. A missing folder has
/// none.
pub fn list(folder: &Path, database_id: &str) -> io::Result<Vec<BackupInfo>> {
    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err),
    };
    let id8 = id8(database_id);
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let Some((made_at, before_upgrade_to)) = made_at_by_name(&file_name, id8) else {
            continue;
        };
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        found.push((
            made_at,
            BackupInfo {
                path: entry.path().to_string_lossy().into_owned(),
                file_name,
                made_at: made_at.format("%Y-%m-%dT%H:%M:%S").to_string(),
                size_bytes: meta.len(),
                before_upgrade_to,
            },
        ));
    }
    found
        .sort_by(|(a, a_info), (b, b_info)| b.cmp(a).then(b_info.file_name.cmp(&a_info.file_name)));
    Ok(found.into_iter().map(|(_, info)| info).collect())
}

/// Seconds a copy of `len` bytes is expected to take at the assumed speed.
pub fn estimate_seconds(len: u64) -> f64 {
    len as f64 / ASSUMED_BYTES_PER_SECOND as f64
}

/// A copy expected to take more than a second shows its progress bar at
/// once; a shorter one only if it is still running a second later (003 SC-005).
pub fn shows_progress_at_once(len: u64) -> bool {
    estimate_seconds(len) > 1.0
}

/// A file read at offsets: the open database through SQLite's own handle,
/// or a backup through an ordinary one.
pub trait ReadAt {
    fn size(&self) -> io::Result<u64>;
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()>;
}

impl ReadAt for RawFile<'_> {
    fn size(&self) -> io::Result<u64> {
        RawFile::size(self)
    }

    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        RawFile::read_exact_at(self, buf, offset)
    }
}

impl ReadAt for File {
    fn size(&self) -> io::Result<u64> {
        Ok(self.metadata()?.len())
    }

    #[cfg(unix)]
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        std::os::unix::fs::FileExt::read_exact_at(self, buf, offset)
    }

    #[cfg(windows)]
    fn read_exact_at(&self, mut buf: &mut [u8], mut offset: u64) -> io::Result<()> {
        use std::os::windows::fs::FileExt;
        while !buf.is_empty() {
            match self.seek_read(buf, offset)? {
                0 => return Err(io::ErrorKind::UnexpectedEof.into()),
                n => {
                    buf = &mut buf[n..];
                    offset += n as u64;
                }
            }
        }
        Ok(())
    }
}

/// Why a copy did not finish.
#[derive(Debug)]
pub enum CopyError {
    /// Asked to stop between chunks.
    Stopped,
    Io(io::Error),
}

impl From<io::Error> for CopyError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

/// A failure a test forced at a backup's point is an I/O failure
/// (009 research.md §14).
#[cfg(feature = "upgrade-test-steps")]
impl From<crate::db::upgrade::testing::Injected> for CopyError {
    fn from(err: crate::db::upgrade::testing::Injected) -> Self {
        Self::Io(io::Error::other(err))
    }
}

/// Copies `src` byte for byte to `out`, a new file the caller created
/// exclusively ([`scratch::create`]), in 1 MiB chunks, reporting `(bytes
/// copied, total)` first with nothing copied and then after each chunk, and
/// asking `cancel` before each chunk. The file is the caller's: it removes it
/// when the copy fails, and only because it created it (#63).
pub fn copy_chunked(
    src: &dyn ReadAt,
    mut out: File,
    cancel: &dyn Fn() -> bool,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<(), CopyError> {
    let total = src.size()?;
    progress(0, total);
    let mut buf = vec![0u8; COPY_CHUNK as usize];
    let mut done = 0;
    while done < total {
        if cancel() {
            return Err(CopyError::Stopped);
        }
        let n = (total - done).min(COPY_CHUNK) as usize;
        src.read_exact_at(&mut buf[..n], done)?;
        out.write_all(&buf[..n])?;
        done += n as u64;
        progress(done, total);
    }
    out.sync_all()?;
    Ok(())
}

/// Stamps a fresh byte copy of the open database as a backup (003 research.md
/// §3, §9): attached to the open connection without a key, which makes
/// SQLCipher use the main database's key (the copy shares its salt, so it
/// opens), the copy gets its open marker cleared, its pending changes
/// removed and the backup stamp set, which for a backup made before an
/// upgrade also says the version (009 research.md §6). The copy carries
/// `database_id`, which is the database's own except for a backup opened
/// directly and about to be upgraded: its backup before the upgrade belongs to
/// the database the file becomes (009 research.md §8). Nothing is waiting to
/// be backed up in it: its content is a backup.
pub fn stamp_backup(
    conn: &Connection,
    partial: &Path,
    database_id: &str,
    name: &str,
    now: &DateTime<FixedOffset>,
    kind: BackupKind,
) -> rusqlite::Result<()> {
    conn.execute("ATTACH DATABASE ?1 AS backup", [partial.to_string_lossy()])?;
    let stamped = conn.execute_batch("BEGIN").and_then(|()| {
        conn.execute(
            "UPDATE backup.app_state
             SET open_machine_id = NULL, open_machine_name = NULL, open_since = NULL,
                 changes_waiting = 0, backup_made_at = ?1, backup_of_name = ?2,
                 backup_before_upgrade_to = ?3, database_id = ?4",
            rusqlite::params![utc_text(now), name, kind.before_upgrade_to(), database_id],
        )?;
        conn.execute("DELETE FROM backup.pending_changes", [])?;
        conn.execute_batch("COMMIT")
    });
    if stamped.is_err() {
        let _ = conn.execute_batch("ROLLBACK");
    }
    let detached = conn.execute("DETACH DATABASE backup", []).map(|_| ());
    remove_journal(partial);
    stamped.and(detached)
}

/// Exclusive locking mode keeps a rollback journal beside a file it wrote
/// until the file is closed; a detached copy's is not needed.
fn remove_journal(path: &Path) {
    let mut journal = path.as_os_str().to_owned();
    journal.push("-journal");
    let _ = fs::remove_file(journal);
}

/// Flushes a finished copy to disk and renames it to its final name, which
/// must not be taken: it fails with `AlreadyExists`, leaving what is there
/// and the partial file, and never replaces anything (009 research.md §17).
/// Opened for writing because Windows flushes a file only through a handle
/// with write access.
pub fn finalize(partial: &Path, final_path: &Path) -> io::Result<()> {
    OpenOptions::new().write(true).open(partial)?.sync_all()?;
    rename_new(partial, final_path)?;
    #[cfg(unix)]
    if let Some(folder) = final_path.parent() {
        let _ = File::open(folder).and_then(|dir| dir.sync_all());
    }
    Ok(())
}

/// Renames `from` to `to`, failing with `AlreadyExists` if anything is at
/// `to`, with no moment between the check and the rename in which something
/// could appear there and be replaced. The OS does the check where it can:
/// `renameat2(RENAME_NOREPLACE)` on Linux, `renamex_np(RENAME_EXCL)` on macOS,
/// `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING` on Windows. A folder on a
/// file system that doesn't offer that (some network and removable drives)
/// gets a hard link to `to`, which fails if it exists, and `from` removed.
/// Only where links fail too does it check and then rename.
fn rename_new(from: &Path, to: &Path) -> io::Result<()> {
    if !testing::native_rename_fails() {
        match rename_new_natively(from, to) {
            Err(err) if is_unsupported(&err) => {
                log::info!("renaming {} by linking it: {err}", from.display());
            }
            done => return done,
        }
    }
    if !testing::hard_links_fail() {
        match fs::hard_link(from, to) {
            Ok(()) => {
                // The same file under two names: taking the new one away
                // again undoes it.
                if let Err(err) = fs::remove_file(from) {
                    let _ = fs::remove_file(to);
                    return Err(err);
                }
                return Ok(());
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => return Err(err),
            Err(err) => {
                log::info!("renaming {} after checking, not linking: {err}", from.display())
            }
        }
    }
    if fs::symlink_metadata(to).is_ok() {
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, "a backup has that name already"));
    }
    fs::rename(from, to)
}

/// Whether the file system or the OS lacks the no-replace rename, as opposed
/// to the rename failing.
fn is_unsupported(err: &io::Error) -> bool {
    #[cfg(unix)]
    {
        // (ENOTSUP and EOPNOTSUPP are one value on Linux, two on macOS.)
        let unsupported = [libc::EINVAL, libc::ENOSYS, libc::ENOTSUP, libc::EOPNOTSUPP];
        err.raw_os_error().is_some_and(|code| unsupported.contains(&code))
    }
    #[cfg(not(unix))]
    {
        err.kind() == io::ErrorKind::Unsupported
    }
}

#[cfg(target_os = "linux")]
fn rename_new_natively(from: &Path, to: &Path) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let (from, to) = (cstring(from.as_os_str().as_bytes())?, cstring(to.as_os_str().as_bytes())?);
    // SAFETY: both are NUL-terminated and outlive the call.
    let rc = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            from.as_ptr(),
            libc::AT_FDCWD,
            to.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if rc == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}

#[cfg(target_os = "macos")]
fn rename_new_natively(from: &Path, to: &Path) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let (from, to) = (cstring(from.as_os_str().as_bytes())?, cstring(to.as_os_str().as_bytes())?);
    // SAFETY: both are NUL-terminated and outlive the call.
    let rc = unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) };
    if rc == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}

#[cfg(unix)]
fn cstring(bytes: &[u8]) -> io::Result<std::ffi::CString> {
    std::ffi::CString::new(bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "a NUL in a path"))
}

#[cfg(windows)]
fn rename_new_natively(from: &Path, to: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_WRITE_THROUGH, MoveFileExW};
    let wide = |path: &Path| path.as_os_str().encode_wide().chain([0]).collect::<Vec<u16>>();
    let (from, to) = (wide(from), wide(to));
    // Without MOVEFILE_REPLACE_EXISTING it fails with ERROR_ALREADY_EXISTS (or
    // ERROR_FILE_EXISTS), which `io::Error` reads as `AlreadyExists`.
    // SAFETY: both are NUL-terminated and outlive the call.
    let ok = unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), MOVEFILE_WRITE_THROUGH) };
    if ok != 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}

/// Keeps the newest `keep` backups of the database in `folder` and securely
/// deletes the older ones (003 FR-025), never `protected`, the backup a restore
/// is being made from (003 FR-028), which does not count towards `keep`.
/// The newest backup made before an upgrade is kept as well and does not count
/// either (009 FR-014); older ones among the rest rotate as any other.
/// Returns the backups that could not be deleted.
pub fn rotate(
    folder: &Path,
    database_id: &str,
    keep: usize,
    protected: Option<&Path>,
) -> Vec<PathBuf> {
    let listed = match list(folder, database_id) {
        Ok(listed) => listed,
        Err(err) => {
            log::warn!("could not list the backups in {}: {err}", folder.display());
            return Vec::new();
        }
    };
    let mut failed = Vec::new();
    let newest_before_upgrade = listed.iter().find(|b| b.before_upgrade_to.is_some());
    let rotated = listed
        .iter()
        .filter(|b| newest_before_upgrade.is_none_or(|kept| kept.path != b.path))
        .map(|b| PathBuf::from(&b.path))
        .filter(|path| protected.is_none_or(|protected| !same_path(path, protected)));
    for path in rotated.skip(keep) {
        if let Err(err) = secure_delete::secure_delete_whole_file(&path, WipeControl::default()) {
            log::warn!("could not delete the old backup {}: {err}", path.display());
            failed.push(path);
        }
    }
    failed
}

/// Whether `a` and `b` name the same file or folder, however they are
/// spelled (003 research.md §7).
pub fn same_path(a: &Path, b: &Path) -> bool {
    a == b || matches!((fs::canonicalize(a), fs::canonicalize(b)), (Ok(a), Ok(b)) if a == b)
}

/// Why the backup location can't be used (003 FR-027).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocationProblem {
    Missing,
    NotWritable,
    /// The folder's disk is full: not even the check's probe file fits
    /// (issue #110).
    InsufficientSpace,
}

impl LocationProblem {
    /// As `BACKUP_LOCATION_UNAVAILABLE`'s `details.reason`.
    pub fn as_reason(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::NotWritable => "notWritable",
            Self::InsufficientSpace => "insufficientSpace",
        }
    }
}

/// Checks backups can be written to `folder`. The default folder is made
/// when missing (at the first backup); a custom one that is missing is on a
/// drive or share that isn't there, and is not made (003 FR-024, FR-027).
/// Writability is tried with a probe file, removed at once.
pub fn check_location(folder: &Path, make_if_missing: bool) -> Result<(), LocationProblem> {
    if !folder.is_dir() {
        if !make_if_missing || folder.exists() {
            return Err(LocationProblem::Missing);
        }
        if let Err(err) = fs::create_dir_all(folder) {
            log::warn!("could not make the backup folder {}: {err}", folder.display());
            return Err(LocationProblem::NotWritable);
        }
    }
    let probe = folder.join(format!(".hoplodex-write-check-{}", random_hex(4).unwrap_or_default()));
    match OpenOptions::new().write(true).create_new(true).open(&probe) {
        Ok(_) => {
            let _ = fs::remove_file(&probe);
            Ok(())
        }
        Err(err) => {
            log::warn!("can't write backups to {}: {err}", folder.display());
            Err(if disk_space::is_disk_full(&err) {
                LocationProblem::InsufficientSpace
            } else {
                LocationProblem::NotWritable
            })
        }
    }
}

/// A folder the user chose for the one backup before an upgrade (009 FR-012),
/// checked by [`check_chosen_folder`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChosenFolder {
    /// Where the backup goes: the chosen folder, or the backup location itself
    /// when that is what was chosen.
    pub folder: PathBuf,
    /// The folder is the database's backup location, so the backup is listed
    /// and rotated with the others; any other folder is outside both.
    pub is_backup_location: bool,
}

/// Checks a folder the user chose for the backup before an upgrade
/// (009 research.md §6): absolute, an existing directory, and writable. It is
/// never created. Room for the copy is checked by [`make_backup`], which also
/// creates the file exclusively and never overwrites one (003 FR-026). A
/// folder that is [`same_path`] as `backup_location` is the backup location.
/// Pass the result's `folder` as the job's, with `make_folder: false`.
pub fn check_chosen_folder(
    chosen: &Path,
    backup_location: &Path,
) -> Result<ChosenFolder, LocationProblem> {
    if !chosen.is_absolute() {
        return Err(LocationProblem::Missing);
    }
    check_location(chosen, false)?;
    let is_backup_location = same_path(chosen, backup_location);
    let folder = if is_backup_location { backup_location } else { chosen };
    Ok(ChosenFolder { folder: folder.to_owned(), is_backup_location })
}

/// Why [`make_backup`] made no backup.
#[derive(Debug)]
pub enum BackupFailure {
    Location(LocationProblem),
    InsufficientSpace(InsufficientSpace),
    /// Skipped, or stopped by a sleep (003 FR-027, FR-037).
    Stopped,
    Io(io::Error),
}

#[cfg(feature = "upgrade-test-steps")]
impl From<crate::db::upgrade::testing::Injected> for BackupFailure {
    fn from(err: crate::db::upgrade::testing::Injected) -> Self {
        Self::Io(io::Error::other(err))
    }
}

impl BackupFailure {
    /// The reason a chooser notice and a `CloseOutcome` give; `None` for a
    /// stop, which is not a failure.
    pub fn reason(&self) -> Option<BackupFailureReason> {
        match self {
            Self::Location(LocationProblem::InsufficientSpace) | Self::InsufficientSpace(_) => {
                Some(BackupFailureReason::InsufficientSpace)
            }
            Self::Location(_) => Some(BackupFailureReason::LocationUnavailable),
            Self::Stopped => None,
            // The disk filled while the copy was being written (issue #110).
            Self::Io(err) if disk_space::is_disk_full(err) => {
                Some(BackupFailureReason::InsufficientSpace)
            }
            Self::Io(_) => Some(BackupFailureReason::Io),
        }
    }
}

/// One backup of an open database.
pub struct BackupJob<'a> {
    pub conn: &'a Connection,
    pub database_path: &'a Path,
    /// The database's name, which the backup is named after.
    pub name: &'a str,
    pub database_id: &'a str,
    pub folder: &'a Path,
    /// The folder is the default one, made if missing.
    pub make_folder: bool,
    pub now: DateTime<FixedOffset>,
    /// A regular backup, or the one made before an upgrade.
    pub kind: BackupKind<'a>,
    /// Asked between chunks and before the rename: `true` stops the backup.
    pub cancel: &'a dyn Fn() -> bool,
    /// `(bytes copied, total)`, first with nothing copied.
    pub progress: &'a mut dyn FnMut(u64, u64),
}

/// Makes a backup of the open database (003 research.md §7): checks the
/// location and its free space, records the backup as unfinished in
/// `machine.json`, copies the file through SQLite's own handle (so the
/// database keeps its lock, 003 research.md §3), stamps the copy, and renames
/// it into place. However it ends, the partial file is gone and the record
/// cleared, except after a crash, which the next launch's
/// [`sweep_unfinished`] tidies. Returns the new backup's path. It does not
/// rotate: the caller decides what is protected.
///
/// A backup made before an upgrade (009 research.md §6) is made the same way,
/// under its own name and stamp ([`BackupKind`]), and reaches the test points
/// of 009 research.md §14 (`BeforeBackup`, `DuringCopy`, `AtStamp`,
/// `BeforeRename`).
pub fn make_backup(
    machine: &MachineSettings,
    mut job: BackupJob,
) -> Result<PathBuf, BackupFailure> {
    if let Some(version) = job.kind.before_upgrade_to()
        && !is_backup_version(version)
    {
        // It would make a name no listing reads as a backup of this kind.
        return Err(BackupFailure::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "not a version a backup's name may end in",
        )));
    }
    reach!(
        Point::BeforeBackup,
        job.database_path,
        Some(job.conn),
        if job.kind.is_before_upgrade()
    );
    check_location(job.folder, job.make_folder).map_err(BackupFailure::Location)?;
    let source = RawFile::of(job.conn).map_err(BackupFailure::Io)?;
    let len = source.size().map_err(BackupFailure::Io)?;
    disk_space::check_room_for_copy(len, job.folder).map_err(BackupFailure::InsufficientSpace)?;

    let final_path =
        job.folder.join(backup_file_name(job.name, &job.now, job.database_id, job.kind));
    let mut partial = final_path.clone().into_os_string();
    partial.push(PARTIAL_SUFFIX);
    let partial = PathBuf::from(partial);
    // Created exclusively and before it is recorded: a file already at that
    // name is not ours, so it stays, and nothing is swept or removed for it.
    let out = scratch::create(&partial).map_err(|err| {
        log::error!("could not create {}: {err}", partial.display());
        BackupFailure::Io(err)
    })?;
    machine.set_unfinished_backup(UnfinishedBackup {
        database_path: job.database_path.to_owned(),
        partial_path: partial.clone(),
        started_at: utc_text(&job.now),
        before_upgrade: job.kind.is_before_upgrade(),
    });

    let made = write_backup(&mut job, &source, out, &partial, &final_path);
    if made.is_err() {
        remove_partial(&partial);
    }
    machine.clear_unfinished_backup();
    match made {
        Ok(()) => Ok(final_path),
        Err(CopyError::Stopped) => Err(BackupFailure::Stopped),
        Err(CopyError::Io(err)) => {
            log::error!("the backup of {} failed: {err}", job.database_path.display());
            Err(BackupFailure::Io(err))
        }
    }
}

/// The copy, the stamp and the rename of [`make_backup`], which removes the
/// partial file if this fails.
fn write_backup(
    job: &mut BackupJob,
    source: &RawFile,
    out: File,
    partial: &Path,
    final_path: &Path,
) -> Result<(), CopyError> {
    copy_chunked(source, out, job.cancel, job.progress)?;
    reach!(Point::DuringCopy, job.database_path, Some(job.conn), if job.kind.is_before_upgrade());
    reach!(Point::AtStamp, job.database_path, Some(job.conn), if job.kind.is_before_upgrade());
    stamp_backup(job.conn, partial, job.database_id, job.name, &job.now, job.kind)
        .map_err(|err| CopyError::Io(sqlite_as_io(err)))?;
    if (job.cancel)() {
        return Err(CopyError::Stopped);
    }
    reach!(Point::BeforeRename, job.database_path, Some(job.conn), if job.kind.is_before_upgrade());
    finalize(partial, final_path).map_err(CopyError::Io)
}

/// A SQLite failure while stamping the copy, as the I/O error it stands for:
/// a full disk stays recognizable as one.
fn sqlite_as_io(err: rusqlite::Error) -> io::Error {
    if err.sqlite_error_code() == Some(rusqlite::ErrorCode::DiskFull) {
        io::Error::new(io::ErrorKind::StorageFull, err)
    } else {
        io::Error::other(err)
    }
}

/// Removes a backup that did not finish, with its journal if it has one.
fn remove_partial(partial: &Path) {
    if let Err(err) = secure_delete::secure_delete_file(partial)
        && err.kind() != io::ErrorKind::NotFound
    {
        log::warn!("could not remove {}: {err}", partial.display());
    }
    remove_journal(partial);
}

/// At launch: a backup recorded as unfinished was cut short by a crash or a
/// forced quit (003 US3-7). Its partial file is removed and the chooser says
/// the changes will be backed up at the next close (not for a backup made
/// before an upgrade: 009 FR-015). A move of backups cut
/// short the same way (003 research.md §22) has its partial copy removed, and
/// the chooser says how many backups are still at the old location.
pub fn sweep_unfinished(machine: &MachineSettings) {
    if let Some(record) = machine.unfinished_backup() {
        log::warn!("the last backup of {} did not finish", record.database_path.display());
        remove_partial(&record.partial_path);
        // An interrupted backup before an upgrade is not reported: the
        // upgrade didn't start, and the next open asks again (009 FR-015).
        // An ordinary one is: its changes will be backed up at the next close.
        if !record.before_upgrade {
            machine.push_notice(ChooserNotice::BackupFailed {
                database_path: record.database_path.to_string_lossy().into_owned(),
                reason: BackupFailureReason::Interrupted,
            });
        }
        machine.clear_unfinished_backup();
    }
    if let Some(record) = machine.unfinished_backup_move() {
        log::warn!("the move of the backups of {} did not finish", record.database_path.display());
        if let Some(partial) = &record.partial_path {
            remove_partial(partial);
        }
        let count = list(&record.from_folder, &record.database_id).map_or(0, |left| left.len());
        if count > 0 {
            machine.push_notice(ChooserNotice::BackupsLeftBehind {
                database_path: record.database_path.to_string_lossy().into_owned(),
                folder: record.from_folder.to_string_lossy().into_owned(),
                count: count as u64,
            });
        }
        machine.clear_unfinished_backup_move();
    }
}

/// The backups of a database a move to a new folder takes, oldest first,
/// and those it leaves because a file of the same name is already there
/// (003 FR-026: nothing is ever overwritten).
#[derive(Debug, Clone, Default)]
pub struct MovePlan {
    pub to_move: Vec<BackupInfo>,
    pub name_taken: Vec<BackupInfo>,
}

impl MovePlan {
    /// The bytes to be moved, which the new folder needs room for.
    pub fn bytes(&self) -> u64 {
        self.to_move.iter().map(|backup| backup.size_bytes).sum()
    }

    /// Every backup of the database in the old folder.
    pub fn count(&self) -> u64 {
        (self.to_move.len() + self.name_taken.len()) as u64
    }
}

/// Plans moving the backups of `database_id` from `from` to `to`.
pub fn plan_move(from: &Path, to: &Path, database_id: &str) -> io::Result<MovePlan> {
    let mut listed = list(from, database_id)?;
    listed.reverse();
    let (name_taken, to_move) = listed
        .into_iter()
        .partition(|backup| fs::symlink_metadata(to.join(&backup.file_name)).is_ok());
    Ok(MovePlan { to_move, name_taken })
}

/// One move of a database's backups to its new location.
pub struct MoveJob<'a> {
    pub machine: &'a MachineSettings,
    pub database_path: &'a Path,
    pub database_id: &'a str,
    pub from: &'a Path,
    pub to: &'a Path,
    pub plan: &'a MovePlan,
    /// Asked between files and between chunks: `true` stops the move.
    pub cancel: &'a dyn Fn() -> bool,
    /// `(bytes processed, total)`, first with nothing processed. Copying
    /// and reading back each count, so `total` is twice the bytes to move.
    pub progress: &'a mut dyn FnMut(u64, u64),
    /// How many backups are still in `from`, after each one moved.
    pub not_yet_moved: &'a dyn Fn(u64),
}

/// How a move ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moved {
    pub moved: u64,
    /// Still in the old folder.
    pub left_behind: u64,
    /// Why some were left behind; `None` when none were, or when stopped.
    pub reason: Option<LeftBehindReason>,
    /// A sleep stopped it (003 FR-037).
    pub stopped: bool,
}

/// Why one backup was not moved.
enum MoveFailure {
    Stopped,
    NameTaken,
    Io(io::Error),
}

impl From<io::Error> for MoveFailure {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<CopyError> for MoveFailure {
    fn from(err: CopyError) -> Self {
        match err {
            CopyError::Stopped => Self::Stopped,
            CopyError::Io(err) => Self::Io(err),
        }
    }
}

/// Moves the planned backups, oldest first (003 FR-026, research.md §22). Each
/// is hard-linked into the new folder and its old name removed; when
/// linking fails (another drive, or a filesystem without hard links) it is
/// copied to `<name>.partial`, read back and compared byte for byte with
/// the original, renamed to its name, and only then is the original
/// securely deleted. A copy that is stopped or fails is removed and its
/// original kept. A name taken at the new folder is skipped and counted as
/// left behind. The move ends at the first failure, and never rotates.
/// While it runs, `machine.json` records it, for the sweep after a crash.
pub fn move_backups(job: MoveJob) -> Moved {
    let total = job.plan.bytes() * 2;
    let mut processed = 0;
    (job.progress)(0, total);
    let mut left_behind = job.plan.count();
    (job.not_yet_moved)(left_behind);
    job.machine.set_unfinished_backup_move(UnfinishedBackupMove {
        database_path: job.database_path.to_owned(),
        database_id: job.database_id.to_owned(),
        from_folder: job.from.to_owned(),
        partial_path: None,
    });
    let mut name_taken = !job.plan.name_taken.is_empty();
    let mut ended = None;
    for backup in &job.plan.to_move {
        if (job.cancel)() {
            ended = Some(MoveFailure::Stopped);
            break;
        }
        let src = Path::new(&backup.path);
        let dst = job.to.join(&backup.file_name);
        let base = processed;
        let mut file_progress = |done: u64| (job.progress)(base + done, total);
        match move_one(job.machine, src, &dst, backup.size_bytes, job.cancel, &mut file_progress) {
            Ok(()) => {
                left_behind -= 1;
                (job.not_yet_moved)(left_behind);
            }
            Err(MoveFailure::NameTaken) => name_taken = true,
            Err(failure) => {
                ended = Some(failure);
                break;
            }
        }
        processed = base + backup.size_bytes * 2;
        (job.progress)(processed, total);
    }
    job.machine.clear_unfinished_backup_move();
    let moved = job.plan.count() - left_behind;
    let (reason, stopped) = match ended {
        Some(MoveFailure::Stopped) => (None, true),
        Some(MoveFailure::Io(err)) => {
            log::error!("the move of backups to {} failed: {err}", job.to.display());
            (Some(failure_reason(job.to, &err)), false)
        }
        Some(MoveFailure::NameTaken) | None => {
            (name_taken.then_some(LeftBehindReason::NameTaken), false)
        }
    };
    Moved { moved, left_behind, reason, stopped }
}

/// Why a move failed, as the user is told: the new folder gone, full, or
/// anything else.
fn failure_reason(to: &Path, err: &io::Error) -> LeftBehindReason {
    if !to.is_dir() {
        LeftBehindReason::LocationUnavailable
    } else if err.kind() == io::ErrorKind::StorageFull {
        LeftBehindReason::InsufficientSpace
    } else {
        LeftBehindReason::Io
    }
}

/// Moves one backup of `len` bytes from `src` to `dst`, reporting its own
/// progress from 0 to twice `len`.
fn move_one(
    machine: &MachineSettings,
    src: &Path,
    dst: &Path,
    len: u64,
    cancel: &dyn Fn() -> bool,
    progress: &mut dyn FnMut(u64),
) -> Result<(), MoveFailure> {
    if !testing::hard_links_fail() {
        match fs::hard_link(src, dst) {
            Ok(()) => {
                // The same file under two names: taking the new one away
                // again undoes it.
                if let Err(err) = fs::remove_file(src) {
                    let _ = fs::remove_file(dst);
                    return Err(err.into());
                }
                sync_folder(dst);
                sync_folder(src);
                progress(len * 2);
                return Ok(());
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                return Err(MoveFailure::NameTaken);
            }
            Err(err) => log::info!("copying {} instead of linking it: {err}", src.display()),
        }
    }
    // The original must be deletable once copied, or the move stops here
    // with nothing done.
    OpenOptions::new().write(true).open(src)?;
    let mut partial = dst.as_os_str().to_owned();
    partial.push(PARTIAL_SUFFIX);
    let partial = PathBuf::from(partial);
    // Exclusive, and before it is recorded, as for a backup: what is already
    // there is not ours and is left alone.
    let out = scratch::create(&partial)?;
    machine.set_unfinished_backup_move_partial(Some(&partial));
    let copied = copy_and_verify(src, out, &partial, dst, cancel, progress);
    if copied.is_err() {
        remove_partial(&partial);
    }
    machine.set_unfinished_backup_move_partial(None);
    copied?;
    // The copy is verified and in place. If the original can't be deleted
    // now, both are kept: nothing is lost, and the move stops.
    secure_delete::secure_delete_whole_file(
        src,
        WipeControl { progress: None, cancel: Some(cancel) },
    )?;
    sync_folder(src);
    Ok(())
}

/// Copies `src` to `partial`, reads it back comparing it byte for byte
/// with `src`, then renames it to `dst`, which must not be taken.
fn copy_and_verify(
    src: &Path,
    out: File,
    partial: &Path,
    dst: &Path,
    cancel: &dyn Fn() -> bool,
    progress: &mut dyn FnMut(u64),
) -> Result<(), MoveFailure> {
    let source = File::open(src)?;
    let len = ReadAt::size(&source)?;
    copy_chunked(&source, out, cancel, &mut |done, _| progress(done))?;
    testing::after_copy(partial);
    let copy = File::open(partial)?;
    if ReadAt::size(&copy)? != len {
        return Err(MoveFailure::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            "the copy is not the size of the original",
        )));
    }
    let mut expected = vec![0u8; COPY_CHUNK as usize];
    let mut found = vec![0u8; COPY_CHUNK as usize];
    let mut done = 0;
    while done < len {
        if cancel() {
            return Err(MoveFailure::Stopped);
        }
        let n = (len - done).min(COPY_CHUNK) as usize;
        source.read_exact_at(&mut expected[..n], done)?;
        copy.read_exact_at(&mut found[..n], done)?;
        if expected[..n] != found[..n] {
            return Err(MoveFailure::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "the copy differs from the original",
            )));
        }
        done += n as u64;
        progress(len + done);
    }
    drop(copy);
    match finalize(partial, dst) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => Err(MoveFailure::NameTaken),
        Err(err) => Err(err.into()),
    }
}

/// Makes a rename or removal in the folder holding `path` durable.
fn sync_folder(path: &Path) {
    #[cfg(unix)]
    if let Some(folder) = path.parent() {
        let _ = File::open(folder).and_then(|dir| dir.sync_all());
    }
    #[cfg(not(unix))]
    let _ = path;
}

type AfterCopy = Box<dyn Fn(&Path)>;

thread_local! {
    static NATIVE_RENAME_FAILS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static HARD_LINKS_FAIL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static AFTER_COPY: std::cell::RefCell<Option<AfterCopy>> = const { std::cell::RefCell::new(None) };
}

/// Lets the tests take a move of backups down the copy path on one drive,
/// and tamper with a copy before it is verified.
#[doc(hidden)]
pub mod testing {
    use super::*;

    /// Undoes [`fail_hard_links`] when dropped.
    pub struct HardLinksFailGuard(());

    impl Drop for HardLinksFailGuard {
        fn drop(&mut self) {
            HARD_LINKS_FAIL.with(|fail| fail.set(false));
        }
    }

    /// Until the guard is dropped, moves on this thread copy instead of
    /// linking, as between drives.
    pub fn fail_hard_links() -> HardLinksFailGuard {
        HARD_LINKS_FAIL.with(|fail| fail.set(true));
        HardLinksFailGuard(())
    }

    pub(super) fn hard_links_fail() -> bool {
        HARD_LINKS_FAIL.with(|fail| fail.get())
    }

    /// Undoes [`fail_native_rename`] when dropped.
    pub struct NativeRenameFailsGuard(());

    impl Drop for NativeRenameFailsGuard {
        fn drop(&mut self) {
            NATIVE_RENAME_FAILS.with(|fail| fail.set(false));
        }
    }

    /// Until the guard is dropped, [`finalize`] on this thread behaves as on
    /// a file system without a no-replace rename, and takes the fallbacks
    /// (009 research.md §17).
    pub fn fail_native_rename() -> NativeRenameFailsGuard {
        NATIVE_RENAME_FAILS.with(|fail| fail.set(true));
        NativeRenameFailsGuard(())
    }

    pub(super) fn native_rename_fails() -> bool {
        NATIVE_RENAME_FAILS.with(|fail| fail.get())
    }

    /// Undoes [`after_each_copy`] when dropped.
    pub struct AfterCopyGuard(());

    impl Drop for AfterCopyGuard {
        fn drop(&mut self) {
            AFTER_COPY.with(|hook| hook.borrow_mut().take());
        }
    }

    /// Until the guard is dropped, `hook` sees each copy a move on this
    /// thread makes, before it is verified.
    pub fn after_each_copy(hook: impl Fn(&Path) + 'static) -> AfterCopyGuard {
        AFTER_COPY.with(|slot| *slot.borrow_mut() = Some(Box::new(hook)));
        AfterCopyGuard(())
    }

    pub(super) fn after_copy(partial: &Path) {
        AFTER_COPY.with(|hook| {
            if let Some(hook) = hook.borrow().as_ref() {
                hook(partial);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The OS's disk-full codes reach the user as insufficient space
    /// (003 FR-026): `ENOSPC` on Linux and macOS, `ERROR_HANDLE_DISK_FULL` and
    /// `ERROR_DISK_FULL` on Windows.
    #[test]
    fn a_full_disk_is_reported_as_insufficient_space() {
        #[cfg(unix)]
        let codes = [28];
        #[cfg(windows)]
        let codes = [39, 112];
        let folder = std::env::temp_dir();
        for code in codes {
            let err = io::Error::from_raw_os_error(code);
            assert_eq!(failure_reason(&folder, &err), LeftBehindReason::InsufficientSpace);
        }
        let other = io::Error::from(io::ErrorKind::PermissionDenied);
        assert_eq!(failure_reason(&folder, &other), LeftBehindReason::Io);
    }

    fn at(rfc3339: &str) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(rfc3339).unwrap()
    }

    /// 009 FR-013: the versions a backup's name may end in.
    #[test]
    fn a_backup_name_may_end_in_these_versions_only() {
        for good in ["0.2.0", "10.20.30", "1.0.0-beta.1", "1.0.0+build.5", "1.2.3-rc-1"] {
            assert!(is_backup_version(good), "{good}");
        }
        for bad in [
            "",
            "1.0",
            "1.0.0.0",
            "1.0.x",
            "v1.0.0",
            "1.0.0-",
            "1.0.0+",
            "1.0.0-a b",
            "1.0.0-rc+1",
            "1.0.0 2027-01-15 093012 3f9c0a12",
        ] {
            assert!(!is_backup_version(bad), "{bad}");
        }
    }

    /// 009 FR-013, research.md §6: the name says what the backup is, and an
    /// ordinary backup of a database named like the suffix is not one.
    #[test]
    fn a_name_with_a_before_suffix_reads_back_as_made_before_an_upgrade() {
        let now = at("2027-01-15T09:30:12+00:00");
        let id = "3f9c0a12deadbeef";
        let regular = backup_file_name("Main collection", &now, id, BackupKind::Regular);
        let before = backup_file_name(
            "Main collection",
            &now,
            id,
            BackupKind::BeforeUpgrade { to_version: "0.2.0" },
        );
        assert_eq!(regular, "Main collection 2027-01-15 093012 3f9c0a12.hoplodex");
        assert_eq!(before, "Main collection 2027-01-15 093012 3f9c0a12 before 0.2.0.hoplodex");
        let made = NaiveDateTime::parse_from_str("2027-01-15 093012", "%Y-%m-%d %H%M%S").unwrap();
        assert_eq!(made_at_by_name(&regular, "3f9c0a12"), Some((made, None)));
        assert_eq!(made_at_by_name(&before, "3f9c0a12"), Some((made, Some("0.2.0".to_owned()))));

        // A database called "Guns before 1.0.0".
        let odd = backup_file_name("Guns before 1.0.0", &now, id, BackupKind::Regular);
        assert_eq!(made_at_by_name(&odd, "3f9c0a12"), Some((made, None)));
        let odd_before = backup_file_name(
            "Guns before 1.0.0",
            &now,
            id,
            BackupKind::BeforeUpgrade { to_version: "1.0.0-beta.1" },
        );
        assert_eq!(
            made_at_by_name(&odd_before, "3f9c0a12"),
            Some((made, Some("1.0.0-beta.1".to_owned())))
        );
        // Another database's, and a suffix with nothing before it.
        assert_eq!(made_at_by_name(&before, "00000000"), None);
        assert_eq!(made_at_by_name("Main before 0.2.0.hoplodex", "3f9c0a12"), None);
    }

    /// 009 FR-015: an interrupted backup before an upgrade is swept without a
    /// notice; an ordinary one still gets its notice.
    #[test]
    fn the_sweep_says_nothing_about_a_backup_made_before_an_upgrade() {
        for (before_upgrade, notices) in [(true, 0), (false, 1)] {
            let config = tempfile::TempDir::new().unwrap();
            let folder = tempfile::TempDir::new().unwrap();
            let machine = MachineSettings::load(config.path()).unwrap();
            let partial = folder.path().join("A 2027-01-15 093012 3f9c0a12.hoplodex.partial");
            fs::write(&partial, b"half a backup").unwrap();
            machine.set_unfinished_backup(UnfinishedBackup {
                database_path: folder.path().join("A.hoplodex"),
                partial_path: partial.clone(),
                started_at: "2027-01-15T09:30:12Z".into(),
                before_upgrade,
            });

            sweep_unfinished(&machine);

            assert!(!partial.exists(), "before_upgrade {before_upgrade}");
            assert_eq!(machine.unfinished_backup(), None);
            assert_eq!(machine.take_notices().len(), notices, "before_upgrade {before_upgrade}");
        }
    }

    /// 009 FR-012: a chosen folder is absolute, exists and is a directory,
    /// and is never made; the backup location, spelled another way, is the
    /// backup location.
    #[test]
    fn a_chosen_folder_must_be_absolute_and_exist() {
        let root = tempfile::TempDir::new().unwrap();
        let location = root.path().join("HoploDex backups");
        fs::create_dir(&location).unwrap();
        let elsewhere = root.path().join("usb");
        fs::create_dir(&elsewhere).unwrap();

        let chosen = check_chosen_folder(&elsewhere, &location).unwrap();
        assert_eq!(chosen, ChosenFolder { folder: elsewhere.clone(), is_backup_location: false });
        let respelled = elsewhere.join("..").join("HoploDex backups");
        let chosen = check_chosen_folder(&respelled, &location).unwrap();
        assert_eq!(chosen, ChosenFolder { folder: location.clone(), is_backup_location: true });

        let missing = root.path().join("not there");
        assert_eq!(check_chosen_folder(&missing, &location), Err(LocationProblem::Missing));
        assert!(!missing.exists(), "a chosen folder is never made");
        assert_eq!(
            check_chosen_folder(Path::new("relative/folder"), &location),
            Err(LocationProblem::Missing)
        );
        let file = root.path().join("a file");
        fs::write(&file, b"x").unwrap();
        assert_eq!(check_chosen_folder(&file, &location), Err(LocationProblem::Missing));
    }
}
