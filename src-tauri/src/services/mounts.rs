//! specs/006-accessory-links research.md §5: every mount rule, as plain
//! functions over one loaded graph. `MountGraph::load` reads the whole
//! `mounts` table once (a narrow scan of four integers per row), and the
//! walks are iterative, so a deep chain can't overflow the stack and a
//! listing never asks the database a question per row (SC-007).
//!
//! No rule here reads a kind, type, caliber or cartridge: no mount is ever
//! judged by what the two records are (FR-011). `labels` reads the type or
//! kind name only to show it in a record's label.

use std::collections::{HashMap, HashSet};

use rusqlite::{Connection, OptionalExtension, named_params};

use crate::commands::CommandError;
use crate::models::firearm::FirearmStatus;
use crate::models::record::{MountDetail, MountedEntry, RecordLabel, RecordRef};

/// FR-010: a host that is missing or disposed, or an item that is disposed.
pub const CHOOSE_ACTIVE: &str = "Choose an active firearm or accessory.";
/// FR-010: a record on itself, or on something that is mounted on it.
pub const SELF_OR_BELOW: &str =
    "A firearm can't be mounted on itself, or on something mounted on it.";

fn field_error(field: &str, message: &str) -> CommandError {
    CommandError::validation(message, Default::default()).on_field(field, message)
}

/// `(firearm_id, accessory_id)` column values, exactly one of them set.
fn record_from_pair(firearm: Option<i64>, accessory: Option<i64>) -> Option<RecordRef> {
    match (firearm, accessory) {
        (Some(id), None) => Some(RecordRef::Firearm(id)),
        (None, Some(id)) => Some(RecordRef::Accessory(id)),
        _ => None,
    }
}

/// The mounts of a database: item to host, and host to items.
#[derive(Debug, Default, Clone)]
pub struct MountGraph {
    host_of: HashMap<RecordRef, RecordRef>,
    /// Each host's items in the order their rows were recorded (a move
    /// updates the row, so the item keeps its place).
    items_of: HashMap<RecordRef, Vec<RecordRef>>,
}

impl MountGraph {
    /// One scan of `mounts`.
    pub fn load(conn: &Connection) -> Result<Self, CommandError> {
        let mut stmt = conn
            .prepare(
                "SELECT item_firearm_id, item_accessory_id, host_firearm_id, host_accessory_id
                 FROM mounts ORDER BY id",
            )
            .map_err(CommandError::from_db)?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    record_from_pair(row.get(0)?, row.get(1)?),
                    record_from_pair(row.get(2)?, row.get(3)?),
                ))
            })
            .map_err(CommandError::from_db)?;
        let mut graph = Self::default();
        for row in rows {
            // The table's CHECKs make every row a full pair.
            if let (Some(item), Some(host)) = row.map_err(CommandError::from_db)? {
                graph.host_of.insert(item, host);
                graph.items_of.entry(host).or_default().push(item);
            }
        }
        Ok(graph)
    }

    /// Records in this graph that `item` is now on `host` (or on nothing),
    /// after the mount was saved, so a run of mounts (an import's) checks
    /// each against the ones before it without loading the table again.
    pub fn place(&mut self, item: RecordRef, host: Option<RecordRef>) {
        if let Some(old) = self.host_of.remove(&item)
            && let Some(items) = self.items_of.get_mut(&old)
        {
            items.retain(|i| *i != item);
        }
        if let Some(host) = host {
            self.host_of.insert(item, host);
            self.items_of.entry(host).or_default().push(item);
        }
    }

    /// What the item is mounted on directly.
    pub fn host_of(&self, item: RecordRef) -> Option<RecordRef> {
        self.host_of.get(&item).copied()
    }

    /// Its host, that host's host, and so on.
    pub fn chain(&self, item: RecordRef) -> Vec<RecordRef> {
        let mut chain = Vec::new();
        let mut seen = HashSet::from([item]);
        let mut at = item;
        while let Some(host) = self.host_of(at) {
            // A loop can't be made through the commands; this keeps a file
            // edited by hand from hanging the walk.
            if !seen.insert(host) {
                break;
            }
            chain.push(host);
            at = host;
        }
        chain
    }

    /// Everything below `host`, depth-first: each record, what it is
    /// mounted on, and its depth (1 = directly on `host`).
    pub fn below(&self, host: RecordRef) -> Vec<(RecordRef, RecordRef, u32)> {
        let mut entries = Vec::new();
        let mut seen = HashSet::from([host]);
        let mut stack: Vec<(RecordRef, RecordRef, u32)> = self
            .items_of
            .get(&host)
            .into_iter()
            .flatten()
            .rev()
            .map(|item| (*item, host, 1))
            .collect();
        while let Some((record, on, depth)) = stack.pop() {
            if !seen.insert(record) {
                continue;
            }
            entries.push((record, on, depth));
            stack.extend(
                self.items_of
                    .get(&record)
                    .into_iter()
                    .flatten()
                    .rev()
                    .map(|item| (*item, record, depth + 1)),
            );
        }
        entries
    }

    /// How many records are below `host` at any depth. `memo` is shared
    /// across calls, so counting every firearm of a listing visits each
    /// mount once.
    pub fn count_below(&self, host: RecordRef, memo: &mut HashMap<RecordRef, usize>) -> usize {
        if let Some(count) = memo.get(&host) {
            return *count;
        }
        let mut visiting = HashSet::new();
        let mut stack = vec![(host, false)];
        while let Some((record, expanded)) = stack.pop() {
            if memo.contains_key(&record) {
                continue;
            }
            let items = self.items_of.get(&record).map(Vec::as_slice).unwrap_or_default();
            if expanded {
                let count: usize =
                    items.iter().map(|item| 1 + memo.get(item).copied().unwrap_or(0)).sum();
                memo.insert(record, count);
                continue;
            }
            if !visiting.insert(record) {
                continue;
            }
            stack.push((record, true));
            stack.extend(
                items.iter().filter(|item| !visiting.contains(*item)).map(|item| (*item, false)),
            );
        }
        memo.get(&host).copied().unwrap_or(0)
    }

    /// FR-010: mounting `item` on `host` would make a record carry itself.
    /// True when `host` is `item` or is below it, which is when `item` is
    /// on `host`'s chain.
    pub fn would_loop(&self, item: RecordRef, host: RecordRef) -> bool {
        host == item || self.chain(host).contains(&item)
    }
}

/// `Some(status)` of a record, or `None` when it doesn't exist.
fn status_of(conn: &Connection, record: RecordRef) -> Result<Option<FirearmStatus>, CommandError> {
    conn.query_row(
        &format!("SELECT status FROM {} WHERE id = :id", record.table()),
        named_params! { ":id": record.id() },
        |row| row.get(0),
    )
    .optional()
    .map_err(CommandError::from_db)
}

/// Whether the record exists.
pub fn exists(conn: &Connection, record: RecordRef) -> Result<bool, CommandError> {
    Ok(status_of(conn, record)?.is_some())
}

fn is_active(conn: &Connection, record: RecordRef) -> Result<bool, CommandError> {
    Ok(status_of(conn, record)? == Some(FirearmStatus::Active))
}

/// Puts `item` on `host`, moves it, or takes it off (`None`), by deleting,
/// updating or inserting the item's one row. Checks, in order: both records
/// are active, and the mount makes no loop. Errors are `VALIDATION_ERROR`
/// on `field` (`host` for `mount_record`, `mountedOn` for a form). Mounting
/// an item on what it is already on changes nothing and checks nothing.
pub fn set_mount(
    conn: &Connection,
    graph: &MountGraph,
    item: RecordRef,
    host: Option<RecordRef>,
    field: &str,
) -> Result<(), CommandError> {
    let item_columns = match item {
        RecordRef::Firearm(_) => "item_firearm_id",
        RecordRef::Accessory(_) => "item_accessory_id",
    };
    let Some(host) = host else {
        conn.execute(
            &format!("DELETE FROM mounts WHERE {item_columns} = :id"),
            named_params! { ":id": item.id() },
        )
        .map_err(CommandError::from_db)?;
        return Ok(());
    };
    if graph.host_of(item) == Some(host) {
        return Ok(());
    }
    if !is_active(conn, item)? || !is_active(conn, host)? {
        return Err(field_error(field, CHOOSE_ACTIVE));
    }
    if graph.would_loop(item, host) {
        return Err(field_error(field, SELF_OR_BELOW));
    }
    let (host_firearm, host_accessory) = host.owner_columns();
    let updated = conn
        .execute(
            &format!(
                "UPDATE mounts SET host_firearm_id = :hf, host_accessory_id = :ha
                 WHERE {item_columns} = :id"
            ),
            named_params! { ":hf": host_firearm, ":ha": host_accessory, ":id": item.id() },
        )
        .map_err(CommandError::from_db)?;
    if updated == 0 {
        let (item_firearm, item_accessory) = item.owner_columns();
        conn.execute(
            "INSERT INTO mounts (
                item_firearm_id, item_accessory_id, host_firearm_id, host_accessory_id
            ) VALUES (:if, :ia, :hf, :ha)",
            named_params! {
                ":if": item_firearm,
                ":ia": item_accessory,
                ":hf": host_firearm,
                ":ha": host_accessory,
            },
        )
        .map_err(CommandError::from_db)?;
    }
    Ok(())
}

/// The mount a record's form saves (research.md §8). A disposed record is
/// never mounted and carries nothing (FR-014), so saving one clears every
/// mount it is in; otherwise `mounted_on` is set as `set_mount` does, with
/// the graph loaded only when there is a host to check.
pub fn save_form_mount(
    conn: &Connection,
    record: RecordRef,
    status: FirearmStatus,
    mounted_on: Option<RecordRef>,
) -> Result<(), CommandError> {
    if status == FirearmStatus::Disposed {
        return clear_mounts(conn, record);
    }
    let graph = match mounted_on {
        Some(_) => MountGraph::load(conn)?,
        None => MountGraph::default(),
    };
    set_mount(conn, &graph, record, mounted_on, "mountedOn")
}

/// Before a record is saved as disposed: deletes every mount whose item or
/// host it is. The status triggers refuse the save otherwise (data-model.md
/// "Rules").
pub fn clear_mounts(conn: &Connection, record: RecordRef) -> Result<(), CommandError> {
    let (item_column, host_column) = match record {
        RecordRef::Firearm(_) => ("item_firearm_id", "host_firearm_id"),
        RecordRef::Accessory(_) => ("item_accessory_id", "host_accessory_id"),
    };
    conn.execute(
        &format!("DELETE FROM mounts WHERE {item_column} = :id OR {host_column} = :id"),
        named_params! { ":id": record.id() },
    )
    .map_err(CommandError::from_db)?;
    Ok(())
}

/// The record's direct host, read for a single record's output.
pub fn host_of_record(
    conn: &Connection,
    record: RecordRef,
) -> Result<Option<RecordRef>, CommandError> {
    let item_column = match record {
        RecordRef::Firearm(_) => "item_firearm_id",
        RecordRef::Accessory(_) => "item_accessory_id",
    };
    conn.query_row(
        &format!("SELECT host_firearm_id, host_accessory_id FROM mounts WHERE {item_column} = :id"),
        named_params! { ":id": record.id() },
        |row| Ok(record_from_pair(row.get(0)?, row.get(1)?)),
    )
    .optional()
    .map(Option::flatten)
    .map_err(CommandError::from_db)
}

/// Runs `body` so that all of its writes stand or none do. A savepoint, not
/// a transaction, because a caller (reversing a disposition) may already
/// have one open.
pub fn atomically<T>(
    conn: &Connection,
    body: impl FnOnce() -> Result<T, CommandError>,
) -> Result<T, CommandError> {
    conn.execute_batch("SAVEPOINT hd_mount_save").map_err(CommandError::from_db)?;
    match body() {
        Ok(value) => {
            conn.execute_batch("RELEASE hd_mount_save").map_err(CommandError::from_db)?;
            Ok(value)
        }
        Err(err) => {
            // Roll back, then drop the savepoint; the original error is
            // the one worth reporting.
            let _ = conn.execute_batch("ROLLBACK TO hd_mount_save; RELEASE hd_mount_save");
            Err(err)
        }
    }
}

/// The labels of `records`, one query per table
/// (`WHERE id IN (SELECT value FROM json_each(?))`). A record that doesn't
/// exist has no entry.
pub fn labels(
    conn: &Connection,
    records: &[RecordRef],
) -> Result<HashMap<RecordRef, RecordLabel>, CommandError> {
    let mut found = HashMap::new();
    for (firearms, sql) in [
        (
            true,
            "SELECT f.id, f.make, f.model, f.nickname, t.name, f.serial_number, f.status
             FROM firearms f JOIN firearm_types t ON t.id = f.firearm_type_id
             WHERE f.id IN (SELECT value FROM json_each(:ids))",
        ),
        (
            false,
            "SELECT a.id, a.make, a.model, NULL, k.name, a.serial_number, a.status
             FROM accessories a JOIN accessory_kinds k ON k.id = a.accessory_kind_id
             WHERE a.id IN (SELECT value FROM json_each(:ids))",
        ),
    ] {
        let mut ids: Vec<i64> = records
            .iter()
            .filter(|r| matches!(r, RecordRef::Firearm(_)) == firearms)
            .map(RecordRef::id)
            .collect();
        if ids.is_empty() {
            continue;
        }
        ids.sort_unstable();
        ids.dedup();
        let ids = serde_json::to_string(&ids).expect("integers serialize");
        let mut stmt = conn.prepare_cached(sql).map_err(CommandError::from_db)?;
        let rows = stmt
            .query_map(named_params! { ":ids": ids }, |row| {
                let id = row.get(0)?;
                Ok(RecordLabel {
                    record: if firearms {
                        RecordRef::Firearm(id)
                    } else {
                        RecordRef::Accessory(id)
                    },
                    make: row.get(1)?,
                    model: row.get(2)?,
                    nickname: row.get(3)?,
                    type_name: row.get(4)?,
                    serial_number: row.get(5)?,
                    status: row.get(6)?,
                })
            })
            .map_err(CommandError::from_db)?;
        for label in rows {
            let label = label.map_err(CommandError::from_db)?;
            found.insert(label.record, label);
        }
    }
    Ok(found)
}

/// One record's label.
pub fn label(conn: &Connection, record: RecordRef) -> Result<Option<RecordLabel>, CommandError> {
    Ok(labels(conn, &[record])?.remove(&record))
}

/// What a record page needs about mounts (FR-013): the chain above the
/// record, direct host first, and everything below it, depth-first. Both
/// are empty for a record that is in no mount, which is every disposed one.
pub fn detail(conn: &Connection, record: RecordRef) -> Result<MountDetail, CommandError> {
    let graph = MountGraph::load(conn)?;
    let chain = graph.chain(record);
    let below = graph.below(record);
    if chain.is_empty() && below.is_empty() {
        return Ok(MountDetail::default());
    }
    let wanted: Vec<RecordRef> =
        chain.iter().copied().chain(below.iter().map(|(r, _, _)| *r)).collect();
    let labels = labels(conn, &wanted)?;
    Ok(MountDetail {
        chain: chain.iter().filter_map(|r| labels.get(r).cloned()).collect(),
        mounted: below
            .iter()
            .filter_map(|(r, host, depth)| {
                labels.get(r).map(|label| MountedEntry {
                    label: label.clone(),
                    host: *host,
                    depth: *depth,
                })
            })
            .collect(),
    })
}
