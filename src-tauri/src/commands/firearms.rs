use rusqlite::{Connection, OptionalExtension, named_params};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::CommandError;
use crate::models::disposition_history::DispositionHistoryEntry;
use crate::models::firearm::{
    DispositionType, Firearm, FirearmInput, FirearmStatus, Origin, validate_firearm_input,
};
use crate::models::record::{MountDetail, RecordLabel, RecordRef};
use crate::services::mounts::{self, MountGraph};
use crate::session::Session;

/// Input for the `dispose_firearm` and `dispose_accessory` commands, per
/// contracts/tauri-commands.md. Without `with_mounted`, equivalent to
/// calling `update_firearm` with `status: "disposed"` and these four fields
/// set.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisposeInput {
    pub disposition_type: DispositionType,
    pub recipient: String,
    pub date: String,
    /// The record's own price, required (research.md §9).
    pub price: i64,
    /// The records below it to dispose of with it (FR-014); everything else
    /// below it is kept.
    #[serde(default)]
    pub with_mounted: Vec<DisposeWith>,
}

/// One record disposed of along with the one being disposed (FR-014): it
/// takes the host's type, recipient and date, and its own price, which may
/// be absent.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct DisposeWith {
    pub record: RecordRef,
    pub price: Option<i64>,
}

/// What the user chose to do with the disposition being reversed (FR-033).
/// Required: the frontend asks and the backend never picks for them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryChoice {
    Keep,
    Discard,
}

/// Input for `reverse_disposition`. `nickname` renames the firearm in the
/// same step, to resolve a FR-031 clash; blank or absent keeps it.
/// `confirmed_warnings` resends after an `ORIGINAL_MARKS_MATCH` (FR-009,
/// specs/002-firearm-identification research.md §5).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReverseDispositionInput {
    pub history: HistoryChoice,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub confirmed_warnings: bool,
}

/// `get_firearm`'s output: the record plus its retained dispositions,
/// newest first (FR-033).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirearmDetail {
    #[serde(flatten)]
    pub firearm: Firearm,
    pub disposition_history: Vec<DispositionHistoryEntry>,
    /// specs/006-accessory-links FR-013: what the firearm is mounted on and
    /// what is mounted on it.
    pub mount: MountDetail,
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
    /// specs/002-firearm-identification US4-1: groups are returned in the
    /// fixed order Domestic, Imported, Re-imported, Unspecified, not sorted
    /// alphabetically like the other keys (research.md §10; the empty group
    /// renamed by specs/004-cartridges-action-types research.md §11).
    Origin,
    /// specs/004-cartridges-action-types FR-008: keyed by the stored
    /// cartridge text, so spellings already on record stay apart;
    /// alphabetical, with the firearms that have none last.
    Cartridge,
    /// specs/004-cartridges-action-types FR-020: keyed by the action's name,
    /// in the action list's order, with the firearms that have none last.
    ActionType,
    /// specs/005-regulated-item-types FR-016: keyed by the classification's
    /// name, in the list's order, with the firearms that have none last.
    RegisteredAs,
    /// specs/005-regulated-item-types FR-016: keyed by the stored "Registered
    /// to" text, alphabetical; every firearm with none, classified or not,
    /// is "Unspecified" and last.
    RegisteredTo,
}

/// The group of firearms with no value for the grouping field, always last
/// (specs/004-cartridges-action-types research.md §11: one term throughout).
pub const UNSPECIFIED: &str = "Unspecified";

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
    /// Shown alongside make and model wherever the firearm is named (FR-031).
    pub nickname: Option<String>,
    /// Tells apart firearms sharing a make and model in browse views.
    pub serial_number: Option<String>,
    pub caliber: String,
    /// specs/004-cartridges-action-types FR-027: shown with the caliber as
    /// "cartridge (caliber)".
    pub cartridge: Option<String>,
    pub firearm_type_name: String,
    /// specs/004-cartridges-action-types FR-020: the action's name, `None`
    /// when not recorded.
    pub action_type_name: Option<String>,
    /// specs/005-regulated-item-types FR-016: the classification's name,
    /// `None` when there is none.
    pub registered_as: Option<String>,
    pub status: FirearmStatus,
    pub thumbnail_photo_id: Option<i64>,
    pub generic_thumbnail_key: String,
    pub estimated_value: Option<i64>,
    pub insurance_warning: InsuranceWarning,
    /// Scheduled coverage (both null when unscheduled, i.e. covered by the
    /// blanket policy in force), so the insurance view can list each
    /// policy's firearms without fetching every full record.
    pub insurance_policy_id: Option<i64>,
    pub scheduled_coverage_amount: Option<i64>,
    /// specs/006-accessory-links FR-013: what it is mounted on directly.
    pub mounted_on: Option<RecordLabel>,
    /// FR-016a: everything below it, at any depth.
    pub mounted_count: usize,
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
        let mut firearm = conn
            .query_row(
                "SELECT * FROM firearms WHERE id = :id",
                named_params! { ":id": id },
                Firearm::from_row,
            )
            .optional()
            .map_err(CommandError::from_db)?
            .ok_or_else(|| CommandError::not_found("No firearm was found with that id."))?;
        firearm.mounted_on = mounts::host_of_record(conn, RecordRef::Firearm(id))?;
        Ok(firearm)
    }

    /// A record already in the collection that a new or edited firearm
    /// clashes with, described for the message naming it.
    struct Clash {
        make: String,
        model: String,
        nickname: Option<String>,
        serial_number: Option<String>,
    }

    impl Clash {
        fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
            Ok(Self {
                make: row.get(0)?,
                model: row.get(1)?,
                nickname: row.get(2)?,
                serial_number: row.get(3)?,
            })
        }
    }

    impl std::fmt::Display for Clash {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{} {}", self.make, self.model)?;
            if let Some(nickname) = &self.nickname {
                write!(f, " \"{nickname}\"")?;
            }
            if let Some(serial) = &self.serial_number {
                write!(f, " (serial {serial})")?;
            }
            Ok(())
        }
    }

    /// The active firearm (other than `exclude_id`, the record being edited,
    /// which never clashes with itself) whose nickname matches, ignoring case
    /// and surrounding whitespace.
    fn find_nickname_clash(
        conn: &Connection,
        exclude_id: Option<i64>,
        nickname: &str,
    ) -> Result<Option<Clash>, CommandError> {
        conn.query_row(
            "SELECT make, model, nickname, serial_number FROM firearms
             WHERE status = 'active' AND id IS NOT :exclude
               AND lower(trim(nickname)) = lower(trim(:nickname))",
            named_params! { ":exclude": exclude_id, ":nickname": nickname },
            Clash::from_row,
        )
        .optional()
        .map_err(CommandError::from_db)
    }

    /// The active firearm (other than `exclude_id`) with the same make,
    /// model and serial number, ignoring case and surrounding whitespace —
    /// unless both records carry a year of manufacture and the years differ
    /// (specs/002-firearm-identification FR-007/FR-008, data-model.md's
    /// "Identity uniqueness"; amends 001 FR-032).
    pub(crate) fn find_identity_clash(
        conn: &Connection,
        exclude_id: Option<i64>,
        make: &str,
        model: &str,
        serial_number: &str,
        year_of_manufacture: Option<i64>,
    ) -> Result<Option<i64>, CommandError> {
        conn.query_row(
            "SELECT id FROM firearms
             WHERE status = 'active' AND id IS NOT :exclude
               AND lower(trim(make)) = lower(trim(:make))
               AND lower(trim(model)) = lower(trim(:model))
               AND lower(trim(serial_number)) = lower(trim(:serial))
               AND NOT (
                   :year IS NOT NULL AND year_of_manufacture IS NOT NULL
                   AND year_of_manufacture <> :year
               )",
            named_params! {
                ":exclude": exclude_id,
                ":make": make,
                ":model": model,
                ":serial": serial_number,
                ":year": year_of_manufacture,
            },
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::from_db)
    }

    /// The other active firearm (other than `exclude_id`) whose original
    /// maker, model and serial number all match this record's, ignoring
    /// case and surrounding whitespace. `None` when any of the three is
    /// absent here (a partial set never warns) or there is no match
    /// (specs/002-firearm-identification FR-009, data-model.md's
    /// "Original-marks warning"). Served by `idx_firearms_original_serial`
    /// (research.md §5) so it stays cheap at 10,000 records.
    pub(crate) fn original_marks_clash(
        conn: &Connection,
        exclude_id: Option<i64>,
        original_make: Option<&str>,
        original_model: Option<&str>,
        original_serial_number: Option<&str>,
    ) -> Result<Option<i64>, CommandError> {
        let (Some(make), Some(model), Some(serial)) =
            (original_make, original_model, original_serial_number)
        else {
            return Ok(None);
        };
        conn.query_row(
            "SELECT id FROM firearms
             WHERE status = 'active' AND id IS NOT :exclude
               AND original_serial_number = :serial COLLATE NOCASE
               AND lower(trim(original_make)) = lower(trim(:make))
               AND lower(trim(original_model)) = lower(trim(:model))",
            named_params! {
                ":exclude": exclude_id,
                ":make": make,
                ":model": model,
                ":serial": serial,
            },
            |row| row.get(0),
        )
        .optional()
        .map_err(CommandError::from_db)
    }

    /// FR-009's non-blocking warning: stopped with `ORIGINAL_MARKS_MATCH`
    /// until the caller resends with `confirmed_warnings: true`. Never
    /// checked for a record that isn't active (a disposed save never
    /// matters here) or once confirmed. `dispose_firearm` and
    /// `assign_firearm_coverage` pass `confirmed_warnings: true` from their
    /// internal saves, since neither changes anything identifying
    /// (research.md §5).
    fn check_original_marks_warning(
        conn: &Connection,
        exclude_id: Option<i64>,
        input: &FirearmInput,
        confirmed_warnings: bool,
    ) -> Result<(), CommandError> {
        if confirmed_warnings || input.status != FirearmStatus::Active {
            return Ok(());
        }
        if let Some(other_id) = original_marks_clash(
            conn,
            exclude_id,
            input.original_make.as_deref(),
            input.original_model.as_deref(),
            input.original_serial_number.as_deref(),
        )? {
            return Err(CommandError::new(
                "ORIGINAL_MARKS_MATCH",
                original_marks_warning_message(conn, other_id)?,
            ));
        }
        Ok(())
    }

    /// The FR-009 warning's message, naming the other firearm — shared by
    /// `check_original_marks_warning` (create/update/restore) and import's
    /// `warnings` report (T048), which never blocks the row.
    pub(crate) fn original_marks_warning_message(
        conn: &Connection,
        other_id: i64,
    ) -> Result<String, CommandError> {
        let other = describe_firearm(conn, other_id)?;
        Ok(format!("{other} already has these original maker's marks."))
    }

    /// FR-031 and FR-032, against the other active firearms. A disposed
    /// record has released its nickname and never competes for an identity,
    /// so neither it nor a record with no serial number is compared. Every
    /// clash is reported at once, each on its own field.
    pub fn check_uniqueness(
        conn: &Connection,
        exclude_id: Option<i64>,
        input: &FirearmInput,
    ) -> Result<(), CommandError> {
        if input.status != FirearmStatus::Active {
            return Ok(());
        }
        let mut errors = std::collections::HashMap::new();

        if let Some(nickname) = &input.nickname
            && let Some(other) = find_nickname_clash(conn, exclude_id, nickname)?
        {
            errors.insert(
                "nickname".to_string(),
                format!("That nickname is already used by {other}."),
            );
        }

        if let Some(serial) = input.serial_number.as_deref().filter(|s| !s.trim().is_empty())
            && let Some(other_id) = find_identity_clash(
                conn,
                exclude_id,
                &input.make,
                &input.model,
                serial,
                input.year_of_manufacture,
            )?
        {
            let other = describe_firearm(conn, other_id)?;
            errors.insert(
                "serialNumber".to_string(),
                format!(
                    "{other} already has this make, model and serial number. \
                         Change one of them, or dispose of or delete the other record. \
                         Or record a year of manufacture on each firearm: two firearms with the \
                         same marks are accepted when both have a year and the years differ."
                ),
            );
        }

        if errors.is_empty() {
            return Ok(());
        }
        let mut messages: Vec<_> = errors.values().cloned().collect();
        messages.sort();
        Err(CommandError::validation(messages.join(" "), errors))
    }

    /// specs/005-regulated-item-types FR-003, FR-004, FR-022: an action, a
    /// barrel length or a capacity may be set only when the firearm's type
    /// says the field applies. One field error per offending field, "<Field>
    /// doesn't apply to a <type>." The backend never clears a value itself.
    /// Runs before `check_action_allowed`, because a type with no mapped
    /// actions allows every action there. The `firearms_fields_apply_*`
    /// triggers enforce the same rule, which only a bug would reach. Import
    /// calls this for every row.
    pub fn check_fields_apply(conn: &Connection, input: &FirearmInput) -> Result<(), CommandError> {
        if input.action_type_id.is_none()
            && input.barrel_length_hundredths.is_none()
            && input.capacity.is_none()
        {
            return Ok(());
        }
        // An unknown type id is left to the foreign key, as before.
        let Some((type_name, action, barrel, capacity)) = conn
            .query_row(
                "SELECT name, action_type_applies, barrel_length_applies, capacity_applies
                 FROM firearm_types WHERE id = :id",
                named_params! { ":id": input.firearm_type_id },
                |row| Ok((row.get::<_, String>(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(CommandError::from_db)?
        else {
            return Ok(());
        };
        let mut errors = std::collections::HashMap::new();
        let mut check = |is_set: bool, applies: bool, field: &str, label: &str| {
            if is_set && !applies {
                errors
                    .insert(field.to_string(), format!("{label} doesn't apply to a {type_name}."));
            }
        };
        check(input.action_type_id.is_some(), action, "actionTypeId", "Action");
        check(
            input.barrel_length_hundredths.is_some(),
            barrel,
            "barrelLengthHundredths",
            "Barrel length",
        );
        check(input.capacity.is_some(), capacity, "capacity", "Capacity");
        if errors.is_empty() {
            return Ok(());
        }
        let mut messages: Vec<_> = errors.values().cloned().collect();
        messages.sort();
        Err(CommandError::validation(messages.join(" "), errors))
    }

    /// specs/005-regulated-item-types FR-007: a classification must name one
    /// on the list. A row that is no longer offered is still accepted, so a
    /// record that holds it can be edited and a new one may still use it
    /// (research.md §5). Import calls this for every row.
    pub fn check_registration_class(
        conn: &Connection,
        input: &FirearmInput,
    ) -> Result<(), CommandError> {
        let Some(class_id) = input.registration_class_id else {
            return Ok(());
        };
        let known: bool = conn
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM registration_classes WHERE id = :id)",
                named_params! { ":id": class_id },
                |row| row.get(0),
            )
            .map_err(CommandError::from_db)?;
        if known {
            return Ok(());
        }
        let message = "Choose a classification from the list.".to_string();
        let errors =
            std::collections::HashMap::from([("registrationClassId".to_string(), message.clone())]);
        Err(CommandError::validation(message, errors))
    }

    /// specs/004-cartridges-action-types FR-017, FR-019, SC-008: the action
    /// must name an action on the list and be allowed for the firearm's type.
    /// A type with no mapped actions (Other, or one added later) allows all
    /// of them, and no action is always allowed. The same rule is enforced by
    /// the `firearms_action_allowed_*` triggers, which only a bug would
    /// reach. Import calls this for every row.
    pub fn check_action_allowed(
        conn: &Connection,
        firearm_type_id: i64,
        action_type_id: Option<i64>,
    ) -> Result<(), CommandError> {
        let Some(action_id) = action_type_id else {
            return Ok(());
        };
        let refuse = |message: String| {
            let errors =
                std::collections::HashMap::from([("actionTypeId".to_string(), message.clone())]);
            Err(CommandError::validation(message, errors))
        };
        let action_name: Option<String> = conn
            .query_row(
                "SELECT name FROM action_types WHERE id = :id",
                named_params! { ":id": action_id },
                |row| row.get(0),
            )
            .optional()
            .map_err(CommandError::from_db)?;
        let Some(action_name) = action_name else {
            return refuse("Choose an action from the list.".to_string());
        };
        let (mapped, allowed): (bool, bool) = conn
            .query_row(
                "SELECT COUNT(*) > 0, COALESCE(SUM(action_type_id = :action), 0) > 0
                 FROM firearm_type_actions WHERE firearm_type_id = :type",
                named_params! { ":action": action_id, ":type": firearm_type_id },
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(CommandError::from_db)?;
        if !mapped || allowed {
            return Ok(());
        }
        let type_name: String = conn
            .query_row(
                "SELECT name FROM firearm_types WHERE id = :id",
                named_params! { ":id": firearm_type_id },
                |row| row.get(0),
            )
            .map_err(CommandError::from_db)?;
        refuse(format!("{action_name} doesn't apply to a {type_name}."))
    }

    fn describe_firearm(conn: &Connection, id: i64) -> Result<Clash, CommandError> {
        conn.query_row(
            "SELECT make, model, nickname, serial_number FROM firearms WHERE id = :id",
            named_params! { ":id": id },
            Clash::from_row,
        )
        .map_err(CommandError::from_db)
    }

    pub fn create_firearm(
        conn: &Connection,
        input: &FirearmInput,
        confirmed_warnings: bool,
    ) -> Result<Firearm, CommandError> {
        create_firearm_with_uid(conn, input, confirmed_warnings, None)
    }

    /// [`create_firearm`], keeping `uid` as the record's identifier instead
    /// of generating one. Import passes the row's `record_id`, already
    /// parsed and checked unused (specs/006-accessory-links research.md
    /// §17); every other caller goes through `create_firearm`.
    pub fn create_firearm_with_uid(
        conn: &Connection,
        input: &FirearmInput,
        confirmed_warnings: bool,
        uid: Option<&str>,
    ) -> Result<Firearm, CommandError> {
        let input = &input.normalized();
        validate_firearm_input(input, None)?;
        check_fields_apply(conn, input)?;
        check_registration_class(conn, input)?;
        check_action_allowed(conn, input.firearm_type_id, input.action_type_id)?;
        check_uniqueness(conn, None, input)?;
        check_original_marks_warning(conn, None, input, confirmed_warnings)?;
        // The row and its mount stand or fall together (research.md §8).
        let id = mounts::atomically(conn, || {
            conn.execute(
                "INSERT INTO firearms (
                    uid, make, model, serial_number, no_serial_attested, caliber, cartridge, firearm_type_id,
                    action_type_id, nickname, notes, accessories,
                    barrel_length_hundredths, overall_length_hundredths, weight_tenths_oz,
                    capacity, finish, condition, status, estimated_value,
                    acquisition_source, acquisition_date, acquisition_price,
                    disposition_type, disposition_recipient, disposition_date, disposition_price,
                    insurance_policy_id, scheduled_coverage_amount,
                    origin, year_of_manufacture, country_of_manufacture, importer_name,
                    original_make, original_model, original_serial_number,
                    registration_class_id, registration_form, registration_approved, registered_to,
                    created_at, updated_at
                ) VALUES (
                    :uid, :make, :model, :serial_number, :no_serial_attested, :caliber, :cartridge, :firearm_type_id,
                    :action_type_id, :nickname, :notes, :accessories,
                    :barrel_length_hundredths, :overall_length_hundredths, :weight_tenths_oz,
                    :capacity, :finish, :condition, :status, :estimated_value,
                    :acquisition_source, :acquisition_date, :acquisition_price,
                    :disposition_type, :disposition_recipient, :disposition_date, :disposition_price,
                    :insurance_policy_id, :scheduled_coverage_amount,
                    :origin, :year_of_manufacture, :country_of_manufacture, :importer_name,
                    :original_make, :original_model, :original_serial_number,
                    :registration_class_id, :registration_form, :registration_approved, :registered_to,
                    datetime('now'), datetime('now')
                )",
                named_params! {
                    // FR-019: set here and never in an UPDATE.
                    ":uid": uid.map_or_else(crate::services::record_id::generate, str::to_owned),
                    ":make": input.make,
                    ":model": input.model,
                    ":serial_number": input.serial_number,
                    ":no_serial_attested": input.no_serial_attested,
                    ":caliber": input.caliber,
                    ":cartridge": input.cartridge,
                    ":firearm_type_id": input.firearm_type_id,
                    ":action_type_id": input.action_type_id,
                    ":nickname": input.nickname,
                    ":notes": input.notes,
                    ":accessories": input.accessories,
                    ":barrel_length_hundredths": input.barrel_length_hundredths,
                    ":overall_length_hundredths": input.overall_length_hundredths,
                    ":weight_tenths_oz": input.weight_tenths_oz,
                    ":capacity": input.capacity,
                    ":finish": input.finish,
                    ":condition": input.condition,
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
                    ":origin": input.origin,
                    ":year_of_manufacture": input.year_of_manufacture,
                    ":country_of_manufacture": input.country_of_manufacture,
                    ":importer_name": input.importer_name,
                    ":original_make": input.original_make,
                    ":original_model": input.original_model,
                    ":original_serial_number": input.original_serial_number,
                    ":registration_class_id": input.registration_class_id,
                    ":registration_form": input.registration_form,
                    ":registration_approved": input.registration_approved,
                    ":registered_to": input.registered_to,
                },
            )
            .map_err(CommandError::from_db)?;
            let id = conn.last_insert_rowid();
            mounts::save_form_mount(conn, RecordRef::Firearm(id), input.status, input.mounted_on)?;
            Ok(id)
        })?;

        get_firearm(conn, id)
    }

    pub fn update_firearm(
        conn: &Connection,
        id: i64,
        input: &FirearmInput,
        confirmed_warnings: bool,
    ) -> Result<Firearm, CommandError> {
        let input = &input.normalized();
        // A missing record is reported by the update below, after the
        // checks, as before.
        let stored = conn
            .query_row(
                "SELECT * FROM firearms WHERE id = :id",
                named_params! { ":id": id },
                Firearm::from_row,
            )
            .optional()
            .map_err(CommandError::from_db)?;
        validate_firearm_input(input, stored.as_ref())?;
        check_fields_apply(conn, input)?;
        check_registration_class(conn, input)?;
        check_action_allowed(conn, input.firearm_type_id, input.action_type_id)?;
        check_uniqueness(conn, Some(id), input)?;
        check_original_marks_warning(conn, Some(id), input, confirmed_warnings)?;
        // The row and its mount stand or fall together (research.md §8).
        mounts::atomically(conn, || {
            // A disposed record is in no mount (FR-014), and the status
            // triggers refuse the save while it is.
            if input.status == FirearmStatus::Disposed {
                mounts::clear_mounts(conn, RecordRef::Firearm(id))?;
            }
            let updated = conn
                .execute(
                    "UPDATE firearms SET
                        make = :make,
                        model = :model,
                        serial_number = :serial_number,
                        no_serial_attested = :no_serial_attested,
                        caliber = :caliber,
                        cartridge = :cartridge,
                        firearm_type_id = :firearm_type_id,
                        action_type_id = :action_type_id,
                        nickname = :nickname,
                        notes = :notes,
                        accessories = :accessories,
                        barrel_length_hundredths = :barrel_length_hundredths,
                        overall_length_hundredths = :overall_length_hundredths,
                        weight_tenths_oz = :weight_tenths_oz,
                        capacity = :capacity,
                        finish = :finish,
                        condition = :condition,
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
                        origin = :origin,
                        year_of_manufacture = :year_of_manufacture,
                        country_of_manufacture = :country_of_manufacture,
                        importer_name = :importer_name,
                        original_make = :original_make,
                        original_model = :original_model,
                        original_serial_number = :original_serial_number,
                        registration_class_id = :registration_class_id,
                        registration_form = :registration_form,
                        registration_approved = :registration_approved,
                        registered_to = :registered_to,
                        updated_at = datetime('now')
                    WHERE id = :id",
                    named_params! {
                        ":id": id,
                        ":make": input.make,
                        ":model": input.model,
                        ":serial_number": input.serial_number,
                        ":no_serial_attested": input.no_serial_attested,
                        ":caliber": input.caliber,
                        ":cartridge": input.cartridge,
                        ":firearm_type_id": input.firearm_type_id,
                        ":action_type_id": input.action_type_id,
                        ":nickname": input.nickname,
                        ":notes": input.notes,
                        ":accessories": input.accessories,
                        ":barrel_length_hundredths": input.barrel_length_hundredths,
                        ":overall_length_hundredths": input.overall_length_hundredths,
                        ":weight_tenths_oz": input.weight_tenths_oz,
                        ":capacity": input.capacity,
                        ":finish": input.finish,
                        ":condition": input.condition,
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
                        ":origin": input.origin,
                        ":year_of_manufacture": input.year_of_manufacture,
                        ":country_of_manufacture": input.country_of_manufacture,
                        ":importer_name": input.importer_name,
                        ":original_make": input.original_make,
                        ":original_model": input.original_model,
                        ":original_serial_number": input.original_serial_number,
                        ":registration_class_id": input.registration_class_id,
                        ":registration_form": input.registration_form,
                        ":registration_approved": input.registration_approved,
                        ":registered_to": input.registered_to,
                    },
                )
                .map_err(CommandError::from_db)?;
            if updated == 0 {
                return Err(CommandError::not_found("No firearm was found with that id."));
            }
            if input.status == FirearmStatus::Active {
                mounts::save_form_mount(
                    conn,
                    RecordRef::Firearm(id),
                    input.status,
                    input.mounted_on,
                )?;
            }
            Ok(())
        })?;
        get_firearm(conn, id)
    }

    /// Disposes of the firearm, and of the listed records mounted below it,
    /// in one step (research.md §9): see `mounts::ops::dispose_with_mounted`.
    pub fn dispose_firearm(
        conn: &Connection,
        id: i64,
        input: &DisposeInput,
    ) -> Result<Firearm, CommandError> {
        get_firearm(conn, id)?;
        crate::commands::mounts::ops::dispose_with_mounted(conn, RecordRef::Firearm(id), input)?;
        get_firearm(conn, id)
    }

    /// Saves the firearm alone as disposed, with the disposition of `input`
    /// and `price` (blank for a record disposed of along with its host).
    pub fn save_disposed(
        conn: &Connection,
        id: i64,
        input: &DisposeInput,
        price: Option<i64>,
    ) -> Result<Firearm, CommandError> {
        let current = get_firearm(conn, id)?;

        let updated_input = FirearmInput {
            status: FirearmStatus::Disposed,
            disposition_type: Some(input.disposition_type),
            disposition_recipient: Some(input.recipient.clone()),
            disposition_date: Some(input.date.clone()),
            disposition_price: price,
            ..FirearmInput::from(&current)
        };

        // Leaving active status behind, so FR-009 never applies here
        // (research.md §5); `confirmed_warnings: true` skips the check.
        update_firearm(conn, id, &updated_input, true)
    }

    pub fn get_firearm_detail(conn: &Connection, id: i64) -> Result<FirearmDetail, CommandError> {
        let firearm = get_firearm(conn, id)?;
        let mut stmt = conn
            .prepare(
                "SELECT * FROM disposition_history WHERE firearm_id = :id
                 ORDER BY reversed_at DESC, id DESC",
            )
            .map_err(CommandError::from_db)?;
        let disposition_history = stmt
            .query_map(named_params! { ":id": id }, DispositionHistoryEntry::from_row)
            .map_err(CommandError::from_db)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::from_db)?;
        let mount = mounts::detail(conn, RecordRef::Firearm(id))?;
        Ok(FirearmDetail { firearm, disposition_history, mount })
    }

    /// Restores a disposed firearm to active status (FR-033). With `keep`
    /// the current disposition is copied to `disposition_history` first.
    /// Runs in one transaction, and the restored record goes through the
    /// same save path as an edit, so FR-031/FR-032 are re-applied against
    /// the firearms active now: a clash fails the whole reversal, naming
    /// the other record, with nothing changed.
    pub fn reverse_disposition(
        conn: &Connection,
        id: i64,
        input: &ReverseDispositionInput,
    ) -> Result<Firearm, CommandError> {
        let current = get_firearm(conn, id)?;
        let (Some(disposition_type), Some(recipient), Some(date)) = (
            current.disposition_type,
            current.disposition_recipient.clone(),
            current.disposition_date.clone(),
        ) else {
            return Err(CommandError::new(
                "VALIDATION_ERROR",
                "Only a disposed firearm can have its disposition reversed.",
            ));
        };
        if current.status != FirearmStatus::Disposed {
            return Err(CommandError::new(
                "VALIDATION_ERROR",
                "Only a disposed firearm can have its disposition reversed.",
            ));
        }

        let restored = FirearmInput {
            status: FirearmStatus::Active,
            disposition_type: None,
            disposition_recipient: None,
            disposition_date: None,
            disposition_price: None,
            nickname: match input.nickname.as_deref().map(str::trim) {
                Some(renamed) if !renamed.is_empty() => Some(renamed.to_owned()),
                _ => current.nickname.clone(),
            },
            ..FirearmInput::from(&current)
        };

        // Dropped without commit on any error below, which rolls it back.
        let tx = conn.unchecked_transaction().map_err(CommandError::from_db)?;
        if input.history == HistoryChoice::Keep {
            let owner = RecordRef::Firearm(id).owner_columns();
            conn.execute(
                "INSERT INTO disposition_history (
                    firearm_id, accessory_id, disposition_type, disposition_recipient,
                    disposition_date, disposition_price, reversed_at
                ) VALUES (
                    :firearm_id, :accessory_id, :type, :recipient, :date, :price,
                    datetime('now')
                )",
                named_params! {
                    ":firearm_id": owner.0,
                    ":accessory_id": owner.1,
                    ":type": disposition_type,
                    ":recipient": recipient,
                    ":date": date,
                    ":price": current.disposition_price,
                },
            )
            .map_err(CommandError::from_db)?;
        }
        let restored = update_firearm(conn, id, &restored, input.confirmed_warnings)?;
        tx.commit().map_err(CommandError::from_db)?;
        Ok(restored)
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
        // Its photos, documents and search entry went with it: return their
        // space and leave none of their content behind (Constitution V).
        crate::db::reclaim_deleted_record(conn);
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
        // The index is trigram-tokenized (0002_fts5.sql), so a quoted phrase
        // matches as contiguous text anywhere inside a value: "365" finds
        // "P365 XL", "cracked handle" finds those two words together.
        // `:has_query` short-circuits the MATCH subquery entirely when there's
        // no query, since MATCH errors on an empty/absent search string.
        // Trigrams need three characters, and a shorter search matches
        // nothing in the index, so one or two characters ("Co" while typing
        // Colt) are looked up with LIKE over the same values instead. They
        // are read from `firearms` and its joins, not the index's columns:
        // the index is external-content, so its type, action, origin and
        // country values only exist inside its triggers (0002_fts5.sql).
        let trimmed = input.query.as_deref().map(str::trim).unwrap_or_default();
        let short_query = trimmed.chars().count() < 3;
        let fts_query = if short_query {
            String::new()
        } else {
            format!("\"{}\"", trimmed.replace('"', "\"\""))
        };
        let like_query = if short_query {
            let escaped = trimmed.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
            format!("%{escaped}%")
        } else {
            String::new()
        };

        let insurance = crate::services::insurance_status::load_context(conn)?;

        // Only the columns a summary needs, read by position: at 10,000
        // records, decoding every column and looking each up by name took
        // most of the 500ms budget (Principle IV).
        let mut stmt = conn
            .prepare(
                "SELECT f.id, f.make, f.model, f.nickname, f.serial_number, f.caliber, f.cartridge,
                        f.status, f.thumbnail_photo_id, f.estimated_value, f.insurance_policy_id,
                        f.scheduled_coverage_amount, f.origin,
                        ft.name, ft.generic_thumbnail_key, at.name, rc.name, f.registered_to
                 FROM firearms f
                 JOIN firearm_types ft ON ft.id = f.firearm_type_id
                 LEFT JOIN action_types at ON at.id = f.action_type_id
                 LEFT JOIN registration_classes rc ON rc.id = f.registration_class_id
                 WHERE (:include_disposed = 1 OR f.status = 'active')
                   AND (:has_query = 0
                        OR (:short_query = 0 AND f.id IN (SELECT rowid FROM firearms_fts WHERE firearms_fts MATCH :query))
                        OR (:short_query = 1 AND (
                            f.make LIKE :like ESCAPE '\\'
                            OR f.model LIKE :like ESCAPE '\\'
                            OR f.nickname LIKE :like ESCAPE '\\'
                            OR f.serial_number LIKE :like ESCAPE '\\'
                            OR f.caliber LIKE :like ESCAPE '\\'
                            OR f.notes LIKE :like ESCAPE '\\'
                            OR f.accessories LIKE :like ESCAPE '\\'
                            OR f.finish LIKE :like ESCAPE '\\'
                            OR f.year_of_manufacture LIKE :like ESCAPE '\\'
                            OR f.importer_name LIKE :like ESCAPE '\\'
                            OR f.original_make LIKE :like ESCAPE '\\'
                            OR f.original_model LIKE :like ESCAPE '\\'
                            OR f.original_serial_number LIKE :like ESCAPE '\\'
                            OR f.cartridge LIKE :like ESCAPE '\\'
                            OR ft.name LIKE :like ESCAPE '\\'
                            OR at.name LIKE :like ESCAPE '\\'
                            OR rc.name LIKE :like ESCAPE '\\'
                            OR f.registration_form LIKE :like ESCAPE '\\'
                            OR f.registered_to LIKE :like ESCAPE '\\'
                            OR (CASE f.origin WHEN 'domestic' THEN 'Domestic' WHEN 'imported' THEN 'Imported' WHEN 'reimported' THEN 'Re-imported' END) LIKE :like ESCAPE '\\'
                            OR (CASE WHEN f.origin = 'reimported' THEN 'United States' ELSE f.country_of_manufacture END) LIKE :like ESCAPE '\\')))
                 ORDER BY f.make, f.model",
            )
            .map_err(CommandError::from_db)?;
        let rows = stmt
            .query_map(
                named_params! {
                    ":include_disposed": input.include_disposed,
                    ":has_query": has_query,
                    ":short_query": short_query,
                    ":query": fts_query,
                    ":like": like_query,
                },
                |row| {
                    let estimated_value = row.get(9)?;
                    let insurance_policy_id = row.get(10)?;
                    let scheduled_coverage_amount = row.get(11)?;
                    let origin: Option<Origin> = row.get(12)?;
                    let summary = FirearmSummary {
                        id: row.get(0)?,
                        make: row.get(1)?,
                        model: row.get(2)?,
                        nickname: row.get(3)?,
                        serial_number: row.get(4)?,
                        caliber: row.get(5)?,
                        cartridge: row.get(6)?,
                        status: row.get(7)?,
                        thumbnail_photo_id: row.get(8)?,
                        estimated_value,
                        insurance_warning: crate::services::insurance_status::record_warning(
                            estimated_value,
                            insurance_policy_id,
                            scheduled_coverage_amount,
                            &insurance,
                        ),
                        insurance_policy_id,
                        scheduled_coverage_amount,
                        firearm_type_name: row.get(13)?,
                        generic_thumbnail_key: row.get(14)?,
                        action_type_name: row.get(15)?,
                        registered_as: row.get(16)?,
                        mounted_on: None,
                        mounted_count: 0,
                    };
                    let registered_to: Option<String> = row.get(17)?;
                    Ok((summary, origin, registered_to))
                },
            )
            .map_err(CommandError::from_db)?;

        // FR-013, FR-016a: the whole mount graph once, and one label query
        // per table for every host named (research.md §5, §22).
        let graph = MountGraph::load(conn)?;
        let mut rows_read = Vec::new();
        for row in rows {
            rows_read.push(row.map_err(CommandError::from_db)?);
        }
        let host_labels = {
            let hosts: Vec<RecordRef> = rows_read
                .iter()
                .filter_map(|(summary, _, _)| graph.host_of(RecordRef::Firearm(summary.id)))
                .collect();
            mounts::labels(conn, &hosts)?
        };
        let mut counts = std::collections::HashMap::new();

        let mut summaries = Vec::new();
        for (mut summary, origin, registered_to) in rows_read {
            let record = RecordRef::Firearm(summary.id);
            summary.mounted_on =
                graph.host_of(record).and_then(|host| host_labels.get(&host).cloned());
            summary.mounted_count = graph.count_below(record, &mut counts);
            let key = match input.group_by {
                Some(GroupBy::Type) => summary.firearm_type_name.clone(),
                Some(GroupBy::Caliber) => summary.caliber.clone(),
                Some(GroupBy::Make) => summary.make.clone(),
                Some(GroupBy::Origin) => {
                    origin.map_or_else(|| UNSPECIFIED.to_string(), |o| o.label().to_string())
                }
                Some(GroupBy::Cartridge) => {
                    summary.cartridge.clone().unwrap_or_else(|| UNSPECIFIED.to_string())
                }
                Some(GroupBy::ActionType) => {
                    summary.action_type_name.clone().unwrap_or_else(|| UNSPECIFIED.to_string())
                }
                Some(GroupBy::RegisteredAs) => {
                    summary.registered_as.clone().unwrap_or_else(|| UNSPECIFIED.to_string())
                }
                Some(GroupBy::RegisteredTo) => {
                    registered_to.unwrap_or_else(|| UNSPECIFIED.to_string())
                }
                None => "All".to_string(),
            };
            summaries.push((key, summary));
        }

        // A map from key to group index, not a search of the groups for
        // every row (research.md §12, §13).
        let mut groups: Vec<FirearmGroup> = Vec::new();
        let mut group_index: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for (key, summary) in summaries {
            match group_index.get(&key) {
                Some(index) => groups[*index].firearms.push(summary),
                None => {
                    group_index.insert(key.clone(), groups.len());
                    groups.push(FirearmGroup { key, firearms: vec![summary] });
                }
            }
        }
        match input.group_by {
            Some(GroupBy::Origin) => {
                // specs/002-firearm-identification US4-1/research.md §10: a
                // fixed order, not alphabetical, so "Unspecified" is always
                // last rather than sorting between "Imported" and
                // "Re-imported".
                const ORIGIN_ORDER: [&str; 4] =
                    ["Domestic", "Imported", "Re-imported", UNSPECIFIED];
                groups.sort_by_key(|g| {
                    ORIGIN_ORDER.iter().position(|o| *o == g.key).unwrap_or(ORIGIN_ORDER.len())
                });
            }
            // specs/004-cartridges-action-types research.md §11: firearms
            // with no cartridge last, the rest alphabetical.
            Some(GroupBy::Cartridge) => groups.sort_by(|a, b| {
                (a.key == UNSPECIFIED, &a.key).cmp(&(b.key == UNSPECIFIED, &b.key))
            }),
            // FR-020: the action list's order, not alphabetical; "Unspecified"
            // is not on the list, so it sorts last.
            Some(GroupBy::ActionType) => {
                let mut stmt = conn
                    .prepare("SELECT name FROM action_types ORDER BY sort_order")
                    .map_err(CommandError::from_db)?;
                let order = stmt
                    .query_map([], |row| row.get::<_, String>(0))
                    .map_err(CommandError::from_db)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(CommandError::from_db)?;
                groups.sort_by_key(|g| {
                    order.iter().position(|name| *name == g.key).unwrap_or(order.len())
                });
            }
            // FR-016: the classification list's order, "Unspecified" last.
            Some(GroupBy::RegisteredAs) => {
                let mut stmt = conn
                    .prepare("SELECT name FROM registration_classes ORDER BY sort_order")
                    .map_err(CommandError::from_db)?;
                let order = stmt
                    .query_map([], |row| row.get::<_, String>(0))
                    .map_err(CommandError::from_db)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(CommandError::from_db)?;
                groups.sort_by_key(|g| {
                    order.iter().position(|name| *name == g.key).unwrap_or(order.len())
                });
            }
            // FR-016: alphabetical by stored text, "Unspecified" last.
            Some(GroupBy::RegisteredTo) => groups.sort_by(|a, b| {
                (a.key == UNSPECIFIED, &a.key).cmp(&(b.key == UNSPECIFIED, &b.key))
            }),
            _ => groups.sort_by(|a, b| a.key.cmp(&b.key)),
        }

        Ok(ListFirearmsOutput { groups })
    }
}

#[tauri::command]
pub async fn create_firearm(
    input: FirearmInput,
    confirmed_warnings: Option<bool>,
    session: State<'_, Session>,
) -> Result<Firearm, CommandError> {
    session.write(|conn| ops::create_firearm(conn, &input, confirmed_warnings.unwrap_or(false)))
}

#[tauri::command]
pub async fn update_firearm(
    id: i64,
    input: FirearmInput,
    confirmed_warnings: Option<bool>,
    session: State<'_, Session>,
) -> Result<Firearm, CommandError> {
    session.write(|conn| ops::update_firearm(conn, id, &input, confirmed_warnings.unwrap_or(false)))
}

#[tauri::command]
pub async fn dispose_firearm(
    id: i64,
    input: DisposeInput,
    session: State<'_, Session>,
) -> Result<Firearm, CommandError> {
    session.write(|conn| ops::dispose_firearm(conn, id, &input))
}

#[tauri::command]
pub async fn reverse_disposition(
    id: i64,
    input: ReverseDispositionInput,
    session: State<'_, Session>,
) -> Result<Firearm, CommandError> {
    session.write(|conn| ops::reverse_disposition(conn, id, &input))
}

#[tauri::command]
pub async fn delete_firearm(
    id: i64,
    confirmed: bool,
    session: State<'_, Session>,
) -> Result<DeleteResult, CommandError> {
    session.write(|conn| ops::delete_firearm(conn, id, confirmed))
}

#[tauri::command]
pub async fn get_firearm(
    id: i64,
    session: State<'_, Session>,
) -> Result<FirearmDetail, CommandError> {
    session.read(|conn| ops::get_firearm_detail(conn, id))
}

#[tauri::command]
pub async fn list_firearms(
    input: ListFirearmsInput,
    session: State<'_, Session>,
) -> Result<ListFirearmsOutput, CommandError> {
    session.read(|conn| ops::list_firearms(conn, &input))
}
