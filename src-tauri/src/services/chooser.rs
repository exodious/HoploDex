//! The file and folder choosers whose answers Rust holds (code scanning
//! alerts `paths/cwe-73` on `import_collection` and `export_collection`).
//!
//! The import's files and the export's folder are picked in a native chooser
//! that Rust opens, and only Rust sees the answer: the page gets a token for
//! it (`session::grants`). So a compromised page can't name a file to read or
//! a folder to write, only ask the user to pick one. [`Chooser`] is the seam:
//! the app's [`NativeChooser`] shows the OS's chooser, and an E2E build's
//! `E2eChooser` answers from a queue file, since no WebDriver can click one.

use std::path::PathBuf;

/// What the user is asked to pick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    /// One spreadsheet, or the firearm and accessory spreadsheets together.
    ImportFiles,
    /// The folder an export is written to.
    ExportFolder,
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
            Pick::ExportFolder => {
                dialog.set_title("Export to folder").blocking_pick_folder().into_iter().collect()
            }
        };
        // `simplified` drops Windows' `\\?\` prefix, as the page's chooser did.
        picked.into_iter().filter_map(|path| path.simplified().into_path().ok()).collect()
    }
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
