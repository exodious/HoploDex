use rusqlite::{named_params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::CommandError;
use crate::models::disposition_history::DispositionHistoryEntry;
use crate::models::firearm::{
    validate_firearm_input, DispositionType, Firearm, FirearmInput, FirearmStatus,
};
use crate::session::Session;

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
    /// fixed order Domestic, Imported, Re-imported, Not specified, not
    /// sorted alphabetically like the other keys (research.md §10).
    Origin,
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
    /// Shown alongside make and model wherever the firearm is named (FR-031).
    pub nickname: Option<String>,
    /// Tells apart firearms sharing a make and model in browse views.
    pub serial_number: Option<String>,
    pub caliber: String,
    pub firearm_type_name: String,
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

        if let Some(nickname) = &input.nickname {
            if let Some(other) = find_nickname_clash(conn, exclude_id, nickname)? {
                errors.insert(
                    "nickname".to_string(),
                    format!("That nickname is already used by {other}."),
                );
            }
        }

        if let Some(serial) = input.serial_number.as_deref().filter(|s| !s.trim().is_empty()) {
            if let Some(other_id) = find_identity_clash(
                conn,
                exclude_id,
                &input.make,
                &input.model,
                serial,
                input.year_of_manufacture,
            )? {
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
        }

        if errors.is_empty() {
            return Ok(());
        }
        let mut messages: Vec<_> = errors.values().cloned().collect();
        messages.sort();
        Err(CommandError::validation(messages.join(" "), errors))
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
        let input = &input.normalized();
        validate_firearm_input(input)?;
        check_uniqueness(conn, None, input)?;
        check_original_marks_warning(conn, None, input, confirmed_warnings)?;
        conn.execute(
            "INSERT INTO firearms (
                make, model, serial_number, no_serial_attested, caliber, firearm_type_id, nickname,
                notes, accessories,
                barrel_length_hundredths, overall_length_hundredths, weight_tenths_oz,
                capacity, finish, condition, status, estimated_value,
                acquisition_source, acquisition_date, acquisition_price,
                disposition_type, disposition_recipient, disposition_date, disposition_price,
                insurance_policy_id, scheduled_coverage_amount,
                origin, year_of_manufacture, country_of_manufacture, importer_name,
                original_make, original_model, original_serial_number,
                created_at, updated_at
            ) VALUES (
                :make, :model, :serial_number, :no_serial_attested, :caliber, :firearm_type_id, :nickname,
                :notes, :accessories,
                :barrel_length_hundredths, :overall_length_hundredths, :weight_tenths_oz,
                :capacity, :finish, :condition, :status, :estimated_value,
                :acquisition_source, :acquisition_date, :acquisition_price,
                :disposition_type, :disposition_recipient, :disposition_date, :disposition_price,
                :insurance_policy_id, :scheduled_coverage_amount,
                :origin, :year_of_manufacture, :country_of_manufacture, :importer_name,
                :original_make, :original_model, :original_serial_number,
                datetime('now'), datetime('now')
            )",
            named_params! {
                ":make": input.make,
                ":model": input.model,
                ":serial_number": input.serial_number,
                ":no_serial_attested": input.no_serial_attested,
                ":caliber": input.caliber,
                ":firearm_type_id": input.firearm_type_id,
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
            },
        )
        .map_err(CommandError::from_db)?;

        get_firearm(conn, conn.last_insert_rowid())
    }

    pub fn update_firearm(
        conn: &Connection,
        id: i64,
        input: &FirearmInput,
        confirmed_warnings: bool,
    ) -> Result<Firearm, CommandError> {
        let input = &input.normalized();
        validate_firearm_input(input)?;
        check_uniqueness(conn, Some(id), input)?;
        check_original_marks_warning(conn, Some(id), input, confirmed_warnings)?;
        let updated = conn
            .execute(
                "UPDATE firearms SET
                    make = :make,
                    model = :model,
                    serial_number = :serial_number,
                    no_serial_attested = :no_serial_attested,
                    caliber = :caliber,
                    firearm_type_id = :firearm_type_id,
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
            status: FirearmStatus::Disposed,
            disposition_type: Some(input.disposition_type),
            disposition_recipient: Some(input.recipient.clone()),
            disposition_date: Some(input.date.clone()),
            disposition_price: Some(input.price),
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
        Ok(FirearmDetail { firearm, disposition_history })
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
            conn.execute(
                "INSERT INTO disposition_history (
                    firearm_id, disposition_type, disposition_recipient,
                    disposition_date, disposition_price, reversed_at
                ) VALUES (:firearm_id, :type, :recipient, :date, :price, datetime('now'))",
                named_params! {
                    ":firearm_id": id,
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
        // Its photos and documents went with it: return their space (Constitution V).
        crate::db::reclaim_freed_space(conn);
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

        let insurance = crate::services::insurance_status::load_context(conn)?;

        let mut summaries = Vec::new();
        for row in rows {
            let (firearm, firearm_type_name, generic_thumbnail_key) =
                row.map_err(CommandError::from_db)?;
            let insurance_warning = crate::services::insurance_status::firearm_warning(
                firearm.estimated_value,
                firearm.insurance_policy_id,
                firearm.scheduled_coverage_amount,
                &insurance,
            );
            summaries.push((
                match input.group_by {
                    Some(GroupBy::Type) => firearm_type_name.clone(),
                    Some(GroupBy::Caliber) => firearm.caliber.clone(),
                    Some(GroupBy::Make) => firearm.make.clone(),
                    Some(GroupBy::Origin) => firearm
                        .origin
                        .map(|o| o.label().to_string())
                        .unwrap_or_else(|| "Not specified".to_string()),
                    None => "All".to_string(),
                },
                FirearmSummary {
                    id: firearm.id,
                    make: firearm.make,
                    model: firearm.model,
                    nickname: firearm.nickname,
                    serial_number: firearm.serial_number,
                    caliber: firearm.caliber,
                    firearm_type_name,
                    status: firearm.status,
                    thumbnail_photo_id: firearm.thumbnail_photo_id,
                    generic_thumbnail_key,
                    estimated_value: firearm.estimated_value,
                    insurance_warning,
                    insurance_policy_id: firearm.insurance_policy_id,
                    scheduled_coverage_amount: firearm.scheduled_coverage_amount,
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
        if input.group_by == Some(GroupBy::Origin) {
            // specs/002-firearm-identification US4-1/research.md §10: a
            // fixed order, not alphabetical, so "Not specified" is always
            // last rather than sorting between "Imported" and
            // "Re-imported".
            const ORIGIN_ORDER: [&str; 4] =
                ["Domestic", "Imported", "Re-imported", "Not specified"];
            groups.sort_by_key(|g| {
                ORIGIN_ORDER.iter().position(|o| *o == g.key).unwrap_or(ORIGIN_ORDER.len())
            });
        } else {
            groups.sort_by(|a, b| a.key.cmp(&b.key));
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
    input: DisposeFirearmInput,
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
