//! The long-running operation, if one is running (research.md §13;
//! data-model.md "Operations registry"): at most one at a time, each
//! stoppable. Loops check [`OperationGuard::is_cancelled`] between rows or
//! chunks; a single long statement is stopped through its connection's
//! interrupt handle. The registry also pauses the idle clock (research.md
//! §15).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use rusqlite::InterruptHandle;

use crate::commands::CommandError;
use crate::models::database::OperationKind;

struct Running {
    kind: OperationKind,
    cancel: AtomicBool,
    interrupt: Option<InterruptHandle>,
}

/// Tauri state: the registry.
#[derive(Default)]
pub struct Operations {
    running: Mutex<Option<Arc<Running>>>,
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
}

impl Drop for OperationGuard<'_> {
    fn drop(&mut self) {
        let mut running = self.operations.lock();
        if running.as_ref().is_some_and(|current| Arc::ptr_eq(current, &self.running)) {
            *running = None;
        }
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
        let registered = Arc::new(Running { kind, cancel: AtomicBool::new(false), interrupt });
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
        let running = self.lock();
        let running = running.as_ref()?;
        running.cancel.store(true, Ordering::SeqCst);
        if let Some(interrupt) = &running.interrupt {
            interrupt.interrupt();
        }
        Some(running.kind)
    }
}
