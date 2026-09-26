//! `machine.json`: what this computer (this OS account) keeps about
//! databases, and nothing more (FR-013, research.md §6, §11; data-model.md
//! "Machine-local"). It holds paths and names only, never collection data or
//! secrets. The config directory is always passed in, so tests and tools use
//! throwaway ones.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};

use crate::db::{now_utc, random_hex};
use crate::models::database::ChooserNotice;

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
}

/// A backup that was being written when HoploDex stopped (research.md §7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnfinishedBackup {
    pub database_path: PathBuf,
    pub partial_path: PathBuf,
    pub started_at: String,
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
    notices: Vec<ChooserNotice>,
}

impl MachineFile {
    fn fresh() -> io::Result<Self> {
        Ok(Self {
            version: VERSION,
            machine_id: random_hex(16).map_err(io::Error::other)?,
            recent_databases: Vec::new(),
            unfinished_backup: None,
            notices: Vec::new(),
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
pub struct MachineSettings {
    path: PathBuf,
    file: Mutex<MachineFile>,
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
                return Ok(Self { path, file: Mutex::new(fresh) });
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
        Ok(Self { path, file: Mutex::new(file) })
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
                },
            );
        });
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

    pub fn unfinished_backup(&self) -> Option<UnfinishedBackup> {
        self.lock().unfinished_backup.clone()
    }

    pub fn set_unfinished_backup(&self, record: UnfinishedBackup) {
        self.update(|file| file.unfinished_backup = Some(record));
    }

    pub fn clear_unfinished_backup(&self) {
        self.update(|file| file.unfinished_backup = None);
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
    if host.is_empty() {
        "another computer".to_owned()
    } else {
        host
    }
}
