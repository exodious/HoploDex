//! `machine.json`: what this computer (this OS account) keeps about
//! databases, and nothing more (FR-013, research.md §6, §11; data-model.md
//! "Machine-local"). It holds paths, names and times only, never collection
//! data or secrets. The config directory is always passed in, so tests and
//! tools use throwaway ones.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};

use crate::db::{now_utc, random_hex};
use crate::models::database::ChooserNotice;
use crate::models::document_opening::DocumentOpening;
use crate::services::keyring::Keyring;

const FILE_NAME: &str = "machine.json";
const VERSION: u32 = 1;
/// `app_state.open_machine_name` holds at most this many characters.
const MAX_DISPLAY_NAME_CHARS: usize = 255;

/// This computer as the open marker records it (FR-032).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineIdentity {
    /// 32 hex digits, random on first run.
    pub id: String,
    /// The host name, shown to someone on another computer.
    pub display_name: String,
}

/// One entry of the recent-databases list (FR-012).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentEntry {
    pub path: PathBuf,
    /// The file name without its extension, refreshed at each open.
    pub name: String,
    /// UTC ISO-8601.
    pub last_opened_at: String,
    /// Cached at each open; `None` until first opened here.
    pub database_id: Option<String>,
    /// Resolved and cached at each open, for restoring a database that no
    /// longer opens.
    pub backup_folder: Option<PathBuf>,
    /// FR-017. The passphrase itself is only ever in the keyring.
    pub passphrase_saved: bool,
    /// The file's modification time, UTC ISO-8601 to the millisecond, when
    /// this computer last closed it (FR-040). `None` while it is open here
    /// or after it ended without a close (a crash), after a close that found
    /// it out of reach or taken over, and after the entry is re-located:
    /// then nothing is said about changes made elsewhere.
    #[serde(default)]
    pub left_modified_at: Option<String>,
}

/// The modification time of the file at `path`, as `left_modified_at`
/// records it, or `None` when it can't be read. Only `stat`s the file, so
/// it is safe on a database this process holds open.
pub fn modified_at(path: &Path) -> Option<String> {
    let modified = fs::metadata(path).and_then(|meta| meta.modified()).ok()?;
    Some(
        chrono::DateTime::<chrono::Utc>::from(modified)
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string(),
    )
}

/// A backup that was being written when HoploDex stopped (research.md §7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnfinishedBackup {
    pub database_path: PathBuf,
    pub partial_path: PathBuf,
    pub started_at: String,
}

/// A move of backups to a new location that was under way when HoploDex
/// stopped (research.md §22).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnfinishedBackupMove {
    pub database_path: PathBuf,
    pub database_id: String,
    /// The old location, where the backups not yet moved still are.
    pub from_folder: PathBuf,
    /// The copy being made, if a backup was being copied rather than linked.
    pub partial_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MachineFile {
    version: u32,
    machine_id: String,
    #[serde(default)]
    recent_databases: Vec<RecentEntry>,
    #[serde(default)]
    unfinished_backup: Option<UnfinishedBackup>,
    #[serde(default)]
    unfinished_backup_move: Option<UnfinishedBackupMove>,
    #[serde(default)]
    notices: Vec<ChooserNotice>,
    /// FR-011. A file written before this field existed reads as Preview, so
    /// `VERSION` stays 1.
    #[serde(default)]
    document_opening: DocumentOpening,
}

impl MachineFile {
    fn fresh() -> io::Result<Self> {
        Ok(Self {
            version: VERSION,
            machine_id: random_hex(16).map_err(io::Error::other)?,
            recent_databases: Vec::new(),
            unfinished_backup: None,
            unfinished_backup_move: None,
            notices: Vec::new(),
            document_opening: DocumentOpening::default(),
        })
    }

    fn is_valid(&self) -> bool {
        self.version == VERSION
            && self.machine_id.len() == 32
            && self.machine_id.chars().all(|c| c.is_ascii_hexdigit())
    }
}

/// The loaded `machine.json`, kept as Tauri state. Every change is written
/// straight back, atomically. A write that fails is logged and not
/// reported: losing the recent list must never stop a database opening.
///
/// It also holds this computer's [`Keyring`], where the passphrases its
/// `passphraseSaved` flags stand for are kept (FR-017). A loaded one has the
/// keyring [`off`](Keyring::off), so tests and tools never reach the OS
/// keyring; the app gives it the real one with
/// [`with_keyring`](Self::with_keyring).
pub struct MachineSettings {
    path: PathBuf,
    file: Mutex<MachineFile>,
    keyring: Keyring,
}

impl MachineSettings {
    /// Loads `machine.json` from `config_dir`, creating the directory and a
    /// new file with a new machine id when there is none. A corrupt or
    /// unreadable file is set aside as `machine.json.bad` and a new one
    /// started: that loses the recent list, and nothing else.
    pub fn load(config_dir: &Path) -> io::Result<Self> {
        fs::create_dir_all(config_dir)?;
        let path = config_dir.join(FILE_NAME);
        let file = match fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<MachineFile>(&bytes) {
                Ok(file) if file.is_valid() => Some(file),
                _ => None,
            },
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                let fresh = MachineFile::fresh()?;
                write_atomically(&path, &fresh)?;
                return Ok(Self { path, file: Mutex::new(fresh), keyring: Keyring::off() });
            }
            Err(_) => None,
        };
        let file = match file {
            Some(file) => file,
            None => {
                log::warn!("{} is unreadable; starting a new one", path.display());
                fs::rename(&path, config_dir.join(format!("{FILE_NAME}.bad")))?;
                let fresh = MachineFile::fresh()?;
                write_atomically(&path, &fresh)?;
                fresh
            }
        };
        Ok(Self { path, file: Mutex::new(file), keyring: Keyring::off() })
    }

    /// The same settings with `keyring` as this computer's keyring.
    pub fn with_keyring(self, keyring: Keyring) -> Self {
        Self { keyring, ..self }
    }

    pub fn keyring(&self) -> &Keyring {
        &self.keyring
    }

    fn lock(&self) -> MutexGuard<'_, MachineFile> {
        self.file.lock().expect("machine settings mutex poisoned")
    }

    /// Applies `change` and writes the file.
    fn update(&self, change: impl FnOnce(&mut MachineFile)) {
        let mut file = self.lock();
        change(&mut file);
        if let Err(err) = write_atomically(&self.path, &file) {
            log::error!("could not write {}: {err}", self.path.display());
        }
    }

    pub fn machine_id(&self) -> String {
        self.lock().machine_id.clone()
    }

    pub fn identity(&self) -> MachineIdentity {
        MachineIdentity { id: self.machine_id(), display_name: host_display_name() }
    }

    /// The recent list, most recent first.
    pub fn recent(&self) -> Vec<RecentEntry> {
        self.lock().recent_databases.clone()
    }

    /// Adds `path` to the top of the recent list, or moves it there and
    /// refreshes what an open learns about it. Whether a passphrase is saved
    /// for it is kept.
    pub fn touch_recent(&self, path: &Path, name: &str, database_id: &str, backup_folder: &Path) {
        self.update(|file| {
            let passphrase_saved = file
                .recent_databases
                .iter()
                .find(|entry| entry.path == path)
                .is_some_and(|entry| entry.passphrase_saved);
            file.recent_databases.retain(|entry| entry.path != path);
            file.recent_databases.insert(
                0,
                RecentEntry {
                    path: path.to_owned(),
                    name: name.to_owned(),
                    last_opened_at: now_utc(),
                    database_id: Some(database_id.to_owned()),
                    backup_folder: Some(backup_folder.to_owned()),
                    passphrase_saved,
                    left_modified_at: None,
                },
            );
        });
    }

    /// Records where the backups of the database at `path` now go, after
    /// its backup location was saved (research.md §8, §11). Nothing happens
    /// when `path` isn't in the list.
    pub fn set_backup_folder(&self, path: &Path, backup_folder: &Path) {
        self.update(|file| {
            if let Some(entry) = file.recent_databases.iter_mut().find(|entry| entry.path == path) {
                entry.backup_folder = Some(backup_folder.to_owned());
            }
        });
    }

    /// Records the modification time of the file at `path` as this
    /// computer leaves it, at a close (FR-040). Nothing happens when `path`
    /// isn't in the list.
    pub fn set_left_modified(&self, path: &Path, left_modified_at: Option<String>) {
        self.update(|file| {
            if let Some(entry) = file.recent_databases.iter_mut().find(|entry| entry.path == path) {
                entry.left_modified_at = left_modified_at;
            }
        });
    }

    /// The recent entry for `path`, if there is one.
    pub fn recent_entry(&self, path: &Path) -> Option<RecentEntry> {
        self.lock().recent_databases.iter().find(|entry| entry.path == path).cloned()
    }

    /// Removes `path` from the recent list, returning the entry that was
    /// there. The database file is never touched (FR-012).
    pub fn remove_recent(&self, path: &Path) -> Option<RecentEntry> {
        let mut removed = None;
        self.update(|file| {
            if let Some(index) = file.recent_databases.iter().position(|entry| entry.path == path) {
                removed = Some(file.recent_databases.remove(index));
            }
        });
        removed
    }

    /// Records whether a passphrase is saved for the database at `path`
    /// (FR-017). Nothing happens when `path` isn't in the list.
    pub fn set_passphrase_saved(&self, path: &Path, saved: bool) {
        self.update(|file| {
            if let Some(entry) = file.recent_databases.iter_mut().find(|entry| entry.path == path) {
                entry.passphrase_saved = saved;
            }
        });
    }

    /// Records that nothing is saved any more for the database
    /// `database_id`, under every path it is listed at: its keyring entry
    /// is shared by them all.
    pub fn clear_passphrase_saved(&self, database_id: &str) {
        self.update(|file| {
            for entry in &mut file.recent_databases {
                if entry.database_id.as_deref() == Some(database_id) {
                    entry.passphrase_saved = false;
                }
            }
        });
    }

    /// Points the recent entry for `path` at `new_path`, where the user found
    /// the file, keeping everything else about it but the time it was left
    /// (FR-012, FR-040). An entry already at `new_path` is merged away, since
    /// entries are identified by path. `None` when `path` isn't in the list.
    pub fn locate_recent(&self, path: &Path, new_path: &Path) -> Option<RecentEntry> {
        let mut located = None;
        self.update(|file| {
            let Some(index) = file.recent_databases.iter().position(|entry| entry.path == path)
            else {
                return;
            };
            let mut entry = file.recent_databases.remove(index);
            entry.path = new_path.to_owned();
            // The file found may be a copy, with a time of its own.
            entry.left_modified_at = None;
            file.recent_databases.retain(|other| other.path != new_path);
            let index = index.min(file.recent_databases.len());
            file.recent_databases.insert(index, entry.clone());
            located = Some(entry);
        });
        located
    }

    /// Keeps `notice` until the chooser next shows its notices.
    pub fn push_notice(&self, notice: ChooserNotice) {
        self.update(|file| file.notices.push(notice));
    }

    /// The waiting notices, which are then gone.
    pub fn take_notices(&self) -> Vec<ChooserNotice> {
        let mut taken = Vec::new();
        self.update(|file| taken = std::mem::take(&mut file.notices));
        taken
    }

    /// How this computer opens a document (FR-011), for every database.
    pub fn document_opening(&self) -> DocumentOpening {
        self.lock().document_opening
    }

    pub fn set_document_opening(&self, value: DocumentOpening) {
        self.update(|file| file.document_opening = value);
    }

    pub fn unfinished_backup(&self) -> Option<UnfinishedBackup> {
        self.lock().unfinished_backup.clone()
    }

    pub fn set_unfinished_backup(&self, record: UnfinishedBackup) {
        self.update(|file| file.unfinished_backup = Some(record));
    }

    pub fn clear_unfinished_backup(&self) {
        self.update(|file| file.unfinished_backup = None);
    }

    pub fn unfinished_backup_move(&self) -> Option<UnfinishedBackupMove> {
        self.lock().unfinished_backup_move.clone()
    }

    pub fn set_unfinished_backup_move(&self, record: UnfinishedBackupMove) {
        self.update(|file| file.unfinished_backup_move = Some(record));
    }

    /// Records the copy the move is making now, or none.
    pub fn set_unfinished_backup_move_partial(&self, partial_path: Option<&Path>) {
        self.update(|file| {
            if let Some(record) = &mut file.unfinished_backup_move {
                record.partial_path = partial_path.map(Path::to_owned);
            }
        });
    }

    pub fn clear_unfinished_backup_move(&self) {
        self.update(|file| file.unfinished_backup_move = None);
    }
}

/// Writes to a temporary file beside `path`, flushes it, then renames it
/// over `path`, so a crash leaves either the old file or the new one.
fn write_atomically(path: &Path, file: &MachineFile) -> io::Result<()> {
    let temp = path.with_extension("json.tmp");
    let json = serde_json::to_vec_pretty(file).map_err(io::Error::other)?;
    let written = File::create(&temp).and_then(|mut out| {
        out.write_all(&json)?;
        out.sync_all()
    });
    match written.and_then(|()| fs::rename(&temp, path)) {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = fs::remove_file(&temp);
            Err(err)
        }
    }
}

/// The host name, without macOS's `.local` suffix, as someone on another
/// computer would recognize it.
fn host_display_name() -> String {
    let host = gethostname::gethostname().to_string_lossy().into_owned();
    let host = if cfg!(target_os = "macos") {
        host.strip_suffix(".local").unwrap_or(&host).to_owned()
    } else {
        host
    };
    let host: String = host.trim().chars().take(MAX_DISPLAY_NAME_CHARS).collect();
    if host.is_empty() { "another computer".to_owned() } else { host }
}
