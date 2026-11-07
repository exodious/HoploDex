//! The troubleshooting log's state and the choice this computer has saved
//! (013 data-model.md).

use serde::{Deserialize, Serialize};

/// What applies now. Only [`LogChoice`] is ever saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LogState {
    Off,
    On,
    Detailed,
}

/// The user's saved choice (`troubleshootingLog` in `machine.json`). Absent
/// means follow the version's default (013 FR-001, FR-002).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LogChoice {
    On,
    Off,
}

/// What `get_troubleshooting_log` and `set_troubleshooting_log` return
/// (013 contracts/tauri-commands.md). A path, never the contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TroubleshootingLog {
    pub state: LogState,
    pub path: Option<String>,
    pub exists: bool,
    pub failed: bool,
}
