//! The long-running operation, if one is running (research.md §13;
//! data-model.md "Operations registry"): at most one at a time, each
//! stoppable. Loops check [`OperationGuard::is_cancelled`] between rows or
//! chunks; a single long statement is stopped through its connection's
//! interrupt handle. The registry also pauses the idle clock (research.md
//! §15).

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use rusqlite::InterruptHandle;

use crate::commands::CommandError;
use crate::models::database::OperationKind;

struct Running {
    kind: OperationKind,
    cancel: AtomicBool,
    interrupt: Option<InterruptHandle>,
    /// How far it got: rows imported, backups deleted; for a move of
    /// backups, how many are not yet moved.
    done: AtomicU64,
    /// The folder a move of backups is moving them from.
    folder: Mutex<Option<String>>,
}

/// Kept in the session.
#[derive(Default)]
pub struct Operations {
    running: Mutex<Option<Arc<Running>>>,
    /// Signalled whenever an operation ends.
    ended: Condvar,
}

/// An operation asked to stop, as the stopper sees it (FR-037): what it
/// was and, once it has ended, how far it got.
#[derive(Clone)]
pub struct StoppedOperation(Arc<Running>);

impl StoppedOperation {
    pub fn kind(&self) -> OperationKind {
        self.0.kind
    }

    /// What it recorded with [`OperationGuard::record_done`].
    pub fn done(&self) -> u64 {
        self.0.done.load(Ordering::SeqCst)
    }

    /// What it recorded with [`OperationGuard::record_folder`].
    pub fn folder(&self) -> Option<String> {
        self.0.folder.lock().expect("operation folder mutex poisoned").clone()
    }
}

impl std::fmt::Debug for StoppedOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoppedOperation").field("kind", &self.0.kind).finish()
    }
}

/// Registration of a running operation, removed when dropped.
pub struct OperationGuard<'a> {
    operations: &'a Operations,
    running: Arc<Running>,
}

impl std::fmt::Debug for OperationGuard<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OperationGuard").field("kind", &self.running.kind).finish()
    }
}

impl OperationGuard<'_> {
    pub fn kind(&self) -> OperationKind {
        self.running.kind
    }

    /// The operation was asked to stop: finish the current row or chunk and
    /// return.
    pub fn is_cancelled(&self) -> bool {
        self.running.cancel.load(Ordering::SeqCst)
    }

    /// Records how far it has got (rows imported, backups deleted), for the
    /// notice when a sleep stops it.
    pub fn record_done(&self, done: u64) {
        self.running.done.store(done, Ordering::SeqCst);
    }

    /// Records the folder a move of backups is moving them from, for the
    /// notice when a sleep stops it.
    pub fn record_folder(&self, folder: &std::path::Path) {
        *self.running.folder.lock().expect("operation folder mutex poisoned") =
            Some(folder.to_string_lossy().into_owned());
    }
}

impl Drop for OperationGuard<'_> {
    fn drop(&mut self) {
        let mut running = self.operations.lock();
        if running.as_ref().is_some_and(|current| Arc::ptr_eq(current, &self.running)) {
            *running = None;
        }
        self.operations.ended.notify_all();
    }
}

impl Operations {
    fn lock(&self) -> MutexGuard<'_, Option<Arc<Running>>> {
        self.running.lock().expect("operations mutex poisoned")
    }

    /// Registers `kind` as the running operation, with the interrupt handle
    /// of the connection its long statement runs on, if any. Refused with
    /// `OPERATION_IN_PROGRESS` while another one runs.
    pub fn begin(
        &self,
        kind: OperationKind,
        interrupt: Option<InterruptHandle>,
    ) -> Result<OperationGuard<'_>, CommandError> {
        let mut running = self.lock();
        if let Some(current) = running.as_ref() {
            return Err(CommandError::operation_in_progress(current.kind));
        }
        let registered = Arc::new(Running {
            kind,
            cancel: AtomicBool::new(false),
            interrupt,
            done: AtomicU64::new(0),
            folder: Mutex::new(None),
        });
        *running = Some(Arc::clone(&registered));
        Ok(OperationGuard { operations: self, running: registered })
    }

    pub fn is_running(&self) -> bool {
        self.lock().is_some()
    }

    /// What is running, if anything.
    pub fn running_kind(&self) -> Option<OperationKind> {
        self.lock().as_ref().map(|running| running.kind)
    }

    /// Whether the running operation, if any, was asked to stop.
    pub fn is_cancelled(&self) -> bool {
        self.lock().as_ref().is_some_and(|running| running.cancel.load(Ordering::SeqCst))
    }

    /// Asks the running operation to stop, interrupting its statement, and
    /// returns what it was (FR-037).
    pub fn stop_running(&self) -> Option<OperationKind> {
        self.stop().map(|stopped| stopped.kind())
    }

    /// Like [`stop_running`](Self::stop_running), keeping hold of the
    /// operation to learn how far it got once it has ended.
    pub fn stop(&self) -> Option<StoppedOperation> {
        let running = self.lock();
        let running = running.as_ref()?;
        running.cancel.store(true, Ordering::SeqCst);
        if let Some(interrupt) = &running.interrupt {
            interrupt.interrupt();
        }
        Some(StoppedOperation(Arc::clone(running)))
    }

    /// Waits up to `timeout` for `stopped` to end. Returns whether it has.
    pub fn wait_ended(&self, stopped: &StoppedOperation, timeout: Duration) -> bool {
        let running = self.lock();
        let (_running, waited) = self
            .ended
            .wait_timeout_while(running, timeout, |running| {
                running.as_ref().is_some_and(|current| Arc::ptr_eq(current, &stopped.0))
            })
            .expect("operations mutex poisoned");
        !waited.timed_out()
    }
}
