use rusqlite::{named_params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::CommandError;
use crate::db::DbHandle;
use crate::models::firearm::{
    validate_firearm_input, CoverageKind, DispositionType, Firearm, FirearmInput, FirearmStatus,
};

/// Input for the `dispose_firearm` command, per contracts/tauri-commands.md.
/// Equivalent to calling `update_firearm` with `status: "disposed"` and
/// these four fields set.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisposeFirearmInput {
    pub disposition_type: DispositionType,
    pub recipient: String,
    pub date: String,
    pub price: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DeleteResult {
    pub deleted: bool,
}

/// Grouping key for `list_firearms`, per contracts/tauri-commands.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupBy {
    Type,
    Caliber,
    Make,
}

/// Input for the `list_firearms` command (User Story 2), per
/// contracts/tauri-commands.md.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListFirearmsInput {
    pub query: Option<String>,
    pub group_by: Option<GroupBy>,
    #[serde(default)]
    pub include_disposed: bool,
    /// Informational only — list/tile views return the same data shape.
    pub view: Option<String>,
}

/// Per-firearm insurance-warning flag surfaced in browse views, so
/// uninsured/under-insured firearms are always visibly flagged (SC-004).
/// The actual uninsured/under-insured/blanket-exceeded/expired-override
/// decision lives in `services::insurance_status` (shared with
/// `get_value_summary` so the browse view and value summary never
/// disagree).
pub use crate::services::insurance_status::InsuranceWarning;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FirearmSummary {
    pub id: i64,
    pub make: String,
    pub model: String,
    /// Tells apart firearms sharing a make and model in browse views.
    pub serial_number: Option<String>,
    pub caliber: String,
    pub firearm_type_name: String,
    pub status: FirearmStatus,
    pub thumbnail_photo_id: Option<i64>,
    pub generic_thumbnail_key: String,
    pub estimated_value: Option<i64>,
    pub insurance_warning: InsuranceWarning,
    /// Coverage assignment, so the insurance view can list each policy's
    /// firearms without fetching every full record.
    pub insurance_policy_id: Option<i64>,
    pub coverage_kind: Option<CoverageKind>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FirearmGroup {
    pub key: String,
    pub firearms: Vec<FirearmSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ListFirearmsOutput {
    pub groups: Vec<FirearmGroup>,
}

/// Pure, `Connection`-based business logic — no `tauri::State`/`AppHandle`
/// involved, so integration tests can call these directly against a real
/// temporary SQLCipher database (constitution: no mocks) without needing to
/// mock Tauri's IPC layer. The `#[tauri::command]` functions below are thin
/// wrappers that just acquire the connection lock and delegate here.
pub mod ops {
    use super::*;

    pub fn get_firearm(conn: &Connection, id: i64) -> Result<Firearm, CommandError> {
        conn.query_row(
            "SELECT * FROM firearms WHERE id = :id",
            named_params! { ":id": id },
            Firearm::from_row,
        )
        .optional()
        .map_err(CommandError::from_db)?
        .ok_or_else(|| CommandError::not_found("No firearm was found with that id."))
    }

    pub fn create_firearm(
        conn: &Connection,
        input: &FirearmInput,
    ) -> Result<Firearm, CommandError> {
        validate_firearm_input(input)?;
        conn.execute(
            "INSERT INTO firearms (
                make, model, serial_number, no_serial_attested, caliber, firearm_type_id,
                notes, accessories, status, estimated_value,
                acquisition_source, acquisition_date, acquisition_price,
                disposition_type, disposition_recipient, disposition_date, disposition_price,
                insurance_policy_id, coverage_kind, scheduled_coverage_amount,
                created_at, updated_at
            ) VALUES (
                :make, :model, :serial_number, :no_serial_attested, :caliber, :firearm_type_id,
                :notes, :accessories, :status, :estimated_value,
                :acquisition_source, :acquisition_date, :acquisition_price,
                :disposition_type, :disposition_recipient, :disposition_date, :disposition_price,
                :insurance_policy_id, :coverage_kind, :scheduled_coverage_amount,
                datetime('now'), datetime('now')
            )",
            named_params! {
                ":make": input.make,
                ":model": input.model,
                ":serial_number": input.serial_number,
                ":no_serial_attested": input.no_serial_attested,
                ":caliber": input.caliber,
                ":firearm_type_id": input.firearm_type_id,
                ":notes": input.notes,
                ":accessories": input.accessories,
                ":status": input.status,
                ":estimated_value": input.estimated_value,
                ":acquisition_source": input.acquisition_source,
                ":acquisition_date": input.acquisition_date,
                ":acquisition_price": input.acquisition_price,
                ":disposition_type": input.disposition_type,
                ":disposition_recipient": input.disposition_recipient,
                ":disposition_date": input.disposition_date,
                ":disposition_price": input.disposition_price,
                ":insurance_policy_id": input.insurance_policy_id,
                ":coverage_kind": input.coverage_kind,
                ":scheduled_coverage_amount": input.scheduled_coverage_amount,
            },
        )
        .map_err(CommandError::from_db)?;

        get_firearm(conn, conn.last_insert_rowid())
    }

    pub fn update_firearm(
        conn: &Connection,
        id: i64,
        input: &FirearmInput,
    ) -> Result<Firearm, CommandError> {
        validate_firearm_input(input)?;
        let updated = conn
            .execute(
                "UPDATE firearms SET
                    make = :make,
                    model = :model,
                    serial_number = :serial_number,
                    no_serial_attested = :no_serial_attested,
                    caliber = :caliber,
                    firearm_type_id = :firearm_type_id,
                    notes = :notes,
                    accessories = :accessories,
                    status = :status,
                    estimated_value = :estimated_value,
                    acquisition_source = :acquisition_source,
                    acquisition_date = :acquisition_date,
                    acquisition_price = :acquisition_price,
                    disposition_type = :disposition_type,
                    disposition_recipient = :disposition_recipient,
                    disposition_date = :disposition_date,
                    disposition_price = :disposition_price,
                    insurance_policy_id = :insurance_policy_id,
                    coverage_kind = :coverage_kind,
                    scheduled_coverage_amount = :scheduled_coverage_amount,
                    updated_at = datetime('now')
                WHERE id = :id",
                named_params! {
                    ":id": id,
                    ":make": input.make,
                    ":model": input.model,
                    ":serial_number": input.serial_number,
                    ":no_serial_attested": input.no_serial_attested,
                    ":caliber": input.caliber,
                    ":firearm_type_id": input.firearm_type_id,
                    ":notes": input.notes,
                    ":accessories": input.accessories,
                    ":status": input.status,
                    ":estimated_value": input.estimated_value,
                    ":acquisition_source": input.acquisition_source,
                    ":acquisition_date": input.acquisition_date,
                    ":acquisition_price": input.acquisition_price,
                    ":disposition_type": input.disposition_type,
                    ":disposition_recipient": input.disposition_recipient,
                    ":disposition_date": input.disposition_date,
                    ":disposition_price": input.disposition_price,
                    ":insurance_policy_id": input.insurance_policy_id,
                    ":coverage_kind": input.coverage_kind,
                    ":scheduled_coverage_amount": input.scheduled_coverage_amount,
                },
            )
            .map_err(CommandError::from_db)?;

        if updated == 0 {
            return Err(CommandError::not_found("No firearm was found with that id."));
        }
        get_firearm(conn, id)
    }

    pub fn dispose_firearm(
        conn: &Connection,
        id: i64,
        input: &DisposeFirearmInput,
    ) -> Result<Firearm, CommandError> {
        let current = get_firearm(conn, id)?;

        let updated_input = FirearmInput {
            make: current.make,
            model: current.model,
            serial_number: current.serial_number,
            no_serial_attested: current.no_serial_attested,
            caliber: current.caliber,
            firearm_type_id: current.firearm_type_id,
            notes: current.notes,
            accessories: current.accessories,
            status: FirearmStatus::Disposed,
            estimated_value: current.estimated_value,
            acquisition_source: current.acquisition_source,
            acquisition_date: current.acquisition_date,
            acquisition_price: current.acquisition_price,
            disposition_type: Some(input.disposition_type),
            disposition_recipient: Some(input.recipient.clone()),
            disposition_date: Some(input.date.clone()),
            disposition_price: Some(input.price),
            insurance_policy_id: current.insurance_policy_id,
            coverage_kind: current.coverage_kind,
            scheduled_coverage_amount: current.scheduled_coverage_amount,
        };

        update_firearm(conn, id, &updated_input)
    }

    pub fn delete_firearm(
        conn: &Connection,
        id: i64,
        confirmed: bool,
    ) -> Result<DeleteResult, CommandError> {
        if !confirmed {
            return Err(CommandError::new(
                "CONFIRMATION_REQUIRED",
                "Deletion must be explicitly confirmed.",
            ));
        }
        // ON DELETE CASCADE (foreign_keys=ON, set at connection open) removes
        // this firearm's Photos and DocumentAttachments automatically.
        let deleted = conn
            .execute("DELETE FROM firearms WHERE id = :id", named_params! { ":id": id })
            .map_err(CommandError::from_db)?;

        if deleted == 0 {
            return Err(CommandError::not_found("No firearm was found with that id."));
        }
        Ok(DeleteResult { deleted: true })
    }

    /// Browse/search/group across the collection (User Story 2). Runs a
    /// single indexed query (FTS5 for `query`, indexed columns otherwise)
    /// so it stays within the 500ms/10k-record budget (Principle IV)
    /// rather than scanning and grouping in application code.
    pub fn list_firearms(
        conn: &Connection,
        input: &ListFirearmsInput,
    ) -> Result<ListFirearmsOutput, CommandError> {
        let has_query = input.query.as_deref().is_some_and(|q| !q.trim().is_empty());
        // FTS5 MATCH treats bare whitespace-separated tokens as an implicit
        // AND; wrapping the whole query as a quoted phrase instead matches
        // it as contiguous text, which is what a user searching "cracked
        // handle" or a multi-word caliber value expects. `:has_query`
        // short-circuits the MATCH subquery entirely when there's no query,
        // since MATCH errors on an empty/absent search string. The trailing
        // `*` makes the phrase's last word a prefix, so results appear while
        // it's still being typed ("Rem" finds Remington); a query with no
        // words at all can't take one, so it stays an exact phrase.
        let fts_query = input
            .query
            .as_deref()
            .map(|q| {
                let phrase = format!("\"{}\"", q.trim().replace('"', "\"\""));
                if q.chars().any(char::is_alphanumeric) {
                    phrase + "*"
                } else {
                    phrase
                }
            })
            .unwrap_or_default();

        let mut stmt = conn
            .prepare(
                "SELECT f.*, ft.name AS firearm_type_name, ft.generic_thumbnail_key AS generic_thumbnail_key
                 FROM firearms f
                 JOIN firearm_types ft ON ft.id = f.firearm_type_id
                 WHERE (:include_disposed = 1 OR f.status = 'active')
                   AND (:has_query = 0 OR f.id IN (SELECT rowid FROM firearms_fts WHERE firearms_fts MATCH :query))
                 ORDER BY f.make, f.model",
            )
            .map_err(CommandError::from_db)?;
        let rows = stmt
            .query_map(
                named_params! {
                    ":include_disposed": input.include_disposed,
                    ":has_query": has_query,
                    ":query": fts_query,
                },
                |row| {
                    let firearm = Firearm::from_row(row)?;
                    let firearm_type_name: String = row.get("firearm_type_name")?;
                    let generic_thumbnail_key: String = row.get("generic_thumbnail_key")?;
                    Ok((firearm, firearm_type_name, generic_thumbnail_key))
                },
            )
            .map_err(CommandError::from_db)?;

        let policy_aggregates = crate::services::insurance_status::load_policy_aggregates(conn)?;

        let mut summaries = Vec::new();
        for row in rows {
            let (firearm, firearm_type_name, generic_thumbnail_key) =
                row.map_err(CommandError::from_db)?;
            let insurance_warning = crate::services::insurance_status::firearm_warning(
                firearm.estimated_value,
                firearm.insurance_policy_id,
                firearm.coverage_kind,
                firearm.scheduled_coverage_amount,
                &policy_aggregates,
            );
            summaries.push((
                match input.group_by {
                    Some(GroupBy::Type) => firearm_type_name.clone(),
                    Some(GroupBy::Caliber) => firearm.caliber.clone(),
                    Some(GroupBy::Make) => firearm.make.clone(),
                    None => "All".to_string(),
                },
                FirearmSummary {
                    id: firearm.id,
                    make: firearm.make,
                    model: firearm.model,
                    serial_number: firearm.serial_number,
                    caliber: firearm.caliber,
                    firearm_type_name,
                    status: firearm.status,
                    thumbnail_photo_id: firearm.thumbnail_photo_id,
                    generic_thumbnail_key,
                    estimated_value: firearm.estimated_value,
                    insurance_warning,
                    insurance_policy_id: firearm.insurance_policy_id,
                    coverage_kind: firearm.coverage_kind,
                },
            ));
        }

        let mut groups: Vec<FirearmGroup> = Vec::new();
        for (key, summary) in summaries {
            match groups.iter_mut().find(|g| g.key == key) {
                Some(group) => group.firearms.push(summary),
                None => groups.push(FirearmGroup { key, firearms: vec![summary] }),
            }
        }
        groups.sort_by(|a, b| a.key.cmp(&b.key));

        Ok(ListFirearmsOutput { groups })
    }
}

#[tauri::command]
pub async fn create_firearm(
    input: FirearmInput,
    state: State<'_, DbHandle>,
) -> Result<Firearm, CommandError> {
    let conn = state.0.lock().expect("db mutex poisoned");
    ops::create_firearm(&conn, &input)
}

#[tauri::command]
pub async fn update_firearm(
    id: i64,
    input: FirearmInput,
    state: State<'_, DbHandle>,
) -> Result<Firearm, CommandError> {
    let conn = state.0.lock().expect("db mutex poisoned");
    ops::update_firearm(&conn, id, &input)
}

#[tauri::command]
pub async fn dispose_firearm(
    id: i64,
    input: DisposeFirearmInput,
    state: State<'_, DbHandle>,
) -> Result<Firearm, CommandError> {
    let conn = state.0.lock().expect("db mutex poisoned");
    ops::dispose_firearm(&conn, id, &input)
}

#[tauri::command]
pub async fn delete_firearm(
    id: i64,
    confirmed: bool,
    state: State<'_, DbHandle>,
) -> Result<DeleteResult, CommandError> {
    let conn = state.0.lock().expect("db mutex poisoned");
    ops::delete_firearm(&conn, id, confirmed)
}

#[tauri::command]
pub async fn get_firearm(id: i64, state: State<'_, DbHandle>) -> Result<Firearm, CommandError> {
    let conn = state.0.lock().expect("db mutex poisoned");
    ops::get_firearm(&conn, id)
}

#[tauri::command]
pub async fn list_firearms(
    input: ListFirearmsInput,
    state: State<'_, DbHandle>,
) -> Result<ListFirearmsOutput, CommandError> {
    let conn = state.0.lock().expect("db mutex poisoned");
    ops::list_firearms(&conn, &input)
}
