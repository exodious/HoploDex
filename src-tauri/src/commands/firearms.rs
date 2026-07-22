use rusqlite::{named_params, Connection, OptionalExtension};
use serde::Deserialize;
use tauri::State;

use crate::commands::CommandError;
use crate::db::DbHandle;
use crate::models::firearm::{
    validate_firearm_input, DispositionType, Firearm, FirearmInput, FirearmStatus,
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
