use std::collections::HashMap;

use serde::Serialize;

use crate::db::DbError;

/// The single error shape returned by every Tauri IPC command, per
/// contracts/tauri-commands.md. `code` is a stable, machine-matchable
/// identifier (e.g. `"VALIDATION_ERROR"`); `message` is safe to show
/// directly to the user; `field_errors` carries per-field validation
/// failures for form rendering.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_errors: Option<HashMap<String, String>>,
}

impl CommandError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into(), field_errors: None }
    }

    pub fn validation(message: impl Into<String>, field_errors: HashMap<String, String>) -> Self {
        Self {
            code: "VALIDATION_ERROR".into(),
            message: message.into(),
            field_errors: Some(field_errors),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new("NOT_FOUND", message)
    }

    /// Maps a raw `rusqlite::Error` to a safe, generic `CommandError` for
    /// use with `.map_err(CommandError::from_db)` at call sites — internal
    /// error detail is logged, never forwarded to the frontend.
    pub fn from_db(err: rusqlite::Error) -> Self {
        log::error!("db error: {err}");
        match err {
            rusqlite::Error::QueryReturnedNoRows => {
                Self::not_found("The requested record was not found.")
            }
            _ => Self::new("INTERNAL_ERROR", "An unexpected error occurred."),
        }
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for CommandError {}

/// Maps infrastructure failures (DB, keyring, filesystem) to a safe,
/// generic `CommandError` — internal error detail is logged, never
/// forwarded to the frontend, since it may include file paths or SQL text.
impl From<DbError> for CommandError {
    fn from(err: DbError) -> Self {
        log::error!("db error: {err}");
        match &err {
            DbError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => {
                Self::not_found("The requested record was not found.")
            }
            _ => Self::new("INTERNAL_ERROR", "An unexpected error occurred."),
        }
    }
}
