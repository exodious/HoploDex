//! Whether PDF preview is available, and why not (research.md §7, §17).

use std::sync::Mutex;

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
}

/// [`PdfAvailability`] as Tauri state: decided at startup and changed at run
/// time (`NoViewer`, and `Held` when the watch catches a copy). `Available`
/// until the startup check exists.
#[derive(Debug)]
pub struct PdfAvailabilityState(Mutex<PdfAvailability>);

impl PdfAvailabilityState {
    pub fn new(initial: PdfAvailability) -> Self {
        Self(Mutex::new(initial))
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
