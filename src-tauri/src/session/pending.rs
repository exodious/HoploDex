//! Pending changes (FR-039, research.md §16): the open form's unsaved input,
//! staged in memory while the database is open and written into it only by
//! a lock or an OS shutdown, then offered at the next open. Housekeeping:
//! writing or removing them never makes a backup due, and a backup never
//! carries them.

use rusqlite::{params, Connection, OptionalExtension};

use crate::commands::CommandError;
use crate::models::database::{Draft, DraftKind, DraftMode, PendingAction, PendingSummary};
use crate::session::Session;

/// The most a draft's `values` may take, serialized (data-model.md).
pub const MAX_VALUES_BYTES: usize = 1 << 20;
/// The most characters a draft's label may have (data-model.md).
const MAX_LABEL_CHARS: usize = 200;

/// Checks a draft (data-model.md "Validation rules") and returns its values
/// serialized, as `pending_changes.values_json` keeps them: at most 1 MiB,
/// a kind and mode that go together (`coverage` only for a firearm, a
/// policy only added or edited), and a target for everything but an add.
pub fn validate_draft(draft: &Draft) -> Result<String, CommandError> {
    let invalid = |field: &str, message: &str| {
        Err(CommandError::validation(
            "The unsaved changes couldn't be kept.",
            [(field.to_owned(), message.to_owned())].into(),
        ))
    };
    let pair_valid = match draft.kind {
        DraftKind::Firearm => true,
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
pub fn stage(session: &Session, draft: Option<Draft>) -> Result<(), CommandError> {
    if let Some(draft) = &draft {
        validate_draft(draft)?;
    }
    session.inspect_mut(|open| {
        open.staged_draft = draft;
        Ok(())
    })
}

/// Writes `draft` as the database's pending changes, replacing any, and,
/// with `clear_marker`, clears the open marker in the same transaction
/// (FR-037's step 2 at sleep and shutdown). With no draft only the marker
/// is cleared.
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
/// treats a form version it doesn't know as not resumable.
pub fn summary(conn: &Connection) -> Result<Option<PendingSummary>, CommandError> {
    let row = conn
        .query_row(
            "SELECT kind, mode, target_id, label, saved_at FROM pending_changes",
            [],
            |row| {
                Ok((
                    row.get::<_, DraftKind>(0)?,
                    row.get::<_, DraftMode>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .optional()
        .map_err(CommandError::from_db)?;
    let Some((kind, mode, target_id, label, saved_at)) = row else { return Ok(None) };
    let resumable = match target_id {
        None => true,
        Some(id) => {
            let table = match kind {
                DraftKind::Firearm => "firearms",
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
    Ok(Some(PendingSummary { kind, mode, target_id, label, saved_at, resumable }))
}

/// Resumes or discards the database's pending changes (FR-039): either way
/// they are removed, and the collection can be used again. `resume` returns
/// the draft exactly as it was kept.
pub fn resolve(session: &Session, action: PendingAction) -> Result<Option<Draft>, CommandError> {
    session
        .write_housekeeping(|conn| {
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
            let draft = match (action, kept) {
                (
                    PendingAction::Resume,
                    Some((form_version, kind, mode, target_id, label, values)),
                ) => {
                    let values = serde_json::from_str(&values).map_err(|err| {
                        log::error!("kept pending changes are not JSON: {err}");
                        CommandError::new("INTERNAL_ERROR", "The unsaved changes couldn't be read.")
                    })?;
                    Some(Draft { form_version, kind, mode, target_id, label, values })
                }
                _ => None,
            };
            conn.execute("DELETE FROM pending_changes", []).map_err(CommandError::from_db)?;
            Ok(draft)
        })
        .and_then(|draft| {
            session.inspect_mut(|open| {
                open.pending_unresolved = false;
                Ok(())
            })?;
            Ok(draft)
        })
}
