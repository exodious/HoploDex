//! Pending changes (003 FR-039, research.md §16): the open form's unsaved input,
//! staged in memory while the database is open and written into it only by
//! a lock or an OS shutdown, then offered at the next open. Housekeeping:
//! writing or removing them never makes a backup due, and a backup never
//! carries them.

use rusqlite::{Connection, OptionalExtension, params};

use crate::commands::CommandError;
use crate::models::database::{
    Draft, DraftKind, DraftMode, PendingAction, PendingResolved, PendingState, PendingSummary,
};
use crate::session::{Session, SessionId};

/// The most a draft's `values` may take, serialized (003 data-model.md).
pub const MAX_VALUES_BYTES: usize = 1 << 20;
/// The most characters a draft's label may have (003 data-model.md).
const MAX_LABEL_CHARS: usize = 200;

/// Checks a draft (003 data-model.md "Validation rules") and returns its values
/// serialized, as `pending_changes.values_json` keeps them: at most 1 MiB,
/// a kind and mode that go together (a firearm or an accessory in any
/// mode, a policy only added or edited), and a target for everything but an add.
pub fn validate_draft(draft: &Draft) -> Result<String, CommandError> {
    let invalid = |field: &str, message: &str| {
        Err(CommandError::validation(
            "The unsaved changes couldn't be kept.",
            [(field.to_owned(), message.to_owned())].into(),
        ))
    };
    let pair_valid = match draft.kind {
        DraftKind::Firearm | DraftKind::Accessory => true,
        DraftKind::Policy => matches!(draft.mode, DraftMode::Add | DraftMode::Edit),
    };
    if !pair_valid {
        return invalid("mode", "This kind of form can't be kept in that mode.");
    }
    if draft.target_id.is_some() == (draft.mode == DraftMode::Add) {
        return invalid("targetId", "Only a new record has no target.");
    }
    if draft.label.chars().count() > MAX_LABEL_CHARS {
        return invalid("label", "Use at most 200 characters.");
    }
    let values = serde_json::to_string(&draft.values).map_err(|err| {
        log::error!("could not serialize a draft: {err}");
        CommandError::new("INTERNAL_ERROR", "The unsaved changes couldn't be kept.")
    })?;
    if values.len() > MAX_VALUES_BYTES {
        return invalid("values", "The unsaved changes are too large to keep.");
    }
    Ok(values)
}

/// Keeps `draft` as the open form's unsaved input, in memory only; `None`
/// clears it (`stage_pending_changes`). Nothing is written to the database.
pub fn stage(session: &Session, id: SessionId, draft: Option<Draft>) -> Result<(), CommandError> {
    if let Some(draft) = &draft {
        validate_draft(draft)?;
    }
    session.inspect_mut(id, |open| {
        open.staged_draft = draft;
        Ok(())
    })
}

/// Writes `draft` as the database's pending changes, replacing any, and,
/// with `clear_marker`, clears the open marker in the same transaction
/// (003 FR-037's step 2 at sleep and shutdown). A draft also records this build's
/// version as the one that saved the database (008 FR-015). With no draft only
/// the marker is cleared: a row already there, such as one resumed whose form never
/// opened, is left as it is (008 research.md §6).
pub fn write_pending(
    conn: &Connection,
    draft: Option<&Draft>,
    clear_marker: bool,
    saved_at: &str,
) -> Result<(), CommandError> {
    let values = draft.map(validate_draft).transpose()?;
    let tx = conn.unchecked_transaction().map_err(CommandError::from_db)?;
    if let (Some(draft), Some(values)) = (draft, values) {
        tx.execute(
            "INSERT OR REPLACE INTO pending_changes
                 (id, kind, mode, target_id, label, form_version, values_json, saved_at)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                draft.kind,
                draft.mode,
                draft.target_id,
                draft.label,
                draft.form_version,
                values,
                saved_at
            ],
        )
        .map_err(CommandError::from_db)?;
    }
    if draft.is_some() {
        // The version that kept the changes, atomically with them (008 FR-015,
        // research.md §16).
        crate::db::record_saved_version(&tx).map_err(CommandError::from_db)?;
    }
    if clear_marker {
        tx.execute(
            "UPDATE app_state SET open_machine_id = NULL, open_machine_name = NULL, open_since = NULL",
            [],
        )
        .map_err(CommandError::from_db)?;
    }
    tx.commit().map_err(CommandError::from_db)
}

/// The pending changes the database holds, if any, as the prompt describes
/// them. `resumable` is whether the record or policy they belong to still
/// exists: it may have been deleted on another computer. The frontend also
/// treats a form version it doesn't know as not resumable. `saved_by_version`
/// is the version that last saved the database, and `app_version` this
/// build's, so the dialog can word where the changes came from (008 FR-016).
pub fn summary(conn: &Connection) -> Result<Option<PendingSummary>, CommandError> {
    let row = conn
        .query_row(
            "SELECT kind, mode, target_id, label, saved_at, form_version FROM pending_changes",
            [],
            |row| {
                Ok((
                    row.get::<_, DraftKind>(0)?,
                    row.get::<_, DraftMode>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::from_db)?;
    let Some((kind, mode, target_id, label, saved_at, form_version)) = row else { return Ok(None) };
    let resumable = match target_id {
        None => true,
        Some(id) => {
            let table = match kind {
                DraftKind::Firearm => "firearms",
                DraftKind::Accessory => "accessories",
                DraftKind::Policy => "insurance_policies",
            };
            conn.query_row(
                &format!("SELECT EXISTS (SELECT 1 FROM {table} WHERE id = ?1)"),
                [id],
                |row| row.get(0),
            )
            .map_err(CommandError::from_db)?
        }
    };
    let saved_by_version = conn
        .query_row("SELECT last_saved_version FROM app_state", [], |row| row.get(0))
        .map_err(CommandError::from_db)?;
    Ok(Some(PendingSummary {
        form_version,
        kind,
        mode,
        target_id,
        label,
        saved_at,
        resumable,
        saved_by_version,
        app_version: crate::APP_VERSION.to_owned(),
    }))
}

/// The kept row as a draft, exactly as it was written.
fn kept_draft(conn: &Connection) -> Result<Option<Draft>, CommandError> {
    let kept = conn
        .query_row(
            "SELECT form_version, kind, mode, target_id, label, values_json
             FROM pending_changes",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, DraftKind>(1)?,
                    row.get::<_, DraftMode>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::from_db)?;
    let Some((form_version, kind, mode, target_id, label, values)) = kept else {
        return Ok(None);
    };
    let values = serde_json::from_str(&values).map_err(|err| {
        log::error!("kept pending changes are not JSON: {err}");
        CommandError::new("INTERNAL_ERROR", "The unsaved changes couldn't be read.")
    })?;
    Ok(Some(Draft { form_version, kind, mode, target_id, label, values }))
}

/// The row is gone but the session thought it was there: nothing a caller
/// can do, and nothing was changed.
fn row_missing() -> CommandError {
    log::error!("the pending changes the session expected are not in the database");
    CommandError::new("INTERNAL_ERROR", "The unsaved changes couldn't be read.")
}

/// Moves the database's pending changes along (003 FR-039; 008 FR-003,
/// research.md §6, data-model.md "PendingState"). The row, the state and the
/// session's staged draft change together, under one hold of the session's
/// mutex:
/// - `Resume` (from `Unresolved`) returns the draft and moves to `Resuming`;
///   the row stays, so a lock, close or crash before the form has opened
///   loses nothing;
/// - `Opened` (from `Resuming`) deletes the row and makes the draft the
///   session's staged draft, so a lock or sleep before the form's own first
///   staging writes it back;
/// - `NotOpened` (from `Resuming`) goes back to `Unresolved` and returns the
///   summary for the dialog;
/// - `Discard` (from `Unresolved`) deletes the row.
///
/// An action in the wrong state is a `VALIDATION_ERROR` that changes nothing.
pub fn resolve(
    session: &Session,
    id: SessionId,
    action: PendingAction,
) -> Result<PendingResolved, CommandError> {
    session.write_checked(id, false, |open| {
        let state = open.pending;
        let allowed = matches!(
            (state, action),
            (PendingState::Unresolved, PendingAction::Resume | PendingAction::Discard)
                | (PendingState::Resuming, PendingAction::Opened | PendingAction::NotOpened)
        );
        if !allowed {
            return Err(CommandError::validation(
                "That doesn't apply to the unsaved changes as they are now.",
                [("action".to_owned(), "Not allowed in this state.".to_owned())].into(),
            ));
        }
        let conn = &open.conn;
        let resolved = match action {
            PendingAction::Resume => {
                let draft = kept_draft(conn)?.ok_or_else(row_missing)?;
                open.pending = PendingState::Resuming;
                PendingResolved { draft: Some(draft), pending: None }
            }
            PendingAction::Opened => {
                let draft = kept_draft(conn)?.ok_or_else(row_missing)?;
                conn.execute("DELETE FROM pending_changes", []).map_err(CommandError::from_db)?;
                open.staged_draft = Some(draft);
                open.pending = PendingState::None;
                PendingResolved { draft: None, pending: None }
            }
            PendingAction::NotOpened => {
                let summary = summary(conn)?.ok_or_else(row_missing)?;
                // The row holds the changes; whatever the form that never
                // opened staged must not replace them at a lock.
                open.staged_draft = None;
                open.pending = PendingState::Unresolved;
                PendingResolved { draft: None, pending: Some(summary) }
            }
            PendingAction::Discard => {
                conn.execute("DELETE FROM pending_changes", []).map_err(CommandError::from_db)?;
                open.pending = PendingState::None;
                PendingResolved { draft: None, pending: None }
            }
        };
        Ok(resolved)
    })
}
