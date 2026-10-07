//! The PDF surface: a child web view in the main window and every per-OS
//! measure on it (research.md §4-§10). One file per OS, as `platform/` does.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

/// Script run in every frame of the surface (research.md §8, §10).
#[allow(dead_code)]
const FRAME_SCRIPT: &str = include_str!("frame_script.js");
