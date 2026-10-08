//! How this computer opens a document (FR-011, data-model.md "Machine-local").
//! It lives in `machine.json` and never travels with a database.

use serde::{Deserialize, Serialize};

/// Whether opening a document previews it in HoploDex or hands it to another
/// app. Serialized as `"preview"` | `"external"`, as the frontend's
/// `DocumentOpening` has it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocumentOpening {
    /// The default: preview in HoploDex, which never writes a copy.
    #[default]
    Preview,
    /// Open in another app, after the native confirmation (FR-012).
    External,
}
