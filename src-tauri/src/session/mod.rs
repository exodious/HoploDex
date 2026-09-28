//! The open database, if any (research.md §13; data-model.md "In memory:
//! the session"). Replaces feature 001's always-open `DbHandle`: every
//! command reaches the database through [`Session::read`] or
//! [`Session::write`], which refuse when nothing is open. There is still one
//! connection and no pool.

pub mod clock;
pub mod fingerprint;
pub mod idle;
pub mod lifecycle;
pub mod operations;
pub mod pending;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use rusqlite::{Connection, InterruptHandle};

use crate::commands::CommandError;
use crate::models::database::{ChooserNotice, DatabaseNotes, Draft, LockSettings};
use crate::services::machine_settings::MachineSettings;
use clock::{Clock, SystemClock};
use fingerprint::{FileFingerprint, FingerprintCheck};
use idle::IdleClock;
use operations::{Operations, StoppedOperation};

/// An open database: its connection, holding the file's exclusive lock and
/// (inside SQLCipher) the derived key, and what the session knows about it.
/// The passphrase itself is not here: the backend never keeps one (FR-007).
pub struct OpenDatabase {
    pub conn: Connection,
    pub path: PathBuf,
    /// The file name without its extension.
    pub name: String,
    pub database_id: String,
    /// Stops a long statement on `conn` from another thread.
    pub interrupt: InterruptHandle,
    /// The open form's unsaved input, mirrored from the frontend and written
    /// only by a lock or an OS shutdown (research.md §16).
    pub staged_draft: Option<Draft>,
    /// The database holds pending changes not yet resumed or discarded
    /// (FR-039).
    pub pending_unresolved: bool,
    /// The file as this session last left it, to notice a take-over
    /// (research.md §6).
    pub fingerprint: FileFingerprint,
    /// The file stopped being reachable while open. Every later write is
    /// refused with `DATABASE_UNAVAILABLE`, whatever the fingerprint says
    /// once it is back, and the close writes nothing (research.md §6).
    pub storage_lost: bool,
    /// The notes this open reports once: a backup opened directly, a
    /// restore. `disk_encryption` is not used here: it is kept in the file.
    pub notes: DatabaseNotes,
    /// The lock settings kept in the file, for the idle clock (FR-034).
    pub lock_settings: LockSettings,
}

impl OpenDatabase {
    /// Wraps a connection from `db::create_database` or `db::open_database`,
    /// taking the file's fingerprint as the open left it.
    pub fn new(conn: Connection, path: &Path) -> Result<Self, CommandError> {
        let database_id: String = conn
            .query_row("SELECT database_id FROM app_state", [], |row| row.get(0))
            .map_err(CommandError::from_db)?;
        let pending_unresolved: bool = conn
            .query_row("SELECT EXISTS (SELECT 1 FROM pending_changes)", [], |row| row.get(0))
            .map_err(CommandError::from_db)?;
        let lock_settings = conn
            .query_row(
                "SELECT idle_lock_enabled, idle_lock_minutes, lock_on_screen_lock
                 FROM collection_settings",
                [],
                |row| {
                    Ok(LockSettings {
                        idle_enabled: row.get(0)?,
                        idle_minutes: row.get(1)?,
                        on_screen_lock: row.get(2)?,
                    })
                },
            )
            .map_err(CommandError::from_db)?;
        let fingerprint =
            FileFingerprint::capture(path).map_err(|_| CommandError::database_unavailable(path))?;
        let interrupt = conn.get_interrupt_handle();
        Ok(Self {
            conn,
            path: path.to_owned(),
            name: database_name(path),
            database_id,
            interrupt,
            staged_draft: None,
            pending_unresolved,
            fingerprint,
            storage_lost: false,
            notes: DatabaseNotes::default(),
            lock_settings,
        })
    }
}

/// A database's name: its file name without the extension.
pub fn database_name(path: &Path) -> String {
    path.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Tauri state: the open database, or none, and where the session reports
/// what happens to it.
pub struct Session {
    open: Mutex<Option<OpenDatabase>>,
    events: Arc<dyn SessionEvents>,
    /// Where `open_document` puts decrypted copies, deleted at every close
    /// (FR-022). `None` where nothing makes them.
    opened_documents_dir: Option<PathBuf>,
    /// The database most recently closed in this run, which the chooser
    /// selects (FR-033).
    last_closed: Mutex<Option<PathBuf>>,
    /// The long-running operation, if any (research.md §13).
    operations: Arc<Operations>,
    clock: Arc<dyn Clock>,
    /// The idle lock's clock (research.md §15).
    idle: IdleClock,
    /// The path of the database last installed, readable while a running
    /// operation holds the session, for the first step of an immediate
    /// close (FR-037).
    open_path: Mutex<Option<PathBuf>>,
    /// A close under way, and whether a sleep or shutdown has turned it
    /// immediate (data-model.md "Session states and transitions").
    closing: Mutex<Closing>,
    /// Signalled when a close under way finishes.
    closing_changed: Condvar,
}

/// A close under way.
#[derive(Default)]
pub(crate) struct Closing {
    /// A normal close of the database at this path has taken it out of the
    /// session and is finishing on its own thread (its backup).
    pub(crate) normal: Option<PathBuf>,
    /// An immediate close has begun (FR-037's step 1) and not finished.
    pub(crate) immediate: Option<ImmediateClose>,
}

/// An immediate close, from its first step to its last.
pub(crate) struct ImmediateClose {
    pub(crate) path: PathBuf,
    /// The operation its first step stopped.
    pub(crate) stopped: Option<StoppedOperation>,
    /// Steps 2 to 5 have been taken on by a thread, which the others wait
    /// for.
    pub(crate) completing: bool,
}

impl Default for Session {
    /// A session that reports to no one, for tests of its guards.
    fn default() -> Self {
        Self::new(Arc::new(NoEvents), None)
    }
}

impl Session {
    pub fn new(events: Arc<dyn SessionEvents>, opened_documents_dir: Option<PathBuf>) -> Self {
        Self {
            open: Mutex::new(None),
            events,
            opened_documents_dir,
            last_closed: Mutex::new(None),
            operations: Arc::default(),
            clock: Arc::new(SystemClock),
            idle: IdleClock::default(),
            open_path: Mutex::new(None),
            closing: Mutex::new(Closing::default()),
            closing_changed: Condvar::new(),
        }
    }

    /// The same session on another clock, for the tests.
    pub fn with_clock(self, clock: Arc<dyn Clock>) -> Self {
        Self { clock, ..self }
    }

    pub fn operations(&self) -> &Operations {
        &self.operations
    }

    /// The registry itself, for whatever stops operations from another
    /// thread (a skip, a sleep).
    pub fn operations_handle(&self) -> Arc<Operations> {
        Arc::clone(&self.operations)
    }

    pub fn clock(&self) -> &dyn Clock {
        &*self.clock
    }

    pub fn idle(&self) -> &IdleClock {
        &self.idle
    }

    fn lock(&self) -> MutexGuard<'_, Option<OpenDatabase>> {
        self.open.lock().expect("session mutex poisoned")
    }

    /// The open database, unless an immediate close has begun: from its
    /// first step no command reaches the database (FR-037).
    fn open_guard(&self) -> Result<MutexGuard<'_, Option<OpenDatabase>>, CommandError> {
        let guard = self.lock();
        if self.closing_immediately() {
            return Err(CommandError::database_closed());
        }
        Ok(guard)
    }

    /// Holds the session for a whole operation that closes and reopens the
    /// database in between (a restore), so no command runs against it
    /// half-way. `DATABASE_CLOSED` once an immediate close has begun.
    pub(crate) fn hold(&self) -> Result<MutexGuard<'_, Option<OpenDatabase>>, CommandError> {
        self.open_guard()
    }

    /// Holds the session whatever is under way, for the close itself.
    pub(crate) fn hold_for_close(&self) -> MutexGuard<'_, Option<OpenDatabase>> {
        self.lock()
    }

    pub(crate) fn closing(&self) -> MutexGuard<'_, Closing> {
        self.closing.lock().expect("session mutex poisoned")
    }

    /// Tells whoever waits for a close under way that it has moved on.
    pub(crate) fn closing_changed(&self) {
        self.closing_changed.notify_all();
    }

    /// Waits up to `timeout` for the close under way, if any, to finish.
    /// Returns whether nothing is closing any more.
    pub(crate) fn wait_closed(&self, timeout: Duration) -> bool {
        let closing = self.closing();
        let (_closing, waited) = self
            .closing_changed
            .wait_timeout_while(closing, timeout, |closing| {
                closing.normal.is_some() || closing.immediate.is_some()
            })
            .expect("session mutex poisoned");
        !waited.timed_out()
    }

    /// An immediate close has begun and not finished.
    pub fn closing_immediately(&self) -> bool {
        self.closing().immediate.is_some()
    }

    /// The path of the database last made the open one, if it is still
    /// open as far as the session knows.
    pub(crate) fn open_path(&self) -> Option<PathBuf> {
        self.open_path.lock().expect("session mutex poisoned").clone()
    }

    /// An operation that closed and reopened the database in place (a
    /// restore, a passphrase change) put `open` back: its lock settings
    /// may have changed with it.
    pub(crate) fn reinstalled(&self, open: &OpenDatabase) {
        self.idle.start(open.lock_settings.clone(), self.clock.now());
    }

    pub fn events(&self) -> &dyn SessionEvents {
        &*self.events
    }

    /// Runs a query against the open database.
    ///
    /// Collection commands are refused with `PENDING_CHANGES_UNRESOLVED`
    /// while the database's pending changes wait to be resumed or discarded
    /// (FR-039), and this and [`write`](Self::write) are how they reach it.
    pub fn read<T>(
        &self,
        query: impl FnOnce(&Connection) -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        let mut open = self.open_guard()?;
        let open = open.as_mut().ok_or_else(CommandError::database_closed)?;
        if open.pending_unresolved {
            return Err(CommandError::pending_changes_unresolved());
        }
        let result = query(&open.conn);
        storage_lost_if_unreachable(open, result)
    }

    /// Runs anything that changes the open database, after checking that
    /// the file at its path is still the one this session last wrote
    /// (FR-032, research.md §6):
    /// - replaced, or changed by someone else: another computer has taken
    ///   the database over. Nothing is written, and the session closes
    ///   without writing anything more (`DATABASE_TAKEN_OVER`).
    /// - unreachable: its drive or network share has gone. Nothing is
    ///   written, and the session stays open but refuses every later write
    ///   (`DATABASE_UNAVAILABLE`), since a remounted drive can come back
    ///   looking replaced.
    ///
    /// The fingerprint is refreshed afterwards, so this session's own writes
    /// never look like someone else's.
    pub fn write<T>(
        &self,
        change: impl FnOnce(&Connection) -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        self.write_checked(true, change)
    }

    /// Like [`write`](Self::write), for the session's own housekeeping (a
    /// dismissed note, resolving pending changes), which unresolved pending
    /// changes don't hold up.
    pub fn write_housekeeping<T>(
        &self,
        change: impl FnOnce(&Connection) -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        self.write_checked(false, change)
    }

    fn write_checked<T>(
        &self,
        collection: bool,
        change: impl FnOnce(&Connection) -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        let mut guard = self.open_guard()?;
        let open = guard.as_mut().ok_or_else(CommandError::database_closed)?;
        if collection && open.pending_unresolved {
            return Err(CommandError::pending_changes_unresolved());
        }
        if open.storage_lost {
            return Err(CommandError::database_unavailable(&open.path));
        }
        match open.fingerprint.check(&open.path) {
            FingerprintCheck::Same => {}
            FingerprintCheck::Replaced => {
                let open = guard.take().expect("checked above");
                drop(guard);
                lifecycle::close_taken_over(self, open);
                return Err(CommandError::database_taken_over());
            }
            FingerprintCheck::Unreachable => {
                log::warn!("{} can no longer be reached", open.path.display());
                open.storage_lost = true;
                return Err(CommandError::database_unavailable(&open.path));
            }
        }
        let result = change(&open.conn);
        let result = storage_lost_if_unreachable(open, result);
        if !open.storage_lost {
            match FileFingerprint::capture(&open.path) {
                Ok(fingerprint) => open.fingerprint = fingerprint,
                Err(err) => {
                    log::warn!("{} can no longer be reached: {err}", open.path.display());
                    open.storage_lost = true;
                }
            }
        }
        result
    }

    /// Runs `look` against the open database itself, for the session's own
    /// commands (its status, its notes), which describe the database rather
    /// than read the collection.
    pub fn inspect<T>(
        &self,
        look: impl FnOnce(&OpenDatabase) -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        let open = self.open_guard()?;
        look(open.as_ref().ok_or_else(CommandError::database_closed)?)
    }

    /// Like [`inspect`](Self::inspect), for what the session itself keeps
    /// about the open database (its notes), never the file.
    pub fn inspect_mut<T>(
        &self,
        change: impl FnOnce(&mut OpenDatabase) -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        let mut open = self.open_guard()?;
        change(open.as_mut().ok_or_else(CommandError::database_closed)?)
    }

    pub fn is_open(&self) -> bool {
        self.lock().is_some()
    }

    /// Makes `open` the session's database, and starts the idle clock on
    /// its lock settings.
    pub fn install(&self, open: OpenDatabase) {
        self.idle.start(open.lock_settings.clone(), self.clock.now());
        *self.open_path.lock().expect("session mutex poisoned") = Some(open.path.clone());
        *self.lock() = Some(open);
    }

    /// Removes the open database from the session, for a close: from here
    /// on every command is refused with `DATABASE_CLOSED`.
    pub fn take(&self) -> Option<OpenDatabase> {
        let taken = self.lock().take();
        self.forget_open();
        taken
    }

    /// Nothing is open any more: the idle clock stops.
    pub(crate) fn forget_open(&self) {
        self.idle.stop();
        *self.open_path.lock().expect("session mutex poisoned") = None;
    }

    /// The database most recently closed in this run, if any.
    pub fn last_closed(&self) -> Option<PathBuf> {
        self.last_closed.lock().expect("session mutex poisoned").clone()
    }

    pub(crate) fn set_last_closed(&self, path: &Path) {
        *self.last_closed.lock().expect("session mutex poisoned") = Some(path.to_owned());
    }

    /// Deletes this session's decrypted document copies (FR-022).
    pub(crate) fn clear_opened_documents(&self) {
        let Some(dir) = &self.opened_documents_dir else { return };
        for leftover in crate::commands::documents::ops::clear_opened_documents(dir) {
            log::warn!("could not delete opened-document copy {}", leftover.display());
        }
    }
}

/// SQLite reports an I/O error, or can't reopen a file, when the drive
/// under an open database has gone (`CommandError::from_db` gives these
/// `DATABASE_UNAVAILABLE`): the same as finding it unreachable beforehand.
fn storage_lost_if_unreachable<T>(
    open: &mut OpenDatabase,
    result: Result<T, CommandError>,
) -> Result<T, CommandError> {
    match result {
        Err(err) if err.code == CommandError::DATABASE_UNAVAILABLE => {
            open.storage_lost = true;
            Err(CommandError::database_unavailable(&open.path))
        }
        other => other,
    }
}

/// Tells a chooser already showing that notices are waiting.
pub const CHOOSER_NOTICES: &str = "chooser:notices";

/// Where the session reports what happens to it: the frontend and
/// `machine.json` in the app, a recorder in the tests.
pub trait SessionEvents: Send + Sync {
    fn emit(&self, event: &str, payload: serde_json::Value);
    /// Keeps a notice for the chooser to show next.
    fn notice(&self, notice: ChooserNotice);
}

impl<R: tauri::Runtime> SessionEvents for tauri::AppHandle<R> {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        if let Err(err) = tauri::Emitter::emit(self, event, payload) {
            log::warn!("could not emit {event}: {err}");
        }
    }

    fn notice(&self, notice: ChooserNotice) {
        use tauri::Manager;
        match self.try_state::<MachineSettings>() {
            Some(machine) => machine.push_notice(notice),
            None => return log::warn!("no machine settings to keep a notice in"),
        }
        // A chooser already showing picks up a notice that came after it,
        // such as the operation a sleep stopped (FR-037).
        SessionEvents::emit(self, CHOOSER_NOTICES, serde_json::json!({}));
    }
}

struct NoEvents;

impl SessionEvents for NoEvents {
    fn emit(&self, _event: &str, _payload: serde_json::Value) {}
    fn notice(&self, _notice: ChooserNotice) {}
}
