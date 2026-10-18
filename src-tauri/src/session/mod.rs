//! The open database, if any (003 research.md §13; 003 data-model.md "In memory:
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
pub mod scoped;
pub mod upgrade;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use rusqlite::{Connection, InterruptHandle};
use serde::Serialize;

use crate::commands::CommandError;
use crate::commands::import_export::ImportSessionStore;
use crate::db;
use crate::models::database::{
    ChooserNotice, DatabaseNotes, Draft, LockSettings, OpenedBackupNote, PendingState,
};
use crate::services::machine_settings::MachineSettings;
use crate::services::passphrase::Passphrase;
use crate::services::preview::{Preview, PreviewLoading};
use clock::{Clock, SystemClock};
use fingerprint::{FileFingerprint, FingerprintCheck};
use idle::IdleClock;
use operations::{Operations, StoppedOperation};

/// An open database: its connection, holding the file's exclusive lock and
/// (inside SQLCipher) the derived key, and what the session knows about it.
/// The passphrase itself is not here: the backend never keeps one (003 FR-007).
pub struct OpenDatabase {
    pub conn: Connection,
    pub path: PathBuf,
    /// The file name without its extension.
    pub name: String,
    pub database_id: String,
    /// Stops a long statement on `conn` from another thread.
    pub interrupt: InterruptHandle,
    /// The open form's unsaved input, mirrored from the frontend and written
    /// only by a lock or an OS shutdown (003 research.md §16).
    pub staged_draft: Option<Draft>,
    /// Where the database's pending changes stand (003 FR-039; 008 FR-003,
    /// research.md §6): a row that waits to be resumed or discarded blocks
    /// collection commands (`Unresolved`); one resumed whose form hasn't
    /// opened yet stays and lets them through (`Resuming`).
    pub pending: PendingState,
    /// The file as this session last left it, to notice a take-over
    /// (003 research.md §6).
    pub fingerprint: FileFingerprint,
    /// The file stopped being reachable while open. Every later write is
    /// refused with `DATABASE_UNAVAILABLE`, whatever the fingerprint says
    /// once it is back, and the close writes nothing (003 research.md §6).
    pub storage_lost: bool,
    /// The notes this open reports once: a backup opened directly, a
    /// restore. `disk_encryption` is not used here: it is kept in the file.
    pub notes: DatabaseNotes,
    /// The lock settings kept in the file, for the idle clock (003 FR-034).
    pub lock_settings: LockSettings,
    /// The one open document preview, if any. It lives and dies with this
    /// value, so every path that drops the `OpenDatabase` (lock, close,
    /// switch, sleep, shutdown, quit) ends the preview too: its TIFF helper
    /// is killed, its PDF bytes zeroized and its surface closed (007 research.md
    /// §20).
    pub preview: Option<Preview>,
    /// The id the last `open_preview` of this open took. Ids count up by one
    /// within a session, so a request for a replaced preview is refused
    /// (007 data-model.md "Preview").
    pub preview_seq: u64,
    /// A TIFF's `Load` still under way, which `close_preview` can end
    /// without waiting for it.
    pub preview_loading: Option<PreviewLoading>,
    /// Which open or unlock of a database this is, from a counter that only
    /// goes up in this run (008 research.md §1). Every accessor compares it
    /// with the id a request named, under the session's mutex, so a command
    /// that asks the user something and acts on the answer afterwards acts
    /// only in the session that asked (008 research.md §2; 007 research.md
    /// §18). A passphrase change keeps it.
    pub session_id: SessionId,
    /// The user answered "Open in another app" in the native confirmation
    /// for a document during this open. It dies with this value, so a lock,
    /// close or switch forgets it, and is never written anywhere (007 FR-012,
    /// research.md §17).
    pub external_open_confirmed: bool,
    /// The imports of this open whose conflicts wait to be decided (issue
    /// #64). They hold the file's plaintext rows and its record ids, so they
    /// belong to this value and nothing else: any path that drops the
    /// `OpenDatabase` (lock, idle lock, close, switch, restore, take-over,
    /// sleep, shutdown) drops them, and a later open of the same file starts
    /// with none.
    pub imports: ImportSessionStore,
}

/// The last [`SessionId`] handed out.
static SESSION_IDS: AtomicU64 = AtomicU64::new(0);

/// Which open of a database a request belongs to (008 research.md §1,
/// data-model.md "SessionId"). Every open, unlock and restore gets a new
/// one from a counter that only goes up within a run, so an id is never
/// handed out twice. `0` is never issued and means "none".
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct SessionId(u64);

impl SessionId {
    /// No session: nothing is open.
    pub const NONE: SessionId = SessionId(0);

    /// The id a request named, or `None` for `0`, which is never issued.
    pub fn from_request(raw: u64) -> Option<Self> {
        (raw != 0).then_some(Self(raw))
    }

    /// The next id, which no earlier call returned.
    fn next() -> Self {
        Self(SESSION_IDS.fetch_add(1, Ordering::Relaxed) + 1)
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// What a session knew about its open database when something was read from
/// it (see [`Session::read_stamped`]); the session itself is the id the
/// request named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenStamp {
    pub external_open_confirmed: bool,
}

impl OpenDatabase {
    /// Wraps a connection from `db::create_database` or `db::open_database`,
    /// taking the file's fingerprint as the open left it.
    pub fn new(conn: Connection, path: &Path) -> Result<Self, CommandError> {
        let database_id: String = conn
            .query_row("SELECT database_id FROM app_state", [], |row| row.get(0))
            .map_err(CommandError::from_db)?;
        let has_pending: bool = conn
            .query_row("SELECT EXISTS (SELECT 1 FROM pending_changes)", [], |row| row.get(0))
            .map_err(CommandError::from_db)?;
        // Every open starts a row in `Unresolved`, even one a session ended
        // while `Resuming` (008 research.md §6).
        let pending = if has_pending { PendingState::Unresolved } else { PendingState::None };
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
            pending,
            fingerprint,
            storage_lost: false,
            notes: DatabaseNotes::default(),
            lock_settings,
            preview: None,
            preview_seq: 0,
            preview_loading: None,
            session_id: SessionId::next(),
            external_open_confirmed: false,
            imports: ImportSessionStore::new(),
        })
    }

    /// Ends the document preview now, whatever it is: a TIFF's load is
    /// stopped and its helper killed, a PDF's bytes are zeroized and its
    /// surface closed (007 research.md §20). Returns the preview that was open,
    /// so a PDF can hand its surface to the next one before it is dropped;
    /// a caller that holds the session's lock drops it after letting go.
    pub fn end_preview(&mut self) -> Option<Preview> {
        if let Some(loading) = self.preview_loading.take()
            && let Some(helper) = loading.helper
        {
            helper.shutdown();
        }
        self.preview.take()
    }
}

/// A database's name: its file name without the extension.
pub fn database_name(path: &Path) -> String {
    path.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Tauri state: the open database, or none, and where the session reports
/// what happens to it.
///
/// A handle to shared state, so it is cheap to clone: every clone is the same
/// session. The preview commands need an `Arc<Session>` for the threads that
/// outlive a call (007 research.md §8's hook timer), which a command gets from
/// [`Session::shared`].
#[derive(Clone)]
pub struct Session(Arc<SessionInner>);

impl std::ops::Deref for Session {
    type Target = SessionInner;

    fn deref(&self) -> &SessionInner {
        &self.0
    }
}

/// A reading of the waiting slot's generation (009 research.md §4 "The slot's
/// life"): how many times the slot has been dropped, up to then.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AwaitingGeneration(u64);

/// What a [`Session`] holds.
pub struct SessionInner {
    open: Mutex<Option<OpenDatabase>>,
    /// The open database's [`SessionId`], or 0 (`SessionId::NONE`) with
    /// nothing open: set by `install`, cleared by `take` and `forget_open`.
    /// Read without the mutex, by work running outside it (008 research.md
    /// §1, §11); the accessors still compare under the mutex.
    current: AtomicU64,
    events: Arc<dyn SessionEvents>,
    /// Where `open_document` puts decrypted copies, deleted at every close
    /// (003 FR-022). `None` where nothing makes them.
    opened_documents_dir: Option<PathBuf>,
    /// The database most recently closed in this run, which the chooser
    /// selects (003 FR-033).
    last_closed: Mutex<Option<PathBuf>>,
    /// The long-running operation, if any (003 research.md §13).
    operations: Arc<Operations>,
    clock: Arc<dyn Clock>,
    /// The idle lock's clock (003 research.md §15).
    idle: IdleClock,
    /// The path of the database last installed, readable while a running
    /// operation holds the session, for the first step of an immediate
    /// close (003 FR-037).
    open_path: Mutex<Option<PathBuf>>,
    /// A close under way, and whether a sleep or shutdown has turned it
    /// immediate (003 data-model.md "Session states and transitions").
    closing: Mutex<Closing>,
    /// Signalled when a close under way finishes.
    closing_changed: Condvar,
    /// The database waiting for the answer to the upgrade question (009
    /// research.md §4). Separate from `open`, so no collection command, scoped
    /// or not, can reach it.
    awaiting_upgrade: Mutex<Option<AwaitingUpgrade>>,
    /// How many times the waiting slot has been dropped (the generation). Only
    /// raised, and only changed, under `awaiting_upgrade`'s mutex. A database
    /// goes into the slot, or back into it, only if the generation is still
    /// the one read before it left the slot or before its key was derived, so
    /// a sleep, screen lock or shutdown that found the slot empty still wins
    /// (009 research.md §4 "The slot's life").
    awaiting_drops: AtomicU64,
    /// The `awaitingId` of the `upgrade_database` call that took the waiting
    /// database out of the slot and is still working, or 0: what
    /// `cancel_upgrade` finds the backup it stops by.
    upgrading: AtomicU64,
    /// A screen lock came while `upgrade_database` worked (since Upgrade was
    /// chosen): set by `lifecycle::screen_locked`, cleared as the next
    /// `upgrade_database` takes the waiting database, read at the end of the
    /// upgrade to decide whether the database opens (009 research.md §5
    /// "Sleep and shutdown").
    screen_locked_in_upgrade: AtomicBool,
}

/// A backup opened directly that needs an upgrade: its stamp is taken inside
/// the upgrade transaction, not before the question, so Cancel leaves the
/// file untouched and still a backup (009 research.md §8).
pub struct AwaitingBackupStamp {
    /// The identity the file gets, which the backup before the upgrade is
    /// named and stamped with.
    pub new_database_id: String,
    /// What `prepare` shows once the file has become a database of its own.
    pub note: OpenedBackupNote,
}

/// The database waiting for the answer to the upgrade question (009 FR-010, data-model.md
/// "AwaitingUpgrade"): keyed and holding the file's exclusive lock, with
/// nothing written. Dropping it closes the connection, which releases the lock
/// and clears the key.
pub struct AwaitingUpgrade {
    /// The `awaitingId` the chooser answers with.
    pub id: SessionId,
    pub conn: Connection,
    pub path: PathBuf,
    pub name: String,
    pub database_id: String,
    /// The step names to apply, in order.
    pub steps_due: Vec<&'static str>,
    /// The open marker found at the question, compared again at Upgrade.
    pub marker_seen: db::SeenMarker,
    pub backup_stamp: Option<AwaitingBackupStamp>,
    /// A typed passphrase the open will save or refresh in the keyring once
    /// the upgrade succeeds (003 FR-017, FR-018); zeroized when dropped.
    pub save_passphrase: Option<Passphrase>,
    /// The database's backup location, resolved for this computer.
    pub backup_folder: PathBuf,
    /// The backup location is the default one, made when missing.
    pub backup_folder_is_default: bool,
}

/// A close under way.
#[derive(Default)]
pub(crate) struct Closing {
    /// A normal close of the database at this path has taken it out of the
    /// session and is finishing on its own thread (its backup).
    pub(crate) normal: Option<PathBuf>,
    /// The id of the session that normal close is ending, for the events
    /// an immediate close announces meanwhile (`SessionId::NONE` with none).
    pub(crate) normal_id: SessionId,
    /// An immediate close has begun (003 FR-037's step 1) and not finished.
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
        Self(Arc::new(SessionInner {
            open: Mutex::new(None),
            current: AtomicU64::new(0),
            events,
            opened_documents_dir,
            last_closed: Mutex::new(None),
            operations: Arc::default(),
            clock: Arc::new(SystemClock),
            idle: IdleClock::default(),
            open_path: Mutex::new(None),
            closing: Mutex::new(Closing::default()),
            closing_changed: Condvar::new(),
            awaiting_upgrade: Mutex::new(None),
            awaiting_drops: AtomicU64::new(0),
            upgrading: AtomicU64::new(0),
            screen_locked_in_upgrade: AtomicBool::new(false),
        }))
    }

    /// The same session on another clock, for the tests. Before any clone of
    /// it exists.
    pub fn with_clock(self, clock: Arc<dyn Clock>) -> Self {
        let Ok(inner) = Arc::try_unwrap(self.0) else {
            panic!("a session that has been cloned can't change its clock");
        };
        Self(Arc::new(SessionInner { clock, ..inner }))
    }

    /// The session this handle is a handle to, as an `Arc` of its own.
    pub fn shared(&self) -> Arc<Session> {
        Arc::new(self.clone())
    }
}

impl SessionInner {
    pub fn operations(&self) -> &Operations {
        &self.operations
    }

    /// The registry itself, for whatever stops operations from another
    /// thread (a skip, a sleep).
    pub fn operations_handle(&self) -> Arc<Operations> {
        Arc::clone(&self.operations)
    }

    /// The open database's session id, or `SessionId::NONE`, without
    /// waiting on the session's mutex (a close may hold it for a while).
    pub fn current_id(&self) -> SessionId {
        SessionId(self.current.load(Ordering::SeqCst))
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
    /// first step no command reaches the database (003 FR-037).
    fn open_guard(&self) -> Result<MutexGuard<'_, Option<OpenDatabase>>, CommandError> {
        let guard = self.lock();
        if self.closing_immediately() {
            return Err(CommandError::database_closed());
        }
        Ok(guard)
    }

    /// The open database, only if it is the session `id`, which is compared
    /// under the session's mutex, before anything else: another session
    /// open, nothing open and an immediate close under way are all
    /// `DATABASE_CLOSED` (008 research.md §2, FR-004). The guard is held for
    /// the caller's work, so a close can't come between the check and it.
    fn open_guard_of(
        &self,
        id: SessionId,
    ) -> Result<MutexGuard<'_, Option<OpenDatabase>>, CommandError> {
        let guard = self.open_guard()?;
        match guard.as_ref() {
            Some(open) if open.session_id == id => Ok(guard),
            _ => Err(CommandError::database_closed()),
        }
    }

    /// Holds the session `id` for a whole operation that closes and reopens
    /// the database in between (a restore), so no command runs against it
    /// half-way. `DATABASE_CLOSED` once an immediate close has begun, or
    /// when `id` isn't the open session.
    pub(crate) fn hold(
        &self,
        id: SessionId,
    ) -> Result<MutexGuard<'_, Option<OpenDatabase>>, CommandError> {
        self.open_guard_of(id)
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
    /// may have changed with it, and its session id is the current one (a
    /// restore's is new, a passphrase change's is the one it had).
    pub(crate) fn reinstalled(&self, open: &OpenDatabase) {
        self.current.store(open.session_id.get(), Ordering::SeqCst);
        self.idle.start(open.lock_settings.clone(), self.clock.now());
    }

    pub fn events(&self) -> &dyn SessionEvents {
        &*self.events
    }

    /// Runs a query against the open database, if it is the session `id`
    /// (every accessor below does the same: the id is compared under the
    /// session's mutex, before the fingerprint check and before `query`
    /// runs, and a mismatch or nothing open is `DATABASE_CLOSED` with
    /// nothing run).
    ///
    /// Collection commands are refused with `PENDING_CHANGES_UNRESOLVED`
    /// while the database's pending changes wait to be resumed or discarded
    /// (003 FR-039; `PendingState::Unresolved`, and not once they are being
    /// resumed), and this and [`write`](Self::write) are how they reach it.
    pub fn read<T>(
        &self,
        id: SessionId,
        query: impl FnOnce(&Connection) -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        let mut open = self.open_guard_of(id)?;
        let open = open.as_mut().ok_or_else(CommandError::database_closed)?;
        if open.pending == PendingState::Unresolved {
            return Err(CommandError::pending_changes_unresolved());
        }
        let result = query(&open.conn);
        storage_lost_if_unreachable(open, result)
    }

    /// [`read`](Self::read), and the [`OpenStamp`] of the open it read from,
    /// for a command that acts on the open later (`open_document`).
    pub fn read_stamped<T>(
        &self,
        id: SessionId,
        query: impl FnOnce(&Connection) -> Result<T, CommandError>,
    ) -> Result<(T, OpenStamp), CommandError> {
        let mut open = self.open_guard_of(id)?;
        let open = open.as_mut().ok_or_else(CommandError::database_closed)?;
        if open.pending == PendingState::Unresolved {
            return Err(CommandError::pending_changes_unresolved());
        }
        let stamp = OpenStamp { external_open_confirmed: open.external_open_confirmed };
        let result = query(&open.conn);
        storage_lost_if_unreachable(open, result).map(|value| (value, stamp))
    }

    /// Runs anything that changes the open database, after checking that
    /// the file at its path is still the one this session last wrote
    /// (003 FR-032, research.md §6):
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
        id: SessionId,
        change: impl FnOnce(&Connection) -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        self.write_checked(id, true, |open| change(&open.conn))
    }

    /// [`write`](Self::write), for a change that also needs what the open
    /// itself keeps (an import's held conflicts, [`OpenDatabase::imports`]):
    /// they are reached only through the open that made them.
    pub fn write_open<T>(
        &self,
        id: SessionId,
        change: impl FnOnce(&OpenDatabase) -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        self.write_checked(id, true, |open| change(open))
    }

    /// Like [`write`](Self::write), for the session's own housekeeping (a
    /// dismissed note, resolving pending changes), which unresolved pending
    /// changes don't hold up.
    pub fn write_housekeeping<T>(
        &self,
        id: SessionId,
        change: impl FnOnce(&Connection) -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        self.write_checked(id, false, |open| change(&open.conn))
    }

    /// The one place a write is checked. `collection` is false for the
    /// session's own housekeeping, which unresolved pending changes don't
    /// hold up; `change` may alter what the session keeps about the open
    /// (resolving pending changes does).
    fn write_checked<T>(
        &self,
        id: SessionId,
        collection: bool,
        change: impl FnOnce(&mut OpenDatabase) -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        let mut guard = self.open_guard_of(id)?;
        let open = guard.as_mut().ok_or_else(CommandError::database_closed)?;
        if collection && open.pending == PendingState::Unresolved {
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
        let changes_before = open.conn.total_changes();
        let result = change(open);
        let result = storage_lost_if_unreachable(open, result);
        // 008 FR-015: a write that succeeded and changed rows (a trigger's
        // included) is a save by this version.
        if result.is_ok()
            && !open.storage_lost
            && open.conn.total_changes() != changes_before
            && let Err(err) = db::record_saved_version(&open.conn)
        {
            log::warn!("could not record the saving version: {err}");
        }
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
        id: SessionId,
        look: impl FnOnce(&OpenDatabase) -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        let open = self.open_guard_of(id)?;
        look(open.as_ref().ok_or_else(CommandError::database_closed)?)
    }

    /// Like [`inspect`](Self::inspect), for what the session itself keeps
    /// about the open database (its notes, a confirmation, a copy outside
    /// the database), never the file. Unlike [`write`](Self::write) it
    /// neither checks the file's fingerprint nor refuses while pending
    /// changes wait. `open_document` writes a decrypted copy through it:
    /// a close or lock takes the same lock before it clears the copies
    /// folder, so what is written there is either cleared by it or never
    /// written (007 research.md §18).
    pub fn inspect_mut<T>(
        &self,
        id: SessionId,
        change: impl FnOnce(&mut OpenDatabase) -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        let mut open = self.open_guard_of(id)?;
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
        let mut slot = self.lock();
        self.current.store(open.session_id.get(), Ordering::SeqCst);
        *slot = Some(open);
    }

    /// Removes the open database from the session, for a close: from here
    /// on every command is refused with `DATABASE_CLOSED`. Only the session
    /// `id`: another session open, or nothing, is `None`, compared under the
    /// mutex, so a late close can't end the session opened after it
    /// (008 research.md §2).
    pub fn take(&self, id: SessionId) -> Option<OpenDatabase> {
        let taken = {
            let mut slot = self.lock();
            if slot.as_ref().is_none_or(|open| open.session_id != id) {
                return None;
            }
            self.current.store(0, Ordering::SeqCst);
            slot.take()
        };
        self.forget_open();
        taken
    }

    /// Ends the open database's preview, if one is open, before anything else
    /// of a close is announced: the surface is gone from the screen and the
    /// helper dead by then (007 FR-014, SC-006). For a close that has begun but
    /// has not yet taken the database (an immediate one). The preview is
    /// dropped outside the session's lock.
    pub(crate) fn end_preview(&self) {
        let ended = self.lock().as_mut().and_then(OpenDatabase::end_preview);
        drop(ended);
    }

    /// Nothing is open any more: the idle clock stops.
    pub(crate) fn forget_open(&self) {
        self.current.store(0, Ordering::SeqCst);
        self.idle.stop();
        *self.open_path.lock().expect("session mutex poisoned") = None;
    }

    fn awaiting(&self) -> MutexGuard<'_, Option<AwaitingUpgrade>> {
        self.awaiting_upgrade.lock().expect("session mutex poisoned")
    }

    /// The slot's generation now. Taking the slot's guard is the proof that
    /// the mutex is held.
    fn generation(&self, _slot: &MutexGuard<'_, Option<AwaitingUpgrade>>) -> AwaitingGeneration {
        AwaitingGeneration(self.awaiting_drops.load(Ordering::SeqCst))
    }

    /// Puts a database that needs an upgrade in the waiting slot, in place of
    /// any other (009 research.md §4), if the slot has not been dropped since
    /// `generation` was read. Otherwise a sleep, screen lock, shutdown or
    /// another open came while its key was being derived: the database is
    /// dropped instead (its connection closes and the passphrase to be saved
    /// is zeroized). Returns whether it is waiting.
    pub(crate) fn wait_for_upgrade(
        &self,
        waiting: AwaitingUpgrade,
        generation: AwaitingGeneration,
    ) -> bool {
        let (replaced, unplaced) = {
            let mut slot = self.awaiting();
            if self.generation(&slot) == generation {
                (slot.replace(waiting), None)
            } else {
                (None, Some(waiting))
            }
        };
        // Connections close outside the slot's lock.
        let placed = unplaced.is_none();
        drop(replaced);
        drop(unplaced);
        placed
    }

    /// Drops the waiting database, if there is one: its connection closes, the
    /// file's lock is released and the key cleared, and nothing was written.
    /// Called when any open, create or restore begins, at quit, sleep, screen
    /// lock and shutdown (009 research.md §4). Raises the slot's generation,
    /// and returns the new one: what an open that begins here passes to
    /// [`wait_for_upgrade`](Self::wait_for_upgrade).
    pub fn drop_awaiting_upgrade(&self) -> AwaitingGeneration {
        let (dropped, generation) = {
            let mut slot = self.awaiting();
            let generation = self.awaiting_drops.fetch_add(1, Ordering::SeqCst) + 1;
            (slot.take(), AwaitingGeneration(generation))
        };
        // Its connection closes outside the slot's lock.
        drop(dropped);
        generation
    }

    /// Puts a database back in the waiting slot after the backup before its
    /// upgrade could not be made, for a retry under the same id, if the slot
    /// has not been dropped since `generation` was read (when it took the
    /// database out). Returns whether it was put back; if not, the database
    /// is dropped here.
    pub(crate) fn put_back_for_upgrade(
        &self,
        waiting: AwaitingUpgrade,
        generation: AwaitingGeneration,
    ) -> bool {
        let mut slot = self.awaiting();
        if self.generation(&slot) != generation {
            drop(slot);
            drop(waiting);
            return false;
        }
        *slot = Some(waiting);
        true
    }

    /// The `awaitingId` of the upgrade at work, or `SessionId::NONE`.
    pub(crate) fn upgrading_id(&self) -> SessionId {
        SessionId(self.upgrading.load(Ordering::SeqCst))
    }

    pub(crate) fn set_upgrading_id(&self, id: SessionId) {
        self.upgrading.store(id.get(), Ordering::SeqCst);
    }

    /// A screen lock came: if `upgrade_database` is working (the waiting
    /// database is out of the slot, so the lock's drop found nothing), it is
    /// remembered for the end of the upgrade.
    pub(crate) fn note_screen_lock_for_upgrade(&self) {
        if self.upgrading_id() != SessionId::NONE {
            self.screen_locked_in_upgrade.store(true, Ordering::SeqCst);
        }
    }

    /// A new `upgrade_database` begins: no screen lock has come yet.
    pub(crate) fn clear_screen_lock_for_upgrade(&self) {
        self.screen_locked_in_upgrade.store(false, Ordering::SeqCst);
    }

    /// Whether a screen lock came since `upgrade_database` last took the
    /// waiting database.
    pub(crate) fn screen_locked_in_upgrade(&self) -> bool {
        self.screen_locked_in_upgrade.load(Ordering::SeqCst)
    }

    /// Keeps `passphrase` with the database waiting under `id`, to save or
    /// refresh in the keyring once it is upgraded and open (003 FR-017,
    /// FR-018; 009 research.md §4 "The passphrase"). Nothing happens for any
    /// other id.
    pub(crate) fn keep_passphrase_for_upgrade(&self, id: SessionId, passphrase: Passphrase) {
        if let Some(waiting) = self.awaiting().as_mut().filter(|waiting| waiting.id == id) {
            waiting.save_passphrase = Some(passphrase);
        }
    }

    /// The database most recently closed in this run, if any.
    pub fn last_closed(&self) -> Option<PathBuf> {
        self.last_closed.lock().expect("session mutex poisoned").clone()
    }

    pub(crate) fn set_last_closed(&self, path: &Path) {
        *self.last_closed.lock().expect("session mutex poisoned") = Some(path.to_owned());
    }

    /// Deletes this session's decrypted document copies (003 FR-022).
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
/// A full disk (`DISK_FULL`) leaves the session working, and names the
/// database's file.
fn storage_lost_if_unreachable<T>(
    open: &mut OpenDatabase,
    result: Result<T, CommandError>,
) -> Result<T, CommandError> {
    match result {
        Err(err) if err.code == CommandError::DATABASE_UNAVAILABLE => {
            open.storage_lost = true;
            Err(CommandError::database_unavailable(&open.path))
        }
        Err(err) => Err(err.located_at(&open.path)),
        Ok(value) => Ok(value),
    }
}

/// Tells a chooser already showing that notices are waiting.
pub const CHOOSER_NOTICES: &str = "chooser:notices";

/// Where the session reports what happens to it: the frontend and
/// `machine.json` in the app, a recorder in the tests.
pub trait SessionEvents: Send + Sync {
    fn emit(&self, event: &str, payload: serde_json::Value);
    /// Emits to the main web view only (the preview events, 007 contracts/
    /// tauri-commands.md "Events"), where the app keeps them from the PDF
    /// surface's web view.
    fn emit_to_main(&self, event: &str, payload: serde_json::Value) {
        self.emit(event, payload);
    }
    /// Keeps a notice for the chooser to show next.
    fn notice(&self, notice: ChooserNotice);
}

impl<R: tauri::Runtime> SessionEvents for tauri::AppHandle<R> {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        if let Err(err) = tauri::Emitter::emit(self, event, payload) {
            log::warn!("could not emit {event}: {err}");
        }
    }

    fn emit_to_main(&self, event: &str, payload: serde_json::Value) {
        if let Err(err) = tauri::Emitter::emit_to(self, "main", event, payload) {
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
        // such as the operation a sleep stopped (003 FR-037).
        SessionEvents::emit(self, CHOOSER_NOTICES, serde_json::json!({}));
    }
}

struct NoEvents;

impl SessionEvents for NoEvents {
    fn emit(&self, _event: &str, _payload: serde_json::Value) {}
    fn notice(&self, _notice: ChooserNotice) {}
}
