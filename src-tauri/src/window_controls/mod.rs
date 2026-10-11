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
