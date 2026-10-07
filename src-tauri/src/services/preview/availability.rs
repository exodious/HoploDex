//! Whether PDF preview is available, and why not (research.md §7, §17).

use std::sync::{Arc, Mutex};

use crate::services::machine_settings::MachineSettings;

/// Why PDF preview is off on this computer (FR-003a).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnavailableReason {
    /// A copy written by the PDF viewer was caught during this version of
    /// HoploDex; PDF preview stays off until a different version starts.
    Held,
    /// The startup check of the web view's PDF switches failed.
    CheckFailed,
    /// The web view has no PDF viewer: a surface turned its PDF into a
    /// download this run.
    NoViewer,
}

impl UnavailableReason {
    /// The plain sentence for a message: what is off and why, ending in what
    /// still works.
    pub fn message(self) -> &'static str {
        match self {
            Self::Held => {
                "PDF previews are turned off on this computer until HoploDex is updated, because \
                 this computer's PDF viewer saved a copy of a document to disk."
            }
            Self::CheckFailed => {
                "PDFs can't be previewed on this computer, because HoploDex couldn't confirm that \
                 its PDF viewer is kept from saving copies of documents to disk."
            }
            Self::NoViewer => {
                "PDFs can't be previewed on this computer, because its web view has no built-in \
                 PDF viewer."
            }
        }
    }
}

/// Whether PDFs are previewed on this computer in this run (data-model.md
/// "In memory, per run"). TIFF and text are unaffected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfAvailability {
    Available,
    Unavailable { reason: UnavailableReason },
}

impl PdfAvailability {
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }

    /// Decides at startup (research.md §7, §17): this version's hold keeps
    /// PDF preview off, then the OS check. A hold for another version is
    /// cleared, so this version tries again. (T058 adds the per-OS check
    /// itself, which gives `os_check_passed`.)
    pub fn at_startup(
        machine: &MachineSettings,
        running_version: &str,
        os_check_passed: bool,
    ) -> Self {
        machine.clear_stale_hold(running_version);
        let held = machine
            .pdf_preview_hold()
            .is_some_and(|hold| hold.version.as_deref() == Some(running_version));
        if held {
            Self::Unavailable { reason: UnavailableReason::Held }
        } else if !os_check_passed {
            Self::Unavailable { reason: UnavailableReason::CheckFailed }
        } else {
            Self::Available
        }
    }
}

/// [`PdfAvailability`] as Tauri state: decided at startup and changed at run
/// time (`NoViewer`, and `Held` when the watch catches a copy). `Available`
/// until the startup check exists.
///
/// A handle to shared state: a clone is the same availability, so the
/// preview commands' `PreviewEnv` and the document list read one value.
#[derive(Debug, Clone)]
pub struct PdfAvailabilityState(Arc<Mutex<PdfAvailability>>);

impl PdfAvailabilityState {
    pub fn new(initial: PdfAvailability) -> Self {
        Self(Arc::new(Mutex::new(initial)))
    }

    pub fn get(&self) -> PdfAvailability {
        *self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn set(&self, availability: PdfAvailability) {
        *self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = availability;
    }
}

impl Default for PdfAvailabilityState {
    fn default() -> Self {
        Self::new(PdfAvailability::Available)
    }
}
