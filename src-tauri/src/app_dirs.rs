//! The app's own directories: its config directory (`machine.json`), its
//! cache directory (the passphrase check's scratch files and the opened
//! documents' copies, FR-035), and the documents and home folders a new
//! database is suggested under (research.md §19). Every command and the
//! startup go through here, never through `app.path()` directly.
//!
//! A shipped build asks the OS, through Tauri. An E2E build (`e2e` feature)
//! takes each directory from the sandbox the harness names in its
//! environment instead, and never asks the OS: on Windows the OS's known
//! folders ignore `APPDATA` and the like, so this is how an E2E run stays out
//! of the developer's real `machine.json` and Documents folder (#27, "Never
//! touch the real databases" in CLAUDE.md). An E2E build launched without
//! the sandbox fails to start rather than fall back to the real directories.

#[cfg(any(test, feature = "e2e"))]
use std::path::PathBuf;

pub use resolve::{cache_dir, config_dir, document_dir, home_dir};

#[cfg(not(feature = "e2e"))]
mod resolve {
    use std::path::PathBuf;

    use tauri::{AppHandle, Manager, Runtime};

    pub fn config_dir<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<PathBuf> {
        app.path().app_config_dir()
    }

    pub fn cache_dir<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<PathBuf> {
        app.path().app_cache_dir()
    }

    pub fn document_dir<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<PathBuf> {
        app.path().document_dir()
    }

    pub fn home_dir<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<PathBuf> {
        app.path().home_dir()
    }
}

#[cfg(feature = "e2e")]
mod resolve {
    use std::path::PathBuf;

    use tauri::{AppHandle, Runtime};

    /// The sandbox's stand-in for the OS config directory (`XDG_CONFIG_HOME`,
    /// `%APPDATA%`): the app's own is the app identifier's folder in it.
    pub const CONFIG_HOME: &str = "HOPLODEX_E2E_CONFIG_HOME";
    /// The same for the OS cache directory (`XDG_CACHE_HOME`,
    /// `%LOCALAPPDATA%`).
    pub const CACHE_HOME: &str = "HOPLODEX_E2E_CACHE_HOME";
    /// The sandbox's documents folder, which specs also type locations in.
    pub const DOCUMENTS: &str = "HOPLODEX_E2E_DOCUMENTS";
    /// The sandbox's home folder, set on macOS only: elsewhere the documents
    /// folder is always set, so the chooser never falls back to the home.
    pub const HOME: &str = "HOPLODEX_E2E_HOME";

    pub fn config_dir<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<PathBuf> {
        Ok(sandbox_dir(CONFIG_HOME)?.join(&app.config().identifier))
    }

    pub fn cache_dir<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<PathBuf> {
        Ok(sandbox_dir(CACHE_HOME)?.join(&app.config().identifier))
    }

    pub fn document_dir<R: Runtime>(_app: &AppHandle<R>) -> tauri::Result<PathBuf> {
        sandbox_dir(DOCUMENTS)
    }

    pub fn home_dir<R: Runtime>(_app: &AppHandle<R>) -> tauri::Result<PathBuf> {
        sandbox_dir(HOME)
    }

    fn sandbox_dir(variable: &str) -> tauri::Result<PathBuf> {
        super::sandbox_dir(std::env::var_os(variable)).ok_or_else(|| {
            log::error!("E2E build: {variable} is unset or not an absolute path");
            tauri::Error::UnknownPath
        })
    }
}

/// A sandbox directory from an E2E variable's value: an absolute path, or
/// nothing, so an unset, empty or relative one (which would land in the
/// working directory) never stands in for a real directory.
#[cfg(any(test, feature = "e2e"))]
fn sandbox_dir(value: Option<std::ffi::OsString>) -> Option<PathBuf> {
    value.map(PathBuf::from).filter(|path| path.is_absolute())
}

#[cfg(test)]
mod tests {
    use super::sandbox_dir;
    use std::path::PathBuf;

    #[test]
    fn an_absolute_path_is_the_sandbox_directory() {
        let dir = std::env::temp_dir().join("hoplodex-e2e-sandbox");
        assert_eq!(sandbox_dir(Some(dir.clone().into_os_string())), Some(dir));
    }

    #[test]
    fn an_unset_empty_or_relative_variable_gives_no_directory() {
        assert_eq!(sandbox_dir(None), None);
        assert_eq!(sandbox_dir(Some("".into())), None);
        assert_eq!(sandbox_dir(Some("config".into())), None);
        assert_eq!(sandbox_dir(Some(PathBuf::from(".").join("cache").into_os_string())), None);
    }
}
