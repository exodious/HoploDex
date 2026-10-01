//! specs/006-accessory-links contracts/tauri-commands.md "Mounts (new)": the
//! mount commands. Each `#[tauri::command]` is thin and calls the matching
//! function in `ops`, which takes a `&Connection`.

use std::collections::HashSet;

use rusqlite::{Connection, named_params};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::CommandError;
use crate::commands::accessories::ops as accessory_ops;
use crate::commands::firearms::ops as firearm_ops;
use crate::commands::firearms::{DisposeInput, DisposeWith};
use crate::models::record::{RecordLabel, RecordRef};
use crate::services::mounts::{self, MountGraph};
use crate::session::Session;

/// A dispose dialog's list no longer matches what is mounted (research.md
/// §9).
const STALE_MOUNTED: &str = "What is mounted has changed. Close the dialog and try again.";

/// Input for `mount_record`: `host: null` takes the item off whatever it
/// is on.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MountRecordInput {
    pub item: RecordRef,
    #[serde(default)]
    pub host: Option<RecordRef>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MountRecordOutput {
    pub item: RecordLabel,
    pub host: Option<RecordLabel>,
}

/// Which side of a mount the candidates are for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MountRole {
    /// Hosts for an item (or for a new record).
    Host,
    /// Items for a host.
    Item,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListMountCandidatesInput {
    pub role: MountRole,
    /// Role `host`: the item being placed (`null` for a new record). Role
    /// `item`: the host receiving it (required).
    #[serde(default)]
    pub record: Option<RecordRef>,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MountCandidate {
    pub label: RecordLabel,
    /// Where the candidate is mounted now (its direct host), so the
    /// frontend can ask before moving it (FR-012).
    pub mounted_on: Option<RecordLabel>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ListMountCandidatesOutput {
    pub candidates: Vec<MountCandidate>,
}

/// Pure, `Connection`-based business logic, mirroring
/// `commands::firearms::ops`.
pub mod ops {
    use super::*;

    const DEFAULT_LIMIT: u32 = 50;
    const MAX_LIMIT: u32 = 100;

    /// FR-012: mounts, moves or unmounts `item` in one step. Everything
    /// mounted on it stays mounted on it (FR-010). Nothing about the two
    /// records' kinds, types or calibers is checked (FR-011).
    pub fn mount_record(
        conn: &Connection,
        input: &MountRecordInput,
    ) -> Result<MountRecordOutput, CommandError> {
        let item = input.item;
        if !mounts::exists(conn, item)? {
            return Err(CommandError::not_found("No such firearm or accessory was found."));
        }
        let graph = MountGraph::load(conn)?;
        mounts::atomically(conn, || mounts::set_mount(conn, &graph, item, input.host, "host"))?;
        let wanted: Vec<RecordRef> = std::iter::once(item).chain(input.host).collect();
        let labels = mounts::labels(conn, &wanted)?;
        let item_label = labels.get(&item).cloned().ok_or_else(internal)?;
        let host_label = match input.host {
            Some(host) => Some(labels.get(&host).cloned().ok_or_else(internal)?),
            None => None,
        };
        Ok(MountRecordOutput { item: item_label, host: host_label })
    }

    /// FR-014, research.md §9: disposes of `host` and the listed records
    /// below it, with the host's type, recipient and date, in one step. The
    /// listed records must all be below the host now (else a stale dialog
    /// could dispose of the wrong record). Every mount that involves the
    /// host or a listed record goes first, so a kept record mounted on a
    /// disposed one is unmounted and a kept record on a kept one stays.
    pub fn dispose_with_mounted(
        conn: &Connection,
        host: RecordRef,
        input: &DisposeInput,
    ) -> Result<(), CommandError> {
        mounts::atomically(conn, || {
            let listed = checked_listing(conn, host, &input.with_mounted)?;
            mounts::clear_mounts(conn, host)?;
            for with in &listed {
                mounts::clear_mounts(conn, with.record)?;
            }
            // The host first, so what is wrong with its own input is
            // reported as its own and not blamed on a record below it.
            save_disposed(conn, host, input, Some(input.price))?;
            for with in listed {
                save_disposed(conn, with.record, input, with.price)
                    .map_err(|err| name_the_failed_record(conn, with.record, err))?;
            }
            Ok(())
        })
    }

    /// FR-014, US3/AC2: a record disposed of with the host that fails its own
    /// checks (a disposition date before its own acquisition date) names
    /// itself and the reason on `withMounted`; the caller's transaction rolls
    /// everything back.
    fn name_the_failed_record(
        conn: &Connection,
        record: RecordRef,
        err: CommandError,
    ) -> CommandError {
        if err.code != "VALIDATION_ERROR" {
            return err;
        }
        let mut reasons: Vec<(String, String)> =
            err.field_errors.iter().flatten().map(|(f, r)| (f.clone(), r.clone())).collect();
        reasons.sort();
        let reason = match reasons.as_slice() {
            [] => err.message.clone(),
            _ => reasons.into_iter().map(|(_, reason)| reason).collect::<Vec<_>>().join(" "),
        };
        let name = match mounts::label(conn, record) {
            Ok(Some(label)) => mounts::record_name(&label),
            _ => "A mounted record".to_owned(),
        };
        let message = format!("{name}: {reason}");
        CommandError::validation(message.clone(), Default::default())
            .on_field("withMounted", message)
    }

    /// The listed records, each once, after checking they are below `host`.
    fn checked_listing(
        conn: &Connection,
        host: RecordRef,
        listed: &[DisposeWith],
    ) -> Result<Vec<DisposeWith>, CommandError> {
        if listed.is_empty() {
            return Ok(Vec::new());
        }
        let below: HashSet<RecordRef> =
            MountGraph::load(conn)?.below(host).into_iter().map(|(record, _, _)| record).collect();
        let mut seen = HashSet::new();
        let mut checked = Vec::new();
        for with in listed {
            if !below.contains(&with.record) {
                return Err(CommandError::validation(STALE_MOUNTED, Default::default())
                    .on_field("withMounted", STALE_MOUNTED));
            }
            if seen.insert(with.record) {
                checked.push(*with);
            }
        }
        Ok(checked)
    }

    fn save_disposed(
        conn: &Connection,
        record: RecordRef,
        input: &DisposeInput,
        price: Option<i64>,
    ) -> Result<(), CommandError> {
        match record {
            RecordRef::Firearm(id) => firearm_ops::save_disposed(conn, id, input, price).map(drop),
            RecordRef::Accessory(id) => {
                accessory_ops::save_disposed(conn, id, input, price).map(drop)
            }
        }
    }

    fn internal() -> CommandError {
        CommandError::new("INTERNAL_ERROR", "An unexpected error occurred.")
    }

    /// The choices for either direction of a mount (research.md §8), each
    /// with where it is mounted now. A query matches make, model, the two
    /// together ("Leupold VX"), nickname, serial number and, for an
    /// accessory, its kind ("sling") (FR-012).
    pub fn list_mount_candidates(
        conn: &Connection,
        input: &ListMountCandidatesInput,
    ) -> Result<ListMountCandidatesOutput, CommandError> {
        let graph = MountGraph::load(conn)?;
        // What may not be chosen, by the rule of the role (FR-010).
        let mut excluded: Vec<RecordRef> = Vec::new();
        match (input.role, input.record) {
            (MountRole::Host, Some(item)) => {
                excluded.push(item);
                excluded.extend(graph.below(item).into_iter().map(|(record, _, _)| record));
            }
            (MountRole::Host, None) => {}
            (MountRole::Item, Some(host)) => {
                excluded.push(host);
                excluded.extend(graph.chain(host));
                excluded.extend(
                    graph
                        .below(host)
                        .into_iter()
                        .filter(|(_, on, _)| *on == host)
                        .map(|(record, _, _)| record),
                );
            }
            (MountRole::Item, None) => {
                let message = "Choose the firearm or accessory to mount on.";
                return Err(CommandError::validation(message, Default::default())
                    .on_field("record", message));
            }
        }
        let ids = |firearms: bool| {
            let ids: Vec<i64> = excluded
                .iter()
                .filter(|r| matches!(r, RecordRef::Firearm(_)) == firearms)
                .map(RecordRef::id)
                .collect();
            serde_json::to_string(&ids).expect("integers serialize")
        };

        let trimmed = input.query.trim();
        let like =
            format!("%{}%", trimmed.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
        let limit = input.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);

        // Both tables in one ordered query: by name (make and model, or the
        // kind when both are blank), then id. Only the match and the order
        // are decided here; the labels come from `mounts::labels`.
        let mut stmt = conn
            .prepare(
                "SELECT 0 AS firearm, f.id,
                        lower(trim(f.make || ' ' || f.model)) AS name
                 FROM firearms f
                 WHERE f.status = 'active'
                   AND f.id NOT IN (SELECT value FROM json_each(:firearms))
                   AND (:has_query = 0
                        OR f.make LIKE :like ESCAPE '\\' OR f.model LIKE :like ESCAPE '\\'
                        OR (f.make || ' ' || f.model) LIKE :like ESCAPE '\\'
                        OR f.nickname LIKE :like ESCAPE '\\'
                        OR f.serial_number LIKE :like ESCAPE '\\')
                 UNION ALL
                 SELECT 1, a.id,
                        COALESCE(NULLIF(lower(trim(COALESCE(a.make, '') || ' ' || COALESCE(a.model, ''))), ''),
                                 lower(k.name))
                 FROM accessories a JOIN accessory_kinds k ON k.id = a.accessory_kind_id
                 WHERE a.status = 'active'
                   AND a.id NOT IN (SELECT value FROM json_each(:accessories))
                   AND (:has_query = 0
                        OR a.make LIKE :like ESCAPE '\\' OR a.model LIKE :like ESCAPE '\\'
                        OR (COALESCE(a.make, '') || ' ' || COALESCE(a.model, ''))
                            LIKE :like ESCAPE '\\'
                        OR k.name LIKE :like ESCAPE '\\'
                        OR a.serial_number LIKE :like ESCAPE '\\')
                 ORDER BY 3, 2, 1
                 LIMIT :limit",
            )
            .map_err(CommandError::from_db)?;
        let found = stmt
            .query_map(
                named_params! {
                    ":firearms": ids(true),
                    ":accessories": ids(false),
                    ":has_query": !trimmed.is_empty(),
                    ":like": like,
                    ":limit": limit,
                },
                |row| {
                    let id = row.get(1)?;
                    Ok(match row.get::<_, i64>(0)? {
                        0 => RecordRef::Firearm(id),
                        _ => RecordRef::Accessory(id),
                    })
                },
            )
            .map_err(CommandError::from_db)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::from_db)?;

        let hosts: Vec<Option<RecordRef>> = found.iter().map(|r| graph.host_of(*r)).collect();
        let wanted: Vec<RecordRef> =
            found.iter().copied().chain(hosts.iter().flatten().copied()).collect();
        let labels = mounts::labels(conn, &wanted)?;
        let candidates = found
            .iter()
            .zip(hosts)
            .filter_map(|(record, host)| {
                Some(MountCandidate {
                    label: labels.get(record)?.clone(),
                    mounted_on: host.and_then(|host| labels.get(&host).cloned()),
                })
            })
            .collect();
        Ok(ListMountCandidatesOutput { candidates })
    }
}

#[tauri::command]
pub async fn mount_record(
    input: MountRecordInput,
    session: State<'_, Session>,
) -> Result<MountRecordOutput, CommandError> {
    session.write(|conn| ops::mount_record(conn, &input))
}

#[tauri::command]
pub async fn list_mount_candidates(
    input: ListMountCandidatesInput,
    session: State<'_, Session>,
) -> Result<ListMountCandidatesOutput, CommandError> {
    session.read(|conn| ops::list_mount_candidates(conn, &input))
}
