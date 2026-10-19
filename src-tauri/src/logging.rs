//! The app's log: what the `log` macros say, written to standard error.
//!
//! Nothing installed a logger before, so the dozens of `log::warn!` and
//! `log::error!` lines in the app (and the one-line statements of 007 and 011:
//! whether WebKit's sandbox is on, what the helpers are confined with) went
//! nowhere. The Flatpak check reads two of them from the app's output
//! (011 US2-6, 011 FR-011, 011 research.md §9, §10, §16), and a person
//! who starts the app from a terminal, or reads the journal for an app started
//! from the desktop, sees the same lines.
//!
//! The helpers and the sandbox probe are this executable started again before
//! this module runs (`main()`), so they log nothing; the parent logs for them.

use std::io::Write;

use log::{Level, LevelFilter, Log, Metadata, Record};

/// Our own crates log at `info` and above; every other crate (Tauri, WebKit's
/// bindings, zbus) only at `warn`, so a dependency's progress notes don't fill it.
fn allowed(level: Level, target: &str) -> bool {
    let ours = target == "hoplodex" || target.starts_with("hoplodex_");
    level <= if ours { Level::Info } else { Level::Warn }
}

/// One line: `[INFO] message` (no time, no target: the journal and a terminal
/// add their own, and nothing here is worth a path in the text).
fn format_line(level: Level, message: &std::fmt::Arguments<'_>) -> String {
    format!("[{level}] {message}\n")
}

struct StderrLogger;

impl Log for StderrLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        allowed(metadata.level(), metadata.target())
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            // One write, so lines from two threads don't mix.
            let _ = std::io::stderr()
                .lock()
                .write_all(format_line(record.level(), record.args()).as_bytes());
        }
    }

    fn flush(&self) {}
}

static LOGGER: StderrLogger = StderrLogger;

/// Installs the logger. Call it once, first in `main()` after the helper
/// modes have been dispatched. A second call, or a logger already installed
/// by something else, changes nothing.
pub fn init() {
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(LevelFilter::Info);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 011 US2-6, 011 FR-011: our own `info` lines are written, a
    /// dependency's are not, and a warning from anywhere is.
    #[test]
    fn our_crates_log_info_and_the_others_only_warnings() {
        assert!(allowed(Level::Info, "hoplodex_lib::services::sandbox"));
        assert!(allowed(Level::Info, "hoplodex"));
        assert!(!allowed(Level::Debug, "hoplodex_lib::session"));
        assert!(!allowed(Level::Info, "tauri::app"));
        assert!(allowed(Level::Warn, "tauri::app"));
        assert!(!allowed(Level::Info, "hoplodexish"));
    }

    #[test]
    fn a_line_is_the_level_and_the_message() {
        assert_eq!(
            format_line(Level::Info, &format_args!("WebKit's sandbox is on")),
            "[INFO] WebKit's sandbox is on\n"
        );
    }
}
