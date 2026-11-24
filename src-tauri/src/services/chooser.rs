//! The file and folder choosers whose answers Rust holds (code scanning
//! alerts `paths/cwe-73` on `import_collection` and `export_collection`).
//!
//! The import's files, the export's folder, a new database's folder, the
//! backup location, the folder for the backup before an upgrade and a
//! database file to open are picked in a native chooser that Rust opens, and
//! only Rust holds the answer: the page gets a token for it
//! (`session::grants`). So a compromised page can't name a file to read or a
//! folder to write, only ask the user to pick one. [`Chooser`] is the seam:
//! the app's [`NativeChooser`] shows the OS's chooser, and an E2E build's
//! `E2eChooser` answers from a queue file, since no WebDriver can click one.
//! Inside a Flatpak the app's is [`SandboxChooser`], which opens the
//! Documents portal's chooser only where a path outside the sandbox can be used.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;

use crate::commands::CommandError;
use crate::session::grants::PickKind;
use crate::session::{Session, SessionId};

/// What the user is asked to pick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    /// One spreadsheet, or the firearm and accessory spreadsheets together.
    ImportFiles,
    /// The folder an export is written to.
    ExportFolder,
    /// The folder a new database goes in.
    DatabaseFolder,
    /// Where the open database's backups go.
    BackupFolder,
    /// The folder for the one backup made before an upgrade.
    UpgradeBackupFolder,
    /// A database file to open.
    DatabaseFile,
}

/// Asks the user. Blocks until they answer, so a caller must not hold the
/// session's lock, nor run on the main thread. Cancelling, or a chooser that
/// couldn't be shown, is no paths.
pub trait Chooser: Send + Sync {
    fn pick(&self, pick: Pick) -> Vec<PathBuf>;
}

/// The OS's chooser, on `tauri-plugin-dialog`, parented to the main window.
pub struct NativeChooser<R: tauri::Runtime> {
    app: tauri::AppHandle<R>,
}

impl<R: tauri::Runtime> NativeChooser<R> {
    pub fn new(app: tauri::AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: tauri::Runtime> Chooser for NativeChooser<R> {
    fn pick(&self, pick: Pick) -> Vec<PathBuf> {
        use tauri_plugin_dialog::{DialogExt, FilePath};

        let dialog = self.app.dialog().file();
        #[cfg(any(windows, target_os = "macos"))]
        let dialog = match tauri::Manager::get_window(&self.app, "main") {
            Some(window) => dialog.set_parent(&window),
            None => dialog,
        };
        let picked: Vec<FilePath> = match pick {
            Pick::ImportFiles => dialog
                .set_title("Import spreadsheet")
                .add_filter("Spreadsheet", &["csv", "xlsx"])
                .blocking_pick_files()
                .unwrap_or_default(),
            Pick::DatabaseFile => dialog
                .set_title("Open a database file")
                .add_filter("HoploDex databases", &["hoplodex"])
                .add_filter("All files", &["*"])
                .blocking_pick_file()
                .into_iter()
                .collect(),
            Pick::ExportFolder
            | Pick::DatabaseFolder
            | Pick::BackupFolder
            | Pick::UpgradeBackupFolder => {
                dialog.set_title(folder_title(pick)).blocking_pick_folder().into_iter().collect()
            }
        };
        // `simplified` drops Windows' `\\?\` prefix, as the page's chooser did.
        picked.into_iter().filter_map(|path| path.simplified().into_path().ok()).collect()
    }
}

fn folder_title(pick: Pick) -> &'static str {
    match pick {
        Pick::DatabaseFolder => "Choose a folder for the new database",
        Pick::BackupFolder => "Choose where to keep this database's backups",
        Pick::UpgradeBackupFolder => "Choose a folder for this backup",
        _ => "Export to folder",
    }
}

/// The chooser inside a Flatpak (code scanning alert `flatpak/cwe-272` on
/// `Reach::check_portal`). The OS's chooser there is the Documents portal's,
/// and each pick in it gives the app a lasting, recursive read and write grant
/// to what was picked, which the app can't revoke, made even where Reach then
/// refuses the path. A database's folder or file, the backup location and the
/// folder for the backup before an upgrade are picked instead in GTK's own
/// chooser, which sees only the sandbox and makes no grant, opened on
/// `folders` (the writable grants). Of the choosers Rust opens, only the
/// import's files and the export's folder still go through the portal; the
/// page's file input for an attached photo or document does too.
#[cfg(target_os = "linux")]
pub struct SandboxChooser<R: tauri::Runtime> {
    portal: NativeChooser<R>,
    folders: Vec<PathBuf>,
}

#[cfg(target_os = "linux")]
impl<R: tauri::Runtime> SandboxChooser<R> {
    pub fn new(app: tauri::AppHandle<R>, folders: Vec<PathBuf>) -> Self {
        Self { portal: NativeChooser::new(app), folders }
    }
}

#[cfg(target_os = "linux")]
impl<R: tauri::Runtime> Chooser for SandboxChooser<R> {
    fn pick(&self, pick: Pick) -> Vec<PathBuf> {
        if uses_the_portal(pick) {
            return self.portal.pick(pick);
        }
        let (sent, answer) = std::sync::mpsc::channel();
        let window = tauri::Manager::get_window(&self.portal.app, "main");
        let folders = self.folders.clone();
        let shown = self.portal.app.run_on_main_thread(move || {
            let parent = window.and_then(|window| window.gtk_window().ok());
            show_in_sandbox(parent.as_ref(), pick, &folders, sent);
        });
        if shown.is_err() {
            return Vec::new();
        }
        answer.recv().unwrap_or_default()
    }
}

/// Whether the Flatpak still picks this through the Documents portal: only
/// where a path outside the sandbox can be used (an import's files, an
/// export's folder).
#[cfg(target_os = "linux")]
fn uses_the_portal(pick: Pick) -> bool {
    matches!(pick, Pick::ImportFiles | Pick::ExportFolder)
}

/// Shows GTK's own chooser, modal to `parent`, and sends what was picked.
/// Runs on the main thread.
#[cfg(target_os = "linux")]
fn show_in_sandbox(
    parent: Option<&gtk::ApplicationWindow>,
    pick: Pick,
    folders: &[PathBuf],
    sent: std::sync::mpsc::Sender<Vec<PathBuf>>,
) {
    use gtk::prelude::*;
    use gtk::{FileChooserAction, ResponseType};

    let (title, action, accept) = match pick {
        Pick::DatabaseFile => ("Open a database file", FileChooserAction::Open, "_Open"),
        _ => (folder_title(pick), FileChooserAction::SelectFolder, "_Select"),
    };
    let dialog = gtk::FileChooserDialog::with_buttons(
        Some(title),
        parent,
        action,
        &[("_Cancel", ResponseType::Cancel), (accept, ResponseType::Accept)],
    );
    dialog.set_modal(true);
    let folders: Vec<&PathBuf> = folders.iter().filter(|folder| folder.is_dir()).collect();
    for folder in &folders {
        let _ = dialog.add_shortcut_folder(folder);
    }
    if let Some(first) = folders.first() {
        dialog.set_current_folder(first);
    }
    if pick == Pick::DatabaseFile {
        for (name, pattern) in [("HoploDex databases", "*.hoplodex"), ("All files", "*")] {
            let filter = gtk::FileFilter::new();
            filter.set_name(Some(name));
            filter.add_pattern(pattern);
            dialog.add_filter(filter);
        }
    }
    dialog.connect_response(move |dialog, response| {
        let picked = if response == ResponseType::Accept { dialog.filename() } else { None };
        let _ = sent.send(picked.into_iter().collect());
        // SAFETY: the dialog is ours and nothing uses it after this.
        unsafe { dialog.destroy() };
    });
    dialog.show();
}

/// A file or folder the user picked: the token a command takes it by, and its
/// path for the dialog to show.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChosenPath {
    pub token: String,
    pub path: String,
}

/// Shows the OS's chooser for `pick` on a blocking thread, since it waits for
/// the user, and holds what was picked under `id` as `kind`. `still_for` says
/// whether what it was asked for is still there (the open session, nothing
/// open, the waiting database): a lock, close or open while the chooser was
/// up ends it, and nothing is held.
pub async fn choose(
    session: &Session,
    id: SessionId,
    still_for: impl Fn() -> bool,
    chooser: Arc<dyn Chooser>,
    pick: Pick,
    kind: PickKind,
) -> Result<Vec<ChosenPath>, CommandError> {
    if !still_for() {
        return Err(CommandError::database_closed());
    }
    let picked =
        tauri::async_runtime::spawn_blocking(move || chooser.pick(pick)).await.map_err(|_| {
            // Not the error's text: it can carry a panic's message.
            log::error!("the chooser's thread failed");
            CommandError::new("INTERNAL_ERROR", "The chooser couldn't be shown.")
        })?;
    if !still_for() {
        return Err(CommandError::database_closed());
    }
    picked
        .into_iter()
        .map(|path| {
            let shown = path.to_string_lossy().into_owned();
            let token = session.hold_pick(id, kind, path)?;
            Ok(ChosenPath { token, path: shown })
        })
        .collect()
}

/// The E2E build's stand-in for the chooser: each pick takes the top line of
/// the file named by `HOPLODEX_E2E_CHOOSER`, a JSON array of paths (`[]` is a
/// cancel), and an empty or missing queue is a cancel. Never in a release
/// build (`scripts/check-no-webdriver.mjs`).
#[cfg(feature = "e2e")]
pub struct E2eChooser;

#[cfg(feature = "e2e")]
impl Chooser for E2eChooser {
    fn pick(&self, _pick: Pick) -> Vec<PathBuf> {
        let Some(queue) = std::env::var_os("HOPLODEX_E2E_CHOOSER") else { return Vec::new() };
        let Ok(text) = std::fs::read_to_string(&queue) else { return Vec::new() };
        let mut lines = text.lines().filter(|line| !line.trim().is_empty());
        let Some(first) = lines.next() else { return Vec::new() };
        let rest: String = lines.map(|line| format!("{line}\n")).collect();
        if let Err(err) = std::fs::write(&queue, rest) {
            log::warn!("could not take an answer off the chooser queue: {err}");
        }
        serde_json::from_str(first).unwrap_or_else(|err| {
            log::warn!("a queued chooser answer isn't JSON: {}", crate::logging::json_error(&err));
            Vec::new()
        })
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::{Pick, uses_the_portal};

    /// Only picks whose portal paths Reach can use make a Documents-portal
    /// grant (code scanning alert `flatpak/cwe-272` on `Reach::check_portal`):
    /// a database's folder or file and a backup folder are refused there, so
    /// they never go through it.
    #[test]
    fn only_the_import_and_export_go_through_the_portal() {
        for pick in [Pick::ImportFiles, Pick::ExportFolder] {
            assert!(uses_the_portal(pick), "{pick:?}");
        }
        for pick in [
            Pick::DatabaseFolder,
            Pick::DatabaseFile,
            Pick::BackupFolder,
            Pick::UpgradeBackupFolder,
        ] {
            assert!(!uses_the_portal(pick), "{pick:?}");
        }
    }
}
