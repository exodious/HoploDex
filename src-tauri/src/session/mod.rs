//! The open database, if any (research.md §13; data-model.md "In memory:
//! the session"). Replaces feature 001's always-open `DbHandle`: every
//! command reaches the database through [`Session::read`] or
//! [`Session::write`], which refuse when nothing is open. There is still one
//! connection and no pool.

pub mod lifecycle;
pub mod operations;

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use rusqlite::{Connection, InterruptHandle};

use crate::commands::CommandError;
use crate::models::database::Draft;

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
}

impl OpenDatabase {
    /// Wraps a connection from `db::create_database` or `db::open_database`.
    pub fn new(conn: Connection, path: &Path) -> rusqlite::Result<Self> {
        let database_id: String =
            conn.query_row("SELECT database_id FROM app_state", [], |row| row.get(0))?;
        let pending_unresolved: bool =
            conn.query_row("SELECT EXISTS (SELECT 1 FROM pending_changes)", [], |row| row.get(0))?;
        let interrupt = conn.get_interrupt_handle();
        Ok(Self {
            conn,
            path: path.to_owned(),
            name: database_name(path),
            database_id,
            interrupt,
            staged_draft: None,
            pending_unresolved,
        })
    }
}

/// A database's name: its file name without the extension.
pub fn database_name(path: &Path) -> String {
    path.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Tauri state: the open database, or none.
#[derive(Default)]
pub struct Session(Mutex<Option<OpenDatabase>>);

impl Session {
    fn lock(&self) -> MutexGuard<'_, Option<OpenDatabase>> {
        self.0.lock().expect("session mutex poisoned")
    }

    /// Runs a query against the open database.
    pub fn read<T>(
        &self,
        query: impl FnOnce(&Connection) -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        let open = self.lock();
        let open = open.as_ref().ok_or_else(CommandError::database_closed)?;
        query(&open.conn)
    }

    /// Runs anything that changes the open database.
    pub fn write<T>(
        &self,
        change: impl FnOnce(&Connection) -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        let open = self.lock();
        let open = open.as_ref().ok_or_else(CommandError::database_closed)?;
        change(&open.conn)
    }

    /// Runs `look` against the open database itself, for the session's own
    /// commands (its status, its notes), which describe the database rather
    /// than read the collection.
    pub fn inspect<T>(
        &self,
        look: impl FnOnce(&OpenDatabase) -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {
        let open = self.lock();
        look(open.as_ref().ok_or_else(CommandError::database_closed)?)
    }

    pub fn is_open(&self) -> bool {
        self.lock().is_some()
    }

    /// Makes `open` the session's database.
    pub fn install(&self, open: OpenDatabase) {
        *self.lock() = Some(open);
    }

    /// Removes the open database from the session, for a close: from here
    /// on every command is refused with `DATABASE_CLOSED`.
    pub fn take(&self) -> Option<OpenDatabase> {
        self.lock().take()
    }
}

/// Where the session's lifecycle events go: the frontend in the app, a
/// recorder in the tests.
pub trait SessionEvents {
    fn emit(&self, event: &str, payload: serde_json::Value);
}

impl<R: tauri::Runtime> SessionEvents for tauri::AppHandle<R> {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        if let Err(err) = tauri::Emitter::emit(self, event, payload) {
            log::warn!("could not emit {event}: {err}");
        }
    }
}
