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
//!
//! A line never carries anything from inside a database: no record's values,
//! no photo's or document's original file name (nor the path it was attached
//! from, nor a copy named after it: [`folder_of`]), no draft's text (nor a
//! JSON error that quotes it: [`json_error`]), and never a passphrase. The
//! paths of databases, backups and the app's own files may be named.

use std::io::Write;
use std::path::Path;

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

/// The folder `path` is in, for a line about a file whose name came from a
/// database, such as an opened document's copy, named after the document.
pub fn folder_of(path: &Path) -> std::path::Display<'_> {
    path.parent().unwrap_or(Path::new(".")).display()
}

/// What a line may say about a JSON error: its kind and place, not its
/// message, which quotes the text it was reading (`invalid type: string "…"`).
pub fn json_error(err: &serde_json::Error) -> String {
    format!("{:?} error at line {}, column {}", err.classify(), err.line(), err.column())
}

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
    fn a_copy_named_after_a_document_is_logged_by_its_folder() {
        let copy = Path::new("/cache/opened-documents/7/Bill of sale.pdf");
        assert_eq!(folder_of(copy).to_string(), "/cache/opened-documents/7");
    }

    #[test]
    fn a_json_error_is_logged_without_the_text_it_quotes() {
        let err = serde_json::from_str::<u32>("\"Serial ABC123\"").unwrap_err();
        assert!(err.to_string().contains("ABC123"), "the premise: serde_json quotes it");
        let line = json_error(&err);
        assert!(!line.contains("ABC123"), "{line}");
        assert_eq!(line, "Data error at line 1, column 15");
    }

    #[test]
    fn a_line_is_the_level_and_the_message() {
        assert_eq!(
            format_line(Level::Info, &format_args!("WebKit's sandbox is on")),
            "[INFO] WebKit's sandbox is on\n"
        );
    }
}
