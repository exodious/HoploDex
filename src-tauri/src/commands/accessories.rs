//! specs/006-accessory-links contracts/tauri-commands.md "Accessories (new)":
//! the accessory commands. Each `#[tauri::command]` is thin and calls the
//! matching function in `ops`, which takes a `&Connection`.

use rusqlite::{Connection, OptionalExtension, named_params};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::CommandError;
use crate::commands::firearms::{DeleteResult, DisposeInput, HistoryChoice};
use crate::models::accessory::{Accessory, AccessoryInput, validate_accessory_input};
use crate::models::disposition_history::DispositionHistoryEntry;
use crate::models::firearm::FirearmStatus;
use crate::models::record::{MountDetail, RecordLabel, RecordRef};
use crate::services::insurance_status::InsuranceWarning;
use crate::services::mounts::{self, MountGraph};
use crate::session::Session;

/// Input for `reverse_accessory_disposition`. An accessory has no nickname
/// and no identity, so there is nothing to re-check or rename (FR-006).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReverseAccessoryDispositionInput {
    pub history: HistoryChoice,
}

/// `get_accessory`'s output: the record, its retained dispositions (newest
/// first) and what it is mounted on and carries (FR-013).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessoryDetail {
    #[serde(flatten)]
    pub accessory: Accessory,
    pub disposition_history: Vec<DispositionHistoryEntry>,
    pub mount: MountDetail,
}

/// Input for `list_accessories`. The search text and the grouping are User
/// Story 4's; an input that carries them is read without them until then.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListAccessoriesInput {
    #[serde(default)]
    pub include_disposed: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccessorySummary {
    pub id: i64,
    pub accessory_kind_id: i64,
    pub kind_name: String,
    pub generic_thumbnail_key: String,
    pub make: Option<String>,
    pub model: Option<String>,
    pub serial_number: Option<String>,
    pub caliber: Option<String>,
    pub cartridge: Option<String>,
    pub status: FirearmStatus,
    pub thumbnail_photo_id: Option<i64>,
    pub estimated_value: Option<i64>,
    pub insurance_warning: InsuranceWarning,
    pub insurance_policy_id: Option<i64>,
    pub scheduled_coverage_amount: Option<i64>,
    /// The direct host only (FR-013).
    pub mounted_on: Option<RecordLabel>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccessoryGroup {
    pub key: String,
    /// Set only when grouped by `mounted_on`, for a host's group.
    pub host: Option<RecordLabel>,
    pub accessories: Vec<AccessorySummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ListAccessoriesOutput {
    pub groups: Vec<AccessoryGroup>,
}

/// Pure, `Connection`-based business logic, mirroring
/// `commands::firearms::ops`.
pub mod ops {
    use super::*;

    const NOT_FOUND: &str = "No accessory was found with that id.";

    fn find_accessory(conn: &Connection, id: i64) -> Result<Option<Accessory>, CommandError> {
        conn.query_row(
            "SELECT * FROM accessories WHERE id = :id",
            named_params! { ":id": id },
            Accessory::from_row,
        )
        .optional()
        .map_err(CommandError::from_db)
    }

    /// The record as stored, with its direct host but without its history.
    fn load_accessory(conn: &Connection, id: i64) -> Result<Accessory, CommandError> {
        let mut accessory =
            find_accessory(conn, id)?.ok_or_else(|| CommandError::not_found(NOT_FOUND))?;
        accessory.mounted_on = mounts::host_of_record(conn, RecordRef::Accessory(id))?;
        Ok(accessory)
    }

    /// The validation of `validate_accessory_input` plus the one rule that
    /// needs the database: the kind must exist, offered or not (FR-002).
    /// Both are reported together, so a form shows every field at once.
    fn validate(
        conn: &Connection,
        input: &AccessoryInput,
        stored: Option<&Accessory>,
    ) -> Result<(), CommandError> {
        let kind_known: bool = conn
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM accessory_kinds WHERE id = :id)",
                named_params! { ":id": input.accessory_kind_id },
                |row| row.get(0),
            )
            .map_err(CommandError::from_db)?;
        match validate_accessory_input(input, stored) {
            Ok(()) if kind_known => Ok(()),
            Ok(()) => {
                let message = "Choose a kind.";
                Err(CommandError::validation(message, Default::default())
                    .on_field("accessoryKindId", message))
            }
            Err(err) if kind_known => Err(err),
            Err(err) => Err(err.on_field("accessoryKindId", "Choose a kind.")),
        }
    }

    pub fn create_accessory(
        conn: &Connection,
        input: &AccessoryInput,
    ) -> Result<Accessory, CommandError> {
        let input = &input.normalized();
        validate(conn, input, None)?;
        // The row and its mount stand or fall together (research.md §8).
        let id = mounts::atomically(conn, || {
            conn.execute(
                "INSERT INTO accessories (
                    uid, accessory_kind_id, make, model, serial_number, caliber, cartridge, notes,
                    status, estimated_value, acquisition_source, acquisition_date, acquisition_price,
                    disposition_type, disposition_recipient, disposition_date, disposition_price,
                    insurance_policy_id, scheduled_coverage_amount, created_at, updated_at
                ) VALUES (
                    :uid, :accessory_kind_id, :make, :model, :serial_number, :caliber, :cartridge, :notes,
                    :status, :estimated_value, :acquisition_source, :acquisition_date, :acquisition_price,
                    :disposition_type, :disposition_recipient, :disposition_date, :disposition_price,
                    :insurance_policy_id, :scheduled_coverage_amount, datetime('now'), datetime('now')
                )",
                named_params! {
                    // FR-019: set here and never in an UPDATE.
                    ":uid": crate::services::record_id::generate(),
                    ":accessory_kind_id": input.accessory_kind_id,
                    ":make": input.make,
                    ":model": input.model,
                    ":serial_number": input.serial_number,
                    ":caliber": input.caliber,
                    ":cartridge": input.cartridge,
                    ":notes": input.notes,
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
                    ":scheduled_coverage_amount": input.scheduled_coverage_amount,
                },
            )
            .map_err(CommandError::from_db)?;
            let id = conn.last_insert_rowid();
            mounts::save_form_mount(
                conn,
                RecordRef::Accessory(id),
                input.status,
                input.mounted_on,
            )?;
            Ok(id)
        })?;

        load_accessory(conn, id)
    }

    pub fn update_accessory(
        conn: &Connection,
        id: i64,
        input: &AccessoryInput,
    ) -> Result<Accessory, CommandError> {
        let input = &input.normalized();
        // A missing record is reported by the update below, after the
        // checks, as for a firearm.
        let stored = find_accessory(conn, id)?;
        validate(conn, input, stored.as_ref())?;
        // The row and its mount stand or fall together (research.md §8).
        mounts::atomically(conn, || {
            // A disposed record is in no mount (FR-014), and the status
            // triggers refuse the save while it is.
            if input.status == FirearmStatus::Disposed {
                mounts::clear_mounts(conn, RecordRef::Accessory(id))?;
            }
            let updated = conn
                .execute(
                    "UPDATE accessories SET
                        accessory_kind_id = :accessory_kind_id,
                        make = :make,
                        model = :model,
                        serial_number = :serial_number,
                        caliber = :caliber,
                        cartridge = :cartridge,
                        notes = :notes,
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
                        scheduled_coverage_amount = :scheduled_coverage_amount,
                        updated_at = datetime('now')
                    WHERE id = :id",
                    named_params! {
                        ":id": id,
                        ":accessory_kind_id": input.accessory_kind_id,
                        ":make": input.make,
                        ":model": input.model,
                        ":serial_number": input.serial_number,
                        ":caliber": input.caliber,
                        ":cartridge": input.cartridge,
                        ":notes": input.notes,
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
                        ":scheduled_coverage_amount": input.scheduled_coverage_amount,
                    },
                )
                .map_err(CommandError::from_db)?;

            if updated == 0 {
                return Err(CommandError::not_found(NOT_FOUND));
            }
            if input.status == FirearmStatus::Active {
                mounts::save_form_mount(
                    conn,
                    RecordRef::Accessory(id),
                    input.status,
                    input.mounted_on,
                )?;
            }
            Ok(())
        })?;
        load_accessory(conn, id)
    }

    pub fn get_accessory(conn: &Connection, id: i64) -> Result<AccessoryDetail, CommandError> {
        let accessory = load_accessory(conn, id)?;
        let mut stmt = conn
            .prepare(
                "SELECT * FROM disposition_history WHERE accessory_id = :id
                 ORDER BY reversed_at DESC, id DESC",
            )
            .map_err(CommandError::from_db)?;
        let disposition_history = stmt
            .query_map(named_params! { ":id": id }, DispositionHistoryEntry::from_row)
            .map_err(CommandError::from_db)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::from_db)?;
        let mount = mounts::detail(conn, RecordRef::Accessory(id))?;
        Ok(AccessoryDetail { accessory, disposition_history, mount })
    }

    /// Disposes of the accessory, and of the listed records mounted below
    /// it, in one step (research.md §9): see
    /// `mounts::ops::dispose_with_mounted`.
    pub fn dispose_accessory(
        conn: &Connection,
        id: i64,
        input: &DisposeInput,
    ) -> Result<Accessory, CommandError> {
        load_accessory(conn, id)?;
        crate::commands::mounts::ops::dispose_with_mounted(conn, RecordRef::Accessory(id), input)?;
        load_accessory(conn, id)
    }

    /// The same checks and result as saving the accessory alone with
    /// `status: "disposed"`, the disposition of `input` and `price` (blank
    /// for a record disposed of along with its host).
    pub fn save_disposed(
        conn: &Connection,
        id: i64,
        input: &DisposeInput,
        price: Option<i64>,
    ) -> Result<Accessory, CommandError> {
        let current = load_accessory(conn, id)?;
        let updated_input = AccessoryInput {
            status: FirearmStatus::Disposed,
            disposition_type: Some(input.disposition_type),
            disposition_recipient: Some(input.recipient.clone()),
            disposition_date: Some(input.date.clone()),
            disposition_price: price,
            ..AccessoryInput::from(&current)
        };
        update_accessory(conn, id, &updated_input)
    }

    /// Restores a disposed accessory to active status (FR-006). With `keep`
    /// the current disposition is copied to `disposition_history` first.
    /// Runs in one transaction. No nickname or identity is re-checked: an
    /// accessory has neither (FR-004).
    pub fn reverse_accessory_disposition(
        conn: &Connection,
        id: i64,
        input: &ReverseAccessoryDispositionInput,
    ) -> Result<Accessory, CommandError> {
        let current = load_accessory(conn, id)?;
        let only_disposed = || {
            CommandError::new(
                "VALIDATION_ERROR",
                "Only a disposed accessory can have its disposition reversed.",
            )
        };
        let (Some(disposition_type), Some(recipient), Some(date)) = (
            current.disposition_type,
            current.disposition_recipient.clone(),
            current.disposition_date.clone(),
        ) else {
            return Err(only_disposed());
        };
        if current.status != FirearmStatus::Disposed {
            return Err(only_disposed());
        }

        let restored = AccessoryInput {
            status: FirearmStatus::Active,
            disposition_type: None,
            disposition_recipient: None,
            disposition_date: None,
            disposition_price: None,
            ..AccessoryInput::from(&current)
        };

        // Dropped without commit on any error below, which rolls it back.
        let tx = conn.unchecked_transaction().map_err(CommandError::from_db)?;
        if input.history == HistoryChoice::Keep {
            conn.execute(
                "INSERT INTO disposition_history (
                    accessory_id, disposition_type, disposition_recipient,
                    disposition_date, disposition_price, reversed_at
                ) VALUES (:accessory_id, :type, :recipient, :date, :price, datetime('now'))",
                named_params! {
                    ":accessory_id": id,
                    ":type": disposition_type,
                    ":recipient": recipient,
                    ":date": date,
                    ":price": current.disposition_price,
                },
            )
            .map_err(CommandError::from_db)?;
        }
        let restored = update_accessory(conn, id, &restored)?;
        tx.commit().map_err(CommandError::from_db)?;
        Ok(restored)
    }

    pub fn delete_accessory(
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
        // ON DELETE CASCADE removes the accessory's photos, documents,
        // retained dispositions and mounts.
        let deleted = conn
            .execute("DELETE FROM accessories WHERE id = :id", named_params! { ":id": id })
            .map_err(CommandError::from_db)?;
        if deleted == 0 {
            return Err(CommandError::not_found(NOT_FOUND));
        }
        // Its photos, documents and search entry went with it: return their
        // space and leave none of their content behind (Constitution V).
        crate::db::reclaim_deleted_record(conn);
        Ok(DeleteResult { deleted: true })
    }

    /// The plain list (FR-016): one group "All" ordered by make, model, then
    /// the kind's place in the kind list. Disposed accessories only on request.
    pub fn list_accessories(
        conn: &Connection,
        input: &ListAccessoriesInput,
    ) -> Result<ListAccessoriesOutput, CommandError> {
        let insurance = crate::services::insurance_status::load_context(conn)?;

        // Only the columns a summary needs, read by position (as
        // `list_firearms`, for the same 500 ms budget).
        let mut stmt = conn
            .prepare(
                "SELECT a.id, a.accessory_kind_id, k.name, k.generic_thumbnail_key,
                        a.make, a.model, a.serial_number, a.caliber, a.cartridge, a.status,
                        a.thumbnail_photo_id, a.estimated_value, a.insurance_policy_id,
                        a.scheduled_coverage_amount
                 FROM accessories a
                 JOIN accessory_kinds k ON k.id = a.accessory_kind_id
                 WHERE (:include_disposed = 1 OR a.status = 'active')
                 ORDER BY a.make, a.model, k.sort_order, a.id",
            )
            .map_err(CommandError::from_db)?;
        let mut accessories = stmt
            .query_map(named_params! { ":include_disposed": input.include_disposed }, |row| {
                let estimated_value = row.get(11)?;
                let insurance_policy_id = row.get(12)?;
                let scheduled_coverage_amount = row.get(13)?;
                Ok(AccessorySummary {
                    id: row.get(0)?,
                    accessory_kind_id: row.get(1)?,
                    kind_name: row.get(2)?,
                    generic_thumbnail_key: row.get(3)?,
                    make: row.get(4)?,
                    model: row.get(5)?,
                    serial_number: row.get(6)?,
                    caliber: row.get(7)?,
                    cartridge: row.get(8)?,
                    status: row.get(9)?,
                    thumbnail_photo_id: row.get(10)?,
                    estimated_value,
                    insurance_warning: crate::services::insurance_status::record_warning(
                        estimated_value,
                        insurance_policy_id,
                        scheduled_coverage_amount,
                        &insurance,
                    ),
                    insurance_policy_id,
                    scheduled_coverage_amount,
                    mounted_on: None,
                })
            })
            .map_err(CommandError::from_db)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::from_db)?;

        // FR-013: the direct host's label, from the whole graph once and one
        // label query per table (research.md §5, §22).
        let graph = MountGraph::load(conn)?;
        let hosts: Vec<RecordRef> = accessories
            .iter()
            .filter_map(|summary| graph.host_of(RecordRef::Accessory(summary.id)))
            .collect();
        let host_labels = mounts::labels(conn, &hosts)?;
        for summary in &mut accessories {
            summary.mounted_on = graph
                .host_of(RecordRef::Accessory(summary.id))
                .and_then(|host| host_labels.get(&host).cloned());
        }

        Ok(ListAccessoriesOutput {
            groups: vec![AccessoryGroup { key: "All".to_string(), host: None, accessories }],
        })
    }
}

#[tauri::command]
pub async fn create_accessory(
    input: AccessoryInput,
    session: State<'_, Session>,
) -> Result<Accessory, CommandError> {
    session.write(|conn| ops::create_accessory(conn, &input))
}

#[tauri::command]
pub async fn update_accessory(
    id: i64,
    input: AccessoryInput,
    session: State<'_, Session>,
) -> Result<Accessory, CommandError> {
    session.write(|conn| ops::update_accessory(conn, id, &input))
}

#[tauri::command]
pub async fn get_accessory(
    id: i64,
    session: State<'_, Session>,
) -> Result<AccessoryDetail, CommandError> {
    session.read(|conn| ops::get_accessory(conn, id))
}

#[tauri::command]
pub async fn list_accessories(
    input: ListAccessoriesInput,
    session: State<'_, Session>,
) -> Result<ListAccessoriesOutput, CommandError> {
    session.read(|conn| ops::list_accessories(conn, &input))
}

#[tauri::command]
pub async fn dispose_accessory(
    id: i64,
    input: DisposeInput,
    session: State<'_, Session>,
) -> Result<Accessory, CommandError> {
    session.write(|conn| ops::dispose_accessory(conn, id, &input))
}

#[tauri::command]
pub async fn reverse_accessory_disposition(
    id: i64,
    input: ReverseAccessoryDispositionInput,
    session: State<'_, Session>,
) -> Result<Accessory, CommandError> {
    session.write(|conn| ops::reverse_accessory_disposition(conn, id, &input))
}

#[tauri::command]
pub async fn delete_accessory(
    id: i64,
    confirmed: bool,
    session: State<'_, Session>,
) -> Result<DeleteResult, CommandError> {
    session.write(|conn| ops::delete_accessory(conn, id, confirmed))
}
