//! specs/006-accessory-links contracts/tauri-commands.md "Accessories (new)":
//! the accessory commands. Each `#[tauri::command]` is thin and calls the
//! matching function in `ops`, which takes a `&Connection`.

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, named_params};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::CommandError;
use crate::commands::firearms::{DeleteResult, DisposeInput, HistoryChoice, UNSPECIFIED};
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

/// Grouping key for `list_accessories`, per contracts/tauri-commands.md
/// "Group order" (FR-017).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessoryGroupBy {
    /// The kind list's order.
    Kind,
    /// Alphabetical, "Unspecified" last.
    Make,
    /// Alphabetical, "Unspecified" last.
    Caliber,
    /// Alphabetical, "Unspecified" last.
    Cartridge,
    /// One group per host record, sorted by its name, "Not mounted" last.
    MountedOn,
}

/// The group of accessories that sit on no host, always last (FR-017).
pub const NOT_MOUNTED: &str = "Not mounted";

/// Input for `list_accessories` (FR-016 to FR-018).
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListAccessoriesInput {
    /// Free text over every accessory text field (FR-018): three or more
    /// characters through `accessories_fts`, one or two through `LIKE`.
    pub query: Option<String>,
    pub group_by: Option<AccessoryGroupBy>,
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
        create_accessory_with_uid(conn, input, None)
    }

    /// [`create_accessory`], keeping `uid` as the record's identifier
    /// instead of generating one. Import passes the row's `record_id`,
    /// already parsed and checked unused (research.md §17).
    pub fn create_accessory_with_uid(
        conn: &Connection,
        input: &AccessoryInput,
        uid: Option<&str>,
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
                    ":uid": uid.map_or_else(crate::services::record_id::generate, str::to_owned),
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

    /// The list (FR-016 to FR-018): searched, then grouped as asked. Within a
    /// group the order is make, model, then the kind's place in the kind
    /// list. Disposed accessories only on request.
    pub fn list_accessories(
        conn: &Connection,
        input: &ListAccessoriesInput,
    ) -> Result<ListAccessoriesOutput, CommandError> {
        // The index is trigram-tokenized (0002_fts5.sql): a quoted phrase
        // matches as contiguous text anywhere inside a value, as
        // `list_firearms` quotes it. A search of one or two characters
        // matches nothing in the index, so it is looked up with LIKE over
        // the same eight values (the kind's name is joined, not stored).
        // `:has_query` short-circuits the MATCH subquery when there is no
        // query, since MATCH errors on an empty search string.
        let trimmed = input.query.as_deref().map(str::trim).unwrap_or_default();
        let has_query = !trimmed.is_empty();
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

        // Only the columns a summary needs, read by position (as
        // `list_firearms`, for the same 500 ms budget).
        let mut stmt = conn
            .prepare(
                "SELECT a.id, a.accessory_kind_id, k.name, k.generic_thumbnail_key,
                        a.make, a.model, a.serial_number, a.caliber, a.cartridge, a.status,
                        a.thumbnail_photo_id, a.estimated_value, a.insurance_policy_id,
                        a.scheduled_coverage_amount, k.sort_order
                 FROM accessories a
                 JOIN accessory_kinds k ON k.id = a.accessory_kind_id
                 WHERE (:include_disposed = 1 OR a.status = 'active')
                   AND (:has_query = 0
                        OR (:short_query = 0 AND a.id IN
                              (SELECT rowid FROM accessories_fts WHERE accessories_fts MATCH :query))
                        OR (:short_query = 1 AND (
                            k.name LIKE :like ESCAPE '\\'
                            OR a.make LIKE :like ESCAPE '\\'
                            OR a.model LIKE :like ESCAPE '\\'
                            OR a.serial_number LIKE :like ESCAPE '\\'
                            OR a.caliber LIKE :like ESCAPE '\\'
                            OR a.cartridge LIKE :like ESCAPE '\\'
                            OR a.acquisition_source LIKE :like ESCAPE '\\'
                            OR a.notes LIKE :like ESCAPE '\\')))
                 ORDER BY a.make, a.model, k.sort_order, a.id",
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
                    let estimated_value = row.get(11)?;
                    let insurance_policy_id = row.get(12)?;
                    let scheduled_coverage_amount = row.get(13)?;
                    let summary = AccessorySummary {
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
                    };
                    let kind_order: i64 = row.get(14)?;
                    Ok((summary, kind_order))
                },
            )
            .map_err(CommandError::from_db)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::from_db)?;

        // FR-013: the direct host's label, from the whole graph once and one
        // label query per table (research.md §5, §22).
        let graph = MountGraph::load(conn)?;
        let hosts: Vec<RecordRef> = rows
            .iter()
            .filter_map(|(summary, _)| graph.host_of(RecordRef::Accessory(summary.id)))
            .collect();
        let host_labels = mounts::labels(conn, &hosts)?;

        // The group each accessory belongs to (research.md §12): a map from
        // key to group index, not a search of the groups for every row.
        // "Mounted on" is keyed by the host's `RecordRef`, never its name,
        // because two hosts can share one.
        let mut groups: Vec<AccessoryGroup> = Vec::new();
        let mut order: Vec<GroupOrder> = Vec::new();
        let mut group_index: HashMap<GroupKey, usize> = HashMap::new();
        for (mut summary, kind_order) in rows {
            summary.mounted_on = graph
                .host_of(RecordRef::Accessory(summary.id))
                .and_then(|host| host_labels.get(&host).cloned());
            let unspecified = |text: &Option<String>| {
                text.as_deref().map(str::trim).filter(|t| !t.is_empty()).map(str::to_owned)
            };
            let (key, heading, host, rank) = match input.group_by {
                None => (GroupKey::All, "All".to_string(), None, GroupOrder::First),
                Some(AccessoryGroupBy::Kind) => (
                    GroupKey::Text(summary.kind_name.clone()),
                    summary.kind_name.clone(),
                    None,
                    GroupOrder::Kind(kind_order),
                ),
                Some(
                    by @ (AccessoryGroupBy::Make
                    | AccessoryGroupBy::Caliber
                    | AccessoryGroupBy::Cartridge),
                ) => {
                    let value = unspecified(match by {
                        AccessoryGroupBy::Make => &summary.make,
                        AccessoryGroupBy::Caliber => &summary.caliber,
                        _ => &summary.cartridge,
                    });
                    match value {
                        Some(text) => (
                            GroupKey::Text(text.clone()),
                            text.clone(),
                            None,
                            GroupOrder::Text(text.into()),
                        ),
                        None => (
                            GroupKey::Text(UNSPECIFIED.to_string()),
                            UNSPECIFIED.to_string(),
                            None,
                            GroupOrder::Last,
                        ),
                    }
                }
                Some(AccessoryGroupBy::MountedOn) => match &summary.mounted_on {
                    Some(label) => {
                        let name = record_name(label);
                        (
                            GroupKey::Host(label.record),
                            name.clone(),
                            Some(label.clone()),
                            GroupOrder::Host(
                                name.into(),
                                label.record.kind() as u8,
                                label.record.id(),
                            ),
                        )
                    }
                    None => (GroupKey::NotMounted, NOT_MOUNTED.to_string(), None, GroupOrder::Last),
                },
            };
            let index = *group_index.entry(key).or_insert_with(|| {
                groups.push(AccessoryGroup { key: heading, host, accessories: Vec::new() });
                order.push(rank);
                groups.len() - 1
            });
            groups[index].accessories.push(summary);
        }

        // Stable sort of the groups by their rank; the accessories within a
        // group keep the query's make, model, kind order.
        let mut ranked: Vec<(GroupOrder, AccessoryGroup)> = order.into_iter().zip(groups).collect();
        ranked.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(ListAccessoriesOutput { groups: ranked.into_iter().map(|(_, group)| group).collect() })
    }

    /// The identity of a group while the rows are being sorted into groups.
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    enum GroupKey {
        All,
        Text(String),
        Host(RecordRef),
        NotMounted,
    }

    /// Where a group goes in the output (contracts/tauri-commands.md "Group
    /// order"). Variants of one grouping never mix, so the derived order
    /// between variants only has to put `Last` after the rest.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    enum GroupOrder {
        First,
        /// The kind list's position.
        Kind(i64),
        /// Alphabetical: case-insensitively, then as written.
        Text(CaseFolded),
        /// By the host's name, then the host's kind (firearm first) and id.
        Host(CaseFolded, u8, i64),
        Last,
    }

    /// Text that sorts case-insensitively and falls back to the exact text,
    /// so spellings that differ only in case keep a stable order.
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct CaseFolded(String, String);

    impl From<String> for CaseFolded {
        fn from(text: String) -> Self {
            Self(text.to_lowercase(), text)
        }
    }

    impl Ord for CaseFolded {
        fn cmp(&self, other: &Self) -> std::cmp::Ordering {
            self.cmp_key().cmp(&other.cmp_key())
        }
    }

    impl PartialOrd for CaseFolded {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(other))
        }
    }

    impl CaseFolded {
        fn cmp_key(&self) -> (&str, &str) {
            (&self.0, &self.1)
        }
    }

    /// A record's name as the Accessories page shows it (FR-005): a firearm
    /// "{make} {model} “{nickname}”", an accessory "{make} {model} · {kind}"
    /// with the kind alone when make and model are both blank.
    fn record_name(label: &RecordLabel) -> String {
        let make_model = [label.make.as_deref(), label.model.as_deref()]
            .into_iter()
            .flatten()
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        match label.record {
            RecordRef::Accessory(_) if make_model.is_empty() => label.type_name.clone(),
            RecordRef::Accessory(_) => format!("{make_model} · {}", label.type_name),
            RecordRef::Firearm(_) => match label.nickname.as_deref() {
                Some(nickname) if !nickname.is_empty() => {
                    format!("{make_model} \u{201C}{nickname}\u{201D}")
                }
                _ => make_model,
            },
        }
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
