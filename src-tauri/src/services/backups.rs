//! Automatic backups (FR-024–FR-029; research.md §7, §9; data-model.md
//! "Backup files"): when one is due, where it goes, what it is called, how
//! it is copied and stamped, and how many are kept. The close in
//! `session::lifecycle` and the restore in `commands::backups` make them
//! through [`make_backup`].

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset, NaiveDateTime, Utc};
use rusqlite::Connection;

use crate::db::random_hex;
use crate::db::raw_file::RawFile;
use crate::models::database::{BackupFailureReason, BackupInfo, ChooserNotice, DATABASE_EXTENSION};
use crate::services::disk_space::{self, InsufficientSpace};
use crate::services::machine_settings::{MachineSettings, UnfinishedBackup};
use crate::services::secure_delete::{self, WipeControl};

/// The default backup folder's name, in the database's own folder.
pub const DEFAULT_FOLDER_NAME: &str = "HoploDex backups";
/// What a backup being written ends in, so no listing or open dialog takes
/// it for a backup.
const PARTIAL_SUFFIX: &str = ".partial";
/// Copies go in chunks of this size, stoppable between them.
const COPY_CHUNK: u64 = 1024 * 1024;
/// A deliberately slow copy speed, allowing for hard disks and network
/// folders, to decide whether the progress bar shows at once (research.md
/// §7). The development machine copies at about 190 MiB/s.
const ASSUMED_BYTES_PER_SECOND: u64 = 50 * 1024 * 1024;

/// The folder a database's backups go to on this computer (research.md
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

/// Whether a close should make a backup (FR-025).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Due {
    Yes,
    /// Automatic backups are turned off.
    Off,
    /// Nothing has changed since the last backup.
    NothingChanged,
    /// There was a backup today already, in local time.
    AlreadyToday,
}

/// Backups on, changes waiting, and no backup yet today on this computer's
/// calendar: `last_backup_at` (UTC) is compared by its local date with
/// `now`'s.
pub fn is_due(
    enabled: bool,
    changes_waiting: bool,
    last_backup_at: Option<&str>,
    now: &DateTime<FixedOffset>,
) -> Due {
    if !enabled {
        return Due::Off;
    }
    if !changes_waiting {
        return Due::NothingChanged;
    }
    let last_local_date = last_backup_at
        .and_then(|last| DateTime::parse_from_rfc3339(last).ok())
        .map(|last| last.with_timezone(now.offset()).date_naive());
    if last_local_date == Some(now.date_naive()) {
        Due::AlreadyToday
    } else {
        Due::Yes
    }
}

/// `<name> <YYYY-MM-DD HHMMSS> <id8>.hoplodex`, in local time (research.md
/// §7). The first 8 hex digits of the database's id say whose backup it is.
pub fn backup_file_name(name: &str, now: &DateTime<FixedOffset>, database_id: &str) -> String {
    format!("{name} {} {}.{DATABASE_EXTENSION}", now.format("%Y-%m-%d %H%M%S"), id8(database_id))
}

fn id8(database_id: &str) -> &str {
    database_id.get(..8).unwrap_or(database_id)
}

/// When a file named like a backup of the database `id8` was made, by its
/// name: a name, the local date and time, then the id. Anything else is not
/// one of its backups.
fn made_at_by_name(file_name: &str, id8: &str) -> Option<NaiveDateTime> {
    let stem = file_name.strip_suffix(&format!(".{DATABASE_EXTENSION}"))?;
    let mut parts = stem.rsplitn(4, ' ');
    let (id, time, date, name) = (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    let digits = |text: &str| text.bytes().all(|b| b.is_ascii_digit() || b == b'-');
    if id != id8 || name.is_empty() || date.len() != 10 || time.len() != 6 {
        return None;
    }
    if !digits(date) || !digits(time) {
        return None;
    }
    NaiveDateTime::parse_from_str(&format!("{date} {time}"), "%Y-%m-%d %H%M%S").ok()
}

/// The backups of the database `database_id` in `folder`, newest first
/// (FR-028). They are chosen by the id and the timestamp in their names,
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
        let Some(made_at) = made_at_by_name(&file_name, id8) else { continue };
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
/// once; a shorter one only if it is still running a second later (SC-005).
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

/// Copies `src` byte for byte to the new file `dst`, in 1 MiB chunks,
/// reporting `(bytes copied, total)` first with nothing copied and then
/// after each chunk, and asking `cancel` before each chunk. The caller
/// removes `dst` when it fails.
pub fn copy_chunked(
    src: &dyn ReadAt,
    dst: &Path,
    cancel: &dyn Fn() -> bool,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<(), CopyError> {
    let total = src.size()?;
    let mut out = OpenOptions::new().write(true).create_new(true).open(dst)?;
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

/// Stamps a fresh byte copy of the open database as a backup (research.md
/// §3, §9): attached to the open connection without a key, which makes
/// SQLCipher use the main database's key (the copy shares its salt, so it
/// opens), the copy gets its open marker cleared, its pending changes
/// removed and the backup stamp set. Nothing is waiting to be backed up in
/// it: its content is a backup.
pub fn stamp_backup(
    conn: &Connection,
    partial: &Path,
    name: &str,
    now: &DateTime<FixedOffset>,
) -> rusqlite::Result<()> {
    conn.execute("ATTACH DATABASE ?1 AS backup", [partial.to_string_lossy()])?;
    let stamped = conn.execute_batch("BEGIN").and_then(|()| {
        conn.execute(
            "UPDATE backup.app_state
             SET open_machine_id = NULL, open_machine_name = NULL, open_since = NULL,
                 changes_waiting = 0, backup_made_at = ?1, backup_of_name = ?2",
            rusqlite::params![utc_text(now), name],
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
/// must not be taken.
pub fn finalize(partial: &Path, final_path: &Path) -> io::Result<()> {
    File::open(partial)?.sync_all()?;
    if fs::symlink_metadata(final_path).is_ok() {
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, "a backup has that name already"));
    }
    fs::rename(partial, final_path)?;
    #[cfg(unix)]
    if let Some(folder) = final_path.parent() {
        let _ = File::open(folder).and_then(|dir| dir.sync_all());
    }
    Ok(())
}

/// Keeps the newest `keep` backups of the database in `folder` and securely
/// deletes the older ones (FR-025), never `protected`, the backup a restore
/// is being made from (FR-028), which does not count towards `keep`.
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
    let rotated = listed
        .iter()
        .map(|b| PathBuf::from(&b.path))
        .filter(|path| protected.map_or(true, |protected| !same_path(path, protected)));
    for path in rotated.skip(keep) {
        if let Err(err) = secure_delete::secure_delete_whole_file(&path, WipeControl::default()) {
            log::warn!("could not delete the old backup {}: {err}", path.display());
            failed.push(path);
        }
    }
    failed
}

fn same_path(a: &Path, b: &Path) -> bool {
    a == b || matches!((fs::canonicalize(a), fs::canonicalize(b)), (Ok(a), Ok(b)) if a == b)
}

/// Why the backup location can't be used (FR-027).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocationProblem {
    Missing,
    NotWritable,
}

impl LocationProblem {
    /// As `BACKUP_LOCATION_UNAVAILABLE`'s `details.reason`.
    pub fn as_reason(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::NotWritable => "notWritable",
        }
    }
}

/// Checks backups can be written to `folder`. The default folder is made
/// when missing (at the first backup); a custom one that is missing is on a
/// drive or share that isn't there, and is not made (FR-024, FR-027).
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
            Err(LocationProblem::NotWritable)
        }
    }
}

/// Why [`make_backup`] made no backup.
#[derive(Debug)]
pub enum BackupFailure {
    Location(LocationProblem),
    InsufficientSpace(InsufficientSpace),
    /// Skipped, or stopped by a sleep (FR-027, FR-037).
    Stopped,
    Io(io::Error),
}

impl BackupFailure {
    /// The reason a chooser notice and a `CloseOutcome` give; `None` for a
    /// stop, which is not a failure.
    pub fn reason(&self) -> Option<BackupFailureReason> {
        match self {
            Self::Location(_) => Some(BackupFailureReason::LocationUnavailable),
            Self::InsufficientSpace(_) => Some(BackupFailureReason::InsufficientSpace),
            Self::Stopped => None,
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
    /// Asked between chunks and before the rename: `true` stops the backup.
    pub cancel: &'a dyn Fn() -> bool,
    /// `(bytes copied, total)`, first with nothing copied.
    pub progress: &'a mut dyn FnMut(u64, u64),
}

/// Makes a backup of the open database (research.md §7): checks the
/// location and its free space, records the backup as unfinished in
/// `machine.json`, copies the file through SQLite's own handle (so the
/// database keeps its lock, research.md §3), stamps the copy, and renames
/// it into place. However it ends, the partial file is gone and the record
/// cleared, except after a crash, which the next launch's
/// [`sweep_unfinished`] tidies. Returns the new backup's path. It does not
/// rotate: the caller decides what is protected.
pub fn make_backup(machine: &MachineSettings, job: BackupJob) -> Result<PathBuf, BackupFailure> {
    check_location(job.folder, job.make_folder).map_err(BackupFailure::Location)?;
    let source = RawFile::of(job.conn).map_err(BackupFailure::Io)?;
    let len = source.size().map_err(BackupFailure::Io)?;
    disk_space::check_room_for_copy(len, job.folder).map_err(BackupFailure::InsufficientSpace)?;

    let final_path = job.folder.join(backup_file_name(job.name, &job.now, job.database_id));
    let mut partial = final_path.clone().into_os_string();
    partial.push(PARTIAL_SUFFIX);
    let partial = PathBuf::from(partial);
    machine.set_unfinished_backup(UnfinishedBackup {
        database_path: job.database_path.to_owned(),
        partial_path: partial.clone(),
        started_at: utc_text(&job.now),
    });

    let made = copy_chunked(&source, &partial, job.cancel, job.progress)
        .and_then(|()| {
            stamp_backup(job.conn, &partial, job.name, &job.now)
                .map_err(|err| CopyError::Io(io::Error::other(err)))
        })
        .and_then(|()| if (job.cancel)() { Err(CopyError::Stopped) } else { Ok(()) })
        .and_then(|()| finalize(&partial, &final_path).map_err(CopyError::Io));
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

/// Removes a backup that did not finish, with its journal if it has one.
fn remove_partial(partial: &Path) {
    if let Err(err) = secure_delete::secure_delete_file(partial) {
        if err.kind() != io::ErrorKind::NotFound {
            log::warn!("could not remove {}: {err}", partial.display());
        }
    }
    remove_journal(partial);
}

/// At launch: a backup recorded as unfinished was cut short by a crash or a
/// forced quit (US3-7). Its partial file is removed and the chooser says
/// the changes will be backed up at the next close.
pub fn sweep_unfinished(machine: &MachineSettings) {
    let Some(record) = machine.unfinished_backup() else { return };
    log::warn!("the last backup of {} did not finish", record.database_path.display());
    remove_partial(&record.partial_path);
    machine.push_notice(ChooserNotice::BackupFailed {
        database_path: record.database_path.to_string_lossy().into_owned(),
        reason: BackupFailureReason::Interrupted,
    });
    machine.clear_unfinished_backup();
}
