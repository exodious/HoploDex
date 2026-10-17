/// This build's version, the one in `Cargo.toml`, `tauri.conf.json` and
/// `package.json` (`tests/app_version_test.rs` keeps them equal). It is what
/// `app_state.last_saved_version` records (008
/// research.md §16).
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod app_dirs;
pub mod commands;
pub mod commands_list;
pub mod db;
pub mod models;
pub mod platform;
pub mod services;
pub mod session;
pub mod window_controls;
