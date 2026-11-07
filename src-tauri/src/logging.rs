//! The app's log: what the `log` macros say, written to standard error and, for
//! HoploDex's own lines while the troubleshooting log is on, to a file in the
//! app's cache directory (013 FR-006, [`file`]).
//!
//! Standard error is what the Flatpak check reads (011 US2-6, 011 FR-011, 011
//! research.md §9, §10, §16) and what a person sees at a terminal or in the
//! journal. It stays at `info` for HoploDex and `warn` for other crates whatever
//! the troubleshooting log does; debug lines go to the file alone (013 FR-026),
//! and other crates' lines never do (013 FR-019).
//!
//! The helpers and the sandbox probe are this executable started again before
//! this module runs (`main()`), so they log nothing and open no file; the parent
//! logs for them (013 FR-031).
//!
//! A line holds only fixed text, HoploDex's own codes, OS error text and codes,
//! durations, and the shape of a path ([`place`]). So no record's values, no
//! name of a database, backup, folder, photo or document, no draft's text (nor a
//! JSON error that quotes it: [`json_error`]), no SQLite message ([`db_error`]),
//! no count of a user's records or row id, and never a passphrase (013 FR-014,
//! FR-016). OS error text can't be vetted, so the home folder is replaced in
//! every line as a backstop ([`clean`]), and what the app tells users about the
//! log says it is designed to leave these out (013 FR-021).

pub mod file;

use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::{Component, MAIN_SEPARATOR, Path, PathBuf, Prefix};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Instant, SystemTime};

use log::{Level, LevelFilter, Log, Metadata, Record};

use crate::models::troubleshooting_log::{LogChoice, LogState, TroubleshootingLog};

/// The variable that sets the log for one run (013 FR-005).
pub const VARIABLE: &str = "HOPLODEX_LOG";

/// How many lines wait in memory until [`start`] can name the folder. Far more
/// than a start logs; past it they are dropped, not grown without bound.
const EARLY_LINES: usize = 256;

/// The extensions a path's shape may keep: the app's own kinds of file, which
/// say nothing about the user. A document's type is not among them.
const KEPT_EXTENSIONS: [&str; 7] = ["hoplodex", "json", "log", "partial", "new", "old", "bad"];

fn ours(target: &str) -> bool {
    target == "hoplodex" || target.starts_with("hoplodex_")
}

/// What standard error takes: our own crates at `info` and above, every other
/// crate (Tauri, WebKit's bindings, zbus) only at `warn`, so a dependency's
/// progress notes don't fill it.
fn allowed(level: Level, target: &str) -> bool {
    level <= if ours(target) { Level::Info } else { Level::Warn }
}

/// One line of standard error: `[INFO] message` (no time, no target: the
/// journal and a terminal add their own, and nothing here is worth a path in
/// the text).
fn format_line(level: Level, message: &str) -> String {
    format!("[{level}] {message}\n")
}

/// One line of the file: seconds since the run began (013 FR-018), so an
/// always-on file isn't a record of when the collection was opened.
fn format_file_line(seconds: f64, level: &str, message: &str) -> String {
    format!("+{seconds:.3} [{level}] {message}\n")
}

/// The first line of each run in the file: the date, the version and the OS,
/// and nothing naming the computer or the user (013 FR-018).
fn header(now: chrono::DateTime<chrono::Utc>, flatpak: bool) -> String {
    format!(
        "--- HoploDex {} on {} {}{}, started {} ---\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        if flatpak { " (Flatpak)" } else { "" },
        now.format("%Y-%m-%d"),
    )
}

/// The backstop for OS error text: the home folder's path becomes `~`, and a
/// line break or control character is escaped, so text can't forge a line or
/// drive a terminal when the file is shown (013 FR-017).
fn clean(message: &str, home: Option<&Path>) -> String {
    let home = home.and_then(Path::to_str).filter(|home| home.len() > 1);
    let replaced;
    let message = match home {
        Some(home) if message.contains(home) => {
            replaced = message.replace(home, "~");
            &replaced
        }
        _ => message,
    };
    if !message.chars().any(char::is_control) {
        return message.to_owned();
    }
    message
        .chars()
        .flat_map(|c| {
            let escaped = c.is_control().then(|| c.escape_default());
            escaped.into_iter().flatten().chain((!c.is_control()).then_some(c))
        })
        .collect()
}

/// Whether a build of `version` keeps the log unless told otherwise: before
/// 1.0.0 and not from it, a pre-release of 1.0.0 included, so the change can't
/// be forgotten at release (013 FR-001, research.md §1).
pub fn default_on(version: &str) -> bool {
    version.split('.').next() == Some("0")
}

/// What applies at a start (013 data-model.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decided {
    pub state: LogState,
    /// Delete what an earlier run left, which no one has asked to keep.
    pub delete_leftovers: bool,
    /// The log runs only because of the version's default, so the user is to
    /// be told, once (013 FR-020).
    pub notice: bool,
}

/// The one place the start's state is worked out (013 research.md §1).
pub fn decide(version: &str, saved: Option<LogChoice>, variable: Option<LogState>) -> Decided {
    let decided = |state, delete_leftovers, notice| Decided { state, delete_leftovers, notice };
    match (variable, saved) {
        (Some(state), _) => decided(state, false, false),
        (None, Some(LogChoice::On)) => decided(LogState::On, false, false),
        (None, Some(LogChoice::Off)) => decided(LogState::Off, true, false),
        (None, None) if default_on(version) => decided(LogState::On, false, true),
        (None, None) => decided(LogState::Off, true, false),
    }
}

/// What the variable says (013 FR-005). An unset or empty variable says
/// nothing; `Err` is a value this version doesn't know, reported unquoted.
fn parse_variable(value: Option<&OsStr>) -> Result<Option<LogState>, ()> {
    match value.map(|value| value.to_str()) {
        None | Some(Some("")) => Ok(None),
        Some(Some("off")) => Ok(Some(LogState::Off)),
        Some(Some("on")) => Ok(Some(LogState::On)),
        Some(Some("debug")) => Ok(Some(LogState::Detailed)),
        Some(_) => Err(()),
    }
}

/// The roots a path's shape may name: where a file is, not whose it is.
fn root_length(names: &[&OsStr]) -> usize {
    let is = |index: usize, name: &str| names.get(index).is_some_and(|n| *n == name);
    if is(0, "run") && is(1, "media") {
        2
    } else if is(0, "run") && is(1, "user") && is(3, "doc") {
        // The Flatpak's document portal, with the user's number hidden below.
        4
    } else if is(0, "media") || is(0, "mnt") || is(0, "Volumes") {
        1
    } else {
        0
    }
}

/// The shape of `path`: its root and depth, every name hidden (013 FR-014,
/// research.md §5).
fn shape(path: &Path, home: Option<&Path>) -> String {
    let separator = MAIN_SEPARATOR.to_string();
    let last_shape = |name: &OsStr| {
        let extension = Path::new(name).extension().and_then(OsStr::to_str);
        match extension.filter(|extension| KEPT_EXTENSIONS.contains(extension)) {
            Some(extension) => format!("*.{extension}"),
            None => "*".to_owned(),
        }
    };
    let (mut start, rest) = match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(below) => ("~".to_owned(), below),
        None => (String::new(), path),
    };
    let mut names: Vec<&OsStr> = Vec::new();
    let mut absolute = false;
    for component in rest.components() {
        match component {
            Component::Prefix(prefix) => {
                start = match prefix.kind() {
                    Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
                        format!("{}:", char::from(letter))
                    }
                    Prefix::UNC(..) | Prefix::VerbatimUNC(..) => r"\\*\*".to_owned(),
                    _ => "*".to_owned(),
                };
            }
            Component::RootDir => absolute = true,
            Component::CurDir => names.push(OsStr::new(".")),
            Component::ParentDir => names.push(OsStr::new("..")),
            Component::Normal(name) => names.push(name),
        }
    }
    let kept = if absolute && start.is_empty() { root_length(&names) } else { 0 };
    let shown: Vec<String> = names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            if *name == "." || *name == ".." {
                name.to_string_lossy().into_owned()
            } else if index < kept {
                // The third of `/run/user/1000/doc` is the user's number.
                if index == 2 { "*".to_owned() } else { name.to_string_lossy().into_owned() }
            } else if index + 1 == names.len() {
                last_shape(name)
            } else {
                "*".to_owned()
            }
        })
        .collect();
    let body = shown.join(&separator);
    match (start.is_empty(), absolute, body.is_empty()) {
        (true, true, _) => format!("{separator}{body}"),
        (true, false, true) => "*".to_owned(),
        (true, false, false) => body,
        (false, _, true) => start,
        (false, _, false) => format!("{start}{separator}{body}"),
    }
}

/// How a line names a path: its shape, such as `~/*/*.hoplodex` (013 FR-014).
/// `tests/log_lines_test.rs` fails on a log macro that prints a path any other
/// way (013 FR-030).
pub fn place(path: &Path) -> String {
    shape(path, HOME.get().and_then(|home| home.as_deref()))
}

/// What a line may say about a JSON error: its kind and place, not its
/// message, which quotes the text it was reading (`invalid type: string "…"`).
pub fn json_error(err: &serde_json::Error) -> String {
    format!("{:?} error at line {}, column {}", err.classify(), err.line(), err.column())
}

/// What a line may say about a database error: its kind and SQLite's code, not
/// its text, which can quote what SQLite was given (013 FR-016).
pub fn db_error(err: &rusqlite::Error) -> String {
    use rusqlite::Error as E;
    match err {
        E::SqliteFailure(failure, _) => {
            format!("SQLite {:?} ({})", failure.code, failure.extended_code)
        }
        E::SqlInputError { error, .. } => {
            format!("SQL input: SQLite {:?} ({})", error.code, error.extended_code)
        }
        E::QueryReturnedNoRows => "no rows".to_owned(),
        E::QueryReturnedMoreThanOneRow => "more than one row".to_owned(),
        E::ExecuteReturnedResults => "a statement returned rows".to_owned(),
        E::InvalidQuery => "an invalid query".to_owned(),
        E::InvalidColumnType(index, _, kind) => format!("column {index} is {kind}"),
        E::InvalidColumnIndex(index) => format!("no column {index}"),
        E::InvalidColumnName(_) => "no such column".to_owned(),
        E::InvalidParameterName(_) => "no such parameter".to_owned(),
        E::FromSqlConversionFailure(index, kind, _) => {
            format!("column {index} could not be read from {kind}")
        }
        E::IntegralValueOutOfRange(index, _) => format!("column {index} is out of range"),
        E::Utf8Error(..) => "text that is not UTF-8".to_owned(),
        E::NulError(_) => "text holding a NUL".to_owned(),
        E::ToSqlConversionFailure(_) => "a value could not be stored".to_owned(),
        _ => "another database error".to_owned(),
    }
}

/// The state behind the log's second output. One instance serves the app; tests
/// make their own, with throwaway folders.
pub struct Output {
    home: Option<PathBuf>,
    /// Whether debug lines are wanted, so the macros can skip the lock.
    detailed: AtomicBool,
    started: Instant,
    /// What the variable said, until [`Output::start`] uses it.
    variable: Option<LogState>,
    inner: Mutex<Inner>,
}

struct Inner {
    state: LogState,
    /// Lines held until [`Output::start`] decides, when it hasn't yet.
    early: Option<Vec<String>>,
    folder: Option<PathBuf>,
    file: Option<file::LogFile>,
    failed: bool,
}

impl Output {
    pub fn new(home: Option<PathBuf>, variable: Option<LogState>) -> Self {
        let state = variable.unwrap_or(LogState::On);
        let holding = state != LogState::Off;
        Self {
            home,
            detailed: AtomicBool::new(state == LogState::Detailed),
            started: Instant::now(),
            variable,
            inner: Mutex::new(Inner {
                state,
                early: holding.then(Vec::new),
                folder: None,
                file: None,
                failed: false,
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn detailed(&self) -> bool {
        self.detailed.load(Ordering::Relaxed)
    }

    /// Starts the file for what [`decide`] says, once `folder` (the app's cache
    /// directory's `logs`, or `None` if it can't be named) and the saved choice
    /// are known. Returns what was decided. A log that can't be written never
    /// stops the app (013 FR-011).
    pub fn start(&self, folder: Option<PathBuf>, saved: Option<LogChoice>) -> Decided {
        let decided = decide(env!("CARGO_PKG_VERSION"), saved, self.variable);
        let to_tidy = folder.clone();
        {
            let mut inner = self.lock();
            inner.folder = folder;
            inner.state = decided.state;
        }
        self.detailed.store(decided.state == LogState::Detailed, Ordering::Relaxed);
        // With no lock held: deleting can log.
        if let Some(folder) = &to_tidy {
            if decided.delete_leftovers {
                file::delete_all(folder);
            } else if decided.state != LogState::Off {
                file::delete_if_old(folder, SystemTime::now());
            }
        }
        let mut inner = self.lock();
        let early = inner.early.take().unwrap_or_default();
        if decided.state != LogState::Off {
            self.open(&mut inner);
            for line in early {
                self.write(&mut inner, &line);
            }
        }
        decided
    }

    fn open(&self, inner: &mut Inner) {
        inner.failed = false;
        let Some(folder) = inner.folder.clone() else {
            inner.failed = true;
            return;
        };
        match file::LogFile::open(&folder) {
            Ok(opened) => {
                inner.file = Some(opened);
                let flatpak = Path::new(crate::services::sandbox::FLATPAK_INFO).exists();
                self.write(inner, &header(chrono::Utc::now(), flatpak));
            }
            Err(_) => inner.failed = true,
        }
    }

    /// A failed write ends the file for this run; the app carries on.
    fn write(&self, inner: &mut Inner, line: &str) {
        if let Some(file) = &mut inner.file
            && file.write_line(line).is_err()
        {
            inner.file = None;
            inner.failed = true;
        }
    }

    fn file_line(&self, level: &str, message: &str) -> String {
        format_file_line(self.started.elapsed().as_secs_f64(), level, message)
    }

    /// Takes one of our own lines, if the file is to have it.
    fn accept(&self, level: Level, message: &str) {
        if level > Level::Info && !self.detailed() {
            return;
        }
        let line = self.file_line(level.as_str(), message);
        let mut inner = self.lock();
        if let Some(early) = &mut inner.early {
            if early.len() < EARLY_LINES {
                early.push(line);
            }
        } else {
            self.write(&mut inner, &line);
        }
    }

    /// A panic's location, never its message, which can quote a value
    /// (013 FR-027). Skipped if the logger is in use, rather than risk a
    /// deadlock.
    fn panic_line(&self, location: &str) {
        let Ok(mut inner) = self.inner.try_lock() else { return };
        let line = self.file_line("PANIC", location);
        self.write(&mut inner, &line);
    }

    /// Changes the state as 013 contracts/tauri-commands.md says, and returns
    /// the choice to save, if any. A raise or a return never saves, so someone
    /// following the version's default doesn't become opted in (013
    /// research.md §1). A raise while off is refused here, whatever the page
    /// shows, because a later start would delete what it gathered.
    pub fn set(&self, wanted: LogState) -> Option<LogChoice> {
        let mut inner = self.lock();
        match (inner.state, wanted) {
            (LogState::Off, LogState::On) => {
                inner.state = LogState::On;
                self.open(&mut inner);
                Some(LogChoice::On)
            }
            (LogState::On, LogState::Detailed) | (LogState::Detailed, LogState::On) => {
                inner.state = wanted;
                self.detailed.store(wanted == LogState::Detailed, Ordering::Relaxed);
                None
            }
            (LogState::On | LogState::Detailed, LogState::Off) => {
                inner.state = LogState::Off;
                inner.file = None;
                inner.failed = false;
                self.detailed.store(false, Ordering::Relaxed);
                let folder = inner.folder.clone();
                // With no lock held: deleting can log.
                drop(inner);
                if let Some(folder) = folder {
                    file::delete_all(&folder);
                }
                Some(LogChoice::Off)
            }
            _ => None,
        }
    }

    pub fn status(&self) -> TroubleshootingLog {
        let inner = self.lock();
        let path = inner.folder.as_ref().map(|folder| folder.join(file::CURRENT));
        TroubleshootingLog {
            state: inner.state,
            exists: path.as_deref().is_some_and(crate::services::scratch::is_plain_file),
            path: path.map(|path| path.to_string_lossy().into_owned()),
            failed: inner.failed,
        }
    }

    /// The file's path, for showing it. Rust's, never the page's (013 FR-024).
    pub fn current_path(&self) -> Option<PathBuf> {
        self.lock().folder.as_ref().map(|folder| folder.join(file::CURRENT))
    }
}

/// Both outputs. `enabled` is what the macros ask before formatting anything.
struct AppLog(Output);

impl Log for AppLog {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        allowed(metadata.level(), metadata.target())
            || (ours(metadata.target()) && self.0.detailed())
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let message = clean(&record.args().to_string(), self.0.home.as_deref());
        if allowed(record.level(), record.target()) {
            // One write, so lines from two threads don't mix.
            let _ = std::io::stderr()
                .lock()
                .write_all(format_line(record.level(), &message).as_bytes());
        }
        if ours(record.target()) {
            self.0.accept(record.level(), &message);
        }
    }

    fn flush(&self) {}
}

static HOME: OnceLock<Option<PathBuf>> = OnceLock::new();
static LOGGER: OnceLock<AppLog> = OnceLock::new();

/// Installs the logger with the real home folder and variable. Call it once,
/// first in `main()` after the helper modes have been dispatched. A second call,
/// or a logger already installed by something else, changes nothing.
pub fn init() {
    init_with(std::env::home_dir(), std::env::var_os(VARIABLE));
}

/// [`init`], with what it reads from the environment given, for tests.
pub fn init_with(home: Option<PathBuf>, variable: Option<OsString>) {
    let parsed = parse_variable(variable.as_deref());
    let _ = HOME.set(home.clone());
    let logger = LOGGER.get_or_init(|| AppLog(Output::new(home, parsed.unwrap_or(None))));
    if log::set_logger(logger).is_ok() {
        apply_level();
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if let (Some(logger), Some(location)) = (LOGGER.get(), info.location()) {
                logger.0.panic_line(&clean(
                    &format!("at {}:{}", location.file(), location.line()),
                    logger.0.home.as_deref(),
                ));
            }
            previous(info);
        }));
        if parsed.is_err() {
            log::warn!("{VARIABLE} has a value this version doesn't know, so it is ignored");
        }
    }
}

fn apply_level() {
    let detailed = LOGGER.get().is_some_and(|logger| logger.0.detailed());
    log::set_max_level(if detailed { LevelFilter::Debug } else { LevelFilter::Info });
}

/// Starts the file ([`Output::start`]) with what the app now knows. Does nothing
/// to a process with no logger, returning the state of a log that is off.
pub fn start(folder: Option<PathBuf>, saved: Option<LogChoice>) -> Decided {
    let Some(logger) = LOGGER.get() else {
        return Decided { state: LogState::Off, delete_leftovers: false, notice: false };
    };
    let decided = logger.0.start(folder, saved);
    apply_level();
    decided
}

/// Changes the state ([`Output::set`]), returning the choice to save.
pub fn set_state(wanted: LogState) -> Option<LogChoice> {
    let choice = LOGGER.get().and_then(|logger| logger.0.set(wanted));
    apply_level();
    choice
}

pub fn status() -> TroubleshootingLog {
    LOGGER.get().map_or(
        TroubleshootingLog { state: LogState::Off, path: None, exists: false, failed: false },
        |logger| logger.0.status(),
    )
}

pub fn current_path() -> Option<PathBuf> {
    LOGGER.get().and_then(|logger| logger.0.current_path())
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
            format_line(Level::Info, "WebKit's sandbox is on"),
            "[INFO] WebKit's sandbox is on\n"
        );
    }

    /// 013 FR-001: the default can't outlive 0.x, pre-releases of 1.0.0 included.
    #[test]
    fn the_default_is_on_only_before_1_0_0() {
        assert!(default_on("0.1.0"));
        assert!(default_on("0.99.3"));
        assert!(!default_on("1.0.0"));
        assert!(!default_on("1.0.0-rc.1"));
        assert!(!default_on("2.3.4"));
    }

    /// 013 data-model.md, "How the state is decided at start".
    #[test]
    fn the_start_is_decided_by_the_variable_then_the_choice_then_the_version() {
        let check = |version, saved, variable, state, delete_leftovers, notice| {
            assert_eq!(
                decide(version, saved, variable),
                Decided { state, delete_leftovers, notice },
                "{version} {saved:?} {variable:?}"
            );
        };
        use LogChoice as C;
        use LogState as S;
        check("0.1.0", None, None, S::On, false, true);
        check("1.0.0", None, None, S::Off, true, false);
        check("0.1.0", Some(C::On), None, S::On, false, false);
        check("1.0.0", Some(C::On), None, S::On, false, false);
        check("0.1.0", Some(C::Off), None, S::Off, true, false);
        check("1.0.0", Some(C::Off), None, S::Off, true, false);
        check("0.1.0", Some(C::On), Some(S::Off), S::Off, false, false);
        check("1.0.0", None, Some(S::Off), S::Off, false, false);
        check("1.0.0", Some(C::Off), Some(S::On), S::On, false, false);
        check("0.1.0", None, Some(S::Detailed), S::Detailed, false, false);
    }

    /// 013 US4-4.
    #[test]
    fn the_variable_takes_three_values_and_reports_anything_else() {
        let parse = |value: &str| parse_variable(Some(OsStr::new(value)));
        assert_eq!(parse_variable(None), Ok(None));
        assert_eq!(parse(""), Ok(None));
        assert_eq!(parse("off"), Ok(Some(LogState::Off)));
        assert_eq!(parse("on"), Ok(Some(LogState::On)));
        assert_eq!(parse("debug"), Ok(Some(LogState::Detailed)));
        assert_eq!(parse("trace"), Err(()));
        assert_eq!(parse("DEBUG"), Err(()));
    }

    /// 013 US2-1.
    #[cfg(unix)]
    #[test]
    fn a_path_under_home_shows_only_its_depth() {
        let home = Path::new("/home/planted");
        let at = |path: &str| shape(Path::new(path), Some(home));
        assert_eq!(at("/home/planted/Guns/Smith family.hoplodex"), "~/*/*.hoplodex");
        assert_eq!(at("/home/planted/x.hoplodex"), "~/*.hoplodex");
        assert_eq!(at("/home/planted"), "~");
        assert_eq!(at("/home/planted/Secret/Bill of sale.pdf"), "~/*/*");
        assert_eq!(at("/home/planted/.config/id/machine.json"), "~/*/*/*.json");
        assert_eq!(at("/home/planted/a/b.hoplodex.partial"), "~/*/*.partial");
    }

    /// 013 US2-2: where it is, not whose or what it is called.
    #[cfg(unix)]
    #[test]
    fn a_path_elsewhere_keeps_only_a_known_root() {
        let at = |path: &str| shape(Path::new(path), Some(Path::new("/home/planted")));
        assert_eq!(at("/run/media/planted/USB/Guns.hoplodex"), "/run/media/*/*/*.hoplodex");
        assert_eq!(at("/media/planted/USB/Guns.hoplodex"), "/media/*/*/*.hoplodex");
        assert_eq!(at("/mnt/nas/Guns.hoplodex"), "/mnt/*/*.hoplodex");
        assert_eq!(at("/Volumes/Planted/Guns.hoplodex"), "/Volumes/*/*.hoplodex");
        assert_eq!(at("/run/user/1000/doc/ab12/Guns.hoplodex"), "/run/user/*/doc/*/*.hoplodex");
        assert_eq!(at("/opt/planted/Guns.hoplodex"), "/*/*/*.hoplodex");
        assert_eq!(at("/media"), "/media");
        assert_eq!(at("/"), "/");
        assert_eq!(at("planted/Guns.hoplodex"), "*/*.hoplodex");
        assert_eq!(at("../planted"), "../*");
        assert_eq!(at(""), "*");
    }

    /// 013 US2-2: a network share's names are hidden with the rest.
    #[cfg(windows)]
    #[test]
    fn a_windows_path_keeps_only_its_drive_letter() {
        let home = Path::new(r"C:\Users\planted");
        let at = |path: &str| shape(Path::new(path), Some(home));
        assert_eq!(at(r"C:\Users\planted\Guns\Smith.hoplodex"), r"~\*\*.hoplodex");
        assert_eq!(at(r"D:\Guns\Smith.hoplodex"), r"D:\*\*.hoplodex");
        assert_eq!(at(r"\\server\share\Guns\Smith.hoplodex"), r"\\*\*\*\*.hoplodex");
        assert_eq!(at(r"\\?\C:\Guns\Smith.hoplodex"), r"C:\*\*.hoplodex");
    }

    #[test]
    fn a_database_error_is_logged_without_its_text() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let err = conn.execute("SELECT planted_column FROM planted_table", []).unwrap_err();
        assert!(err.to_string().contains("planted"), "the premise: SQLite quotes it");
        let line = db_error(&err);
        assert!(!line.contains("planted"), "{line}");
        assert!(line.contains("SQLite"), "{line}");
        let failure = rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(19),
            Some("planted value".to_owned()),
        );
        assert_eq!(db_error(&failure), "SQLite ConstraintViolation (19)");
    }

    /// 013 FR-017.
    #[test]
    fn a_line_loses_the_home_folder_and_its_control_characters() {
        let home = Some(Path::new("/home/planted"));
        assert_eq!(
            clean("cannot open /home/planted/a: Permission denied", home),
            "cannot open ~/a: Permission denied"
        );
        assert_eq!(clean("one\ntwo\r\u{1b}[31m", None), r"one\ntwo\r\u{1b}[31m");
        assert_eq!(clean("déjà vu", Some(Path::new("/"))), "déjà vu");
    }

    /// 013 FR-018: nothing naming the computer or the user.
    #[test]
    fn the_header_gives_the_date_version_and_os() {
        let date = chrono::DateTime::parse_from_rfc3339("2026-11-07T23:59:58Z").unwrap();
        let line = header(date.into(), true);
        assert!(line.contains(env!("CARGO_PKG_VERSION")), "{line}");
        assert!(line.contains(std::env::consts::OS), "{line}");
        assert!(line.contains("(Flatpak)"), "{line}");
        assert!(line.contains("started 2026-11-07 "), "{line}");
        assert!(!line.contains("23:59"), "{line}");
        assert_eq!(
            format_file_line(4.31, "DEBUG", "opening ~/*.hoplodex"),
            "+4.310 [DEBUG] opening ~/*.hoplodex\n"
        );
    }
}
