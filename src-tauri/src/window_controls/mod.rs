//! Browser controls, native side (research.md §13, §14, §15): removing the web
//! view's default context menu and developer-tools commands, reload and
//! navigation shortcuts, per OS, outside development builds. One module per
//! OS, as `platform/` does.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

use tauri::{Runtime, WebviewWindow};

/// Whether this build installs the filters: release and E2E builds do, a
/// development build keeps Inspect Element and reload (FR-014, research.md
/// §15). `profile.e2e` inherits release, so it has no debug assertions.
pub const INSTALLED: bool = cfg!(not(debug_assertions));

/// Installs the native browser controls on the `main` web view's window, in
/// builds that have them ([`INSTALLED`]); in others it does nothing.
/// `log_items` logs each context menu item offered and whether it was kept
/// (`log::info!`), which the window controls check (research.md §18) asks
/// for and the app does not. Called once from `setup()`, after the window
/// exists.
///
/// The per-OS code is compiled in every build, so a development build's
/// lints and tests cover it; only the call is conditional.
pub fn apply<R: Runtime>(window: &WebviewWindow<R>, log_items: bool) -> tauri::Result<()> {
    if !INSTALLED {
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    return linux::apply(window, log_items);
    #[cfg(target_os = "macos")]
    return macos::apply(window, log_items);
    #[cfg(windows)]
    return windows::apply(window, log_items);
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        let _ = (window, log_items);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::INSTALLED;

    /// FR-014: filters compiled for release and e2e only. `cargo test
    /// --profile e2e` (which has no debug assertions) reports it true; the
    /// default test profile reports it false.
    #[test]
    fn filters_compiled_for_release_and_e2e_only() {
        assert_eq!(INSTALLED, cfg!(not(debug_assertions)));
    }
}
