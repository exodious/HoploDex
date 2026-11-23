//! 006 research.md §5: every mount rule, as plain
//! functions over one loaded graph. `MountGraph::load` reads the whole
//! `mounts` table once (a narrow scan of four integers per row), and the
//! walks are iterative, so a deep chain can't overflow the stack and a
//! listing never asks the database a question per row (006 SC-007).
//!
//! No rule here reads a kind, type, caliber or cartridge: no mount is ever
//! judged by what the two records are (006 FR-011). `labels` reads the type or
//! kind name only to show it in a record's label.

use std::collections::{HashMap, HashSet};

use rusqlite::{Connection, OptionalExtension, named_params};

use crate::commands::CommandError;
use crate::models::firearm::FirearmStatus;
use crate::models::record::{MountDetail, MountedEntry, RecordCounts, RecordLabel, RecordRef};

/// 006 FR-010: a host that is missing or disposed, or an item that is disposed.
pub const CHOOSE_ACTIVE: &str = "Choose an active firearm or accessory.";
/// 006 FR-010: a record on itself, or on something that is mounted on it,
/// worded by the kind of the record being mounted.
pub fn self_or_below(item: RecordRef) -> String {
    let what = match item {
        RecordRef::Firearm(_) => "A firearm",
        RecordRef::Accessory(_) => "An accessory",
    };
    format!("{what} can't be mounted on itself, or on something mounted on it.")
}

/// The most levels a record may sit below the top of its mount chain: an
/// optic on a mount on a rail on a handguard on a firearm is 4. It keeps
/// every walk up a chain to this many steps, whatever a file asks for
/// (code scanning alert 83).
pub const MAX_MOUNT_DEPTH: usize = 16;
/// Heights (how many levels are below a record) are counted up to this, which
/// is enough to tell a mount past [`MAX_MOUNT_DEPTH`].
const HEIGHT_CAP: usize = MAX_MOUNT_DEPTH + 1;
/// A mount that would put something deeper than [`MAX_MOUNT_DEPTH`].
pub const TOO_DEEP: &str = "Mounts can't stack more than 16 levels deep.";

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
    /// How many of each host's items have each height, up to
    /// [`HEIGHT_CAP`]. A record's height is one more than its tallest
    /// item's, so a mount changes a few counts up the host's chain and never
    /// walks down (code scanning alert 83).
    items_by_height: HashMap<RecordRef, [u32; HEIGHT_CAP + 1]>,
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
        graph.count_heights();
        Ok(graph)
    }

    /// Works out every record's height once, children first, and counts
    /// each host's items by theirs. A record on a loop (a file edited by
    /// hand) counts as [`HEIGHT_CAP`].
    fn count_heights(&mut self) {
        let mut heights: HashMap<RecordRef, usize> = HashMap::new();
        let mut visiting = HashSet::new();
        for &top in self.items_of.keys() {
            let mut stack = vec![(top, false)];
            while let Some((at, expanded)) = stack.pop() {
                if heights.contains_key(&at) {
                    continue;
                }
                let items = self.items_of.get(&at).map(Vec::as_slice).unwrap_or_default();
                if expanded {
                    let tallest = items.iter().map(|i| heights.get(i).map_or(HEIGHT_CAP, |h| *h));
                    heights.insert(at, tallest.max().map_or(0, |h| (h + 1).min(HEIGHT_CAP)));
                    continue;
                }
                if !visiting.insert(at) {
                    continue;
                }
                stack.push((at, true));
                stack.extend(
                    items
                        .iter()
                        .filter(|item| !visiting.contains(*item))
                        .map(|item| (*item, false)),
                );
            }
        }
        for (item, host) in &self.host_of {
            let height = heights.get(item).copied().unwrap_or(0);
            self.items_by_height.entry(*host).or_default()[height] += 1;
        }
    }

    /// How many levels are below `record`, up to [`HEIGHT_CAP`].
    fn height(&self, record: RecordRef) -> usize {
        self.items_by_height
            .get(&record)
            .and_then(|counts| counts.iter().rposition(|n| *n > 0))
            .map_or(0, |h| (h + 1).min(HEIGHT_CAP))
    }

    /// One of `host`'s items went from height `from` to `to` (`None`: it
    /// left or arrived), and so on up the chain while a height changes. A
    /// change dies out within [`HEIGHT_CAP`] steps, where heights reach the
    /// cap.
    fn recount(&mut self, mut host: RecordRef, mut from: Option<usize>, mut to: Option<usize>) {
        for _ in 0..=HEIGHT_CAP {
            let before = self.height(host);
            let counts = self.items_by_height.entry(host).or_default();
            if let Some(h) = from {
                counts[h] = counts[h].saturating_sub(1);
            }
            if let Some(h) = to {
                counts[h] += 1;
            }
            let after = self.height(host);
            match self.host_of(host) {
                Some(next) if after != before => {
                    (host, from, to) = (next, Some(before), Some(after));
                }
                _ => return,
            }
        }
    }

    /// Records in this graph that `item` is now on `host` (or on nothing),
    /// after the mount was saved, so a run of mounts (an import's) checks
    /// each against the ones before it without loading the table again.
    pub fn place(&mut self, item: RecordRef, host: Option<RecordRef>) {
        let height = self.height(item);
        if let Some(old) = self.host_of.remove(&item) {
            if let Some(items) = self.items_of.get_mut(&old) {
                items.retain(|i| *i != item);
            }
            self.recount(old, Some(height), None);
        }
        if let Some(host) = host {
            self.host_of.insert(item, host);
            self.items_of.entry(host).or_default().push(item);
            self.recount(host, None, Some(height));
        }
    }

    /// What the item is mounted on directly.
    pub fn host_of(&self, item: RecordRef) -> Option<RecordRef> {
        self.host_of.get(&item).copied()
    }

    /// Its host, that host's host, and so on.
    pub fn chain(&self, item: RecordRef) -> Vec<RecordRef> {
        self.chain_upto(item, usize::MAX)
    }

    /// The first `limit` records of [`chain`](Self::chain).
    fn chain_upto(&self, item: RecordRef, limit: usize) -> Vec<RecordRef> {
        let mut chain = Vec::new();
        let mut seen = HashSet::from([item]);
        let mut at = item;
        while chain.len() < limit
            && let Some(host) = self.host_of(at)
        {
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

    /// How many firearms and accessories are below `host` at any depth.
    /// `memo` is shared across calls, so counting every firearm of a
    /// listing visits each mount once.
    pub fn count_below(
        &self,
        host: RecordRef,
        memo: &mut HashMap<RecordRef, RecordCounts>,
    ) -> RecordCounts {
        if let Some(counts) = memo.get(&host) {
            return *counts;
        }
        let mut visiting = HashSet::new();
        let mut stack = vec![(host, false)];
        while let Some((record, expanded)) = stack.pop() {
            if memo.contains_key(&record) {
                continue;
            }
            let items = self.items_of.get(&record).map(Vec::as_slice).unwrap_or_default();
            if expanded {
                let mut counts = RecordCounts::of(items.iter().copied());
                for item in items {
                    counts += memo.get(item).copied().unwrap_or_default();
                }
                memo.insert(record, counts);
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
        memo.get(&host).copied().unwrap_or_default()
    }

    /// 006 FR-010: mounting `item` on `host` would make a record carry itself.
    /// True when `host` is `item` or is below it, which is when `item` is
    /// on `host`'s chain. Only [`MAX_MOUNT_DEPTH`] steps of the chain are
    /// walked: a host deeper than that is refused by [`Self::too_deep`].
    pub fn would_loop(&self, item: RecordRef, host: RecordRef) -> bool {
        host == item || self.chain_upto(host, MAX_MOUNT_DEPTH).contains(&item)
    }

    /// Mounting `item` (and everything on it) on `host` would put a record
    /// more than [`MAX_MOUNT_DEPTH`] levels below the top of its chain.
    pub fn too_deep(&self, item: RecordRef, host: RecordRef) -> bool {
        let above = self.chain_upto(host, MAX_MOUNT_DEPTH).len() + 1;
        above + self.height(item) > MAX_MOUNT_DEPTH
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
/// are active, the mount makes no loop, and it puts nothing deeper than
/// [`MAX_MOUNT_DEPTH`]. Errors are `VALIDATION_ERROR`
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
        return Err(field_error(field, &self_or_below(item)));
    }
    if graph.too_deep(item, host) {
        return Err(field_error(field, TOO_DEEP));
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

/// The mount a record's form saves (006 research.md §8). A disposed record is
/// never mounted and carries nothing (006 FR-014), so saving one clears every
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
/// host it is. The status triggers refuse the save otherwise (006 data-model.md
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

/// The acquisition dates that `records` have, one query per table. A record
/// with none, or that doesn't exist, has no entry.
fn acquisition_dates(
    conn: &Connection,
    records: &[RecordRef],
) -> Result<HashMap<RecordRef, String>, CommandError> {
    let mut found = HashMap::new();
    for firearms in [true, false] {
        let ids: Vec<i64> = records
            .iter()
            .filter(|r| matches!(r, RecordRef::Firearm(_)) == firearms)
            .map(RecordRef::id)
            .collect();
        if ids.is_empty() {
            continue;
        }
        let table = if firearms { "firearms" } else { "accessories" };
        let ids = serde_json::to_string(&ids).expect("integers serialize");
        let mut stmt = conn
            .prepare_cached(&format!(
                "SELECT id, acquisition_date FROM {table}
                 WHERE acquisition_date IS NOT NULL
                   AND id IN (SELECT value FROM json_each(:ids))"
            ))
            .map_err(CommandError::from_db)?;
        let rows = stmt
            .query_map(named_params! { ":ids": ids }, |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(CommandError::from_db)?;
        for row in rows {
            let (id, date) = row.map_err(CommandError::from_db)?;
            let record = if firearms { RecordRef::Firearm(id) } else { RecordRef::Accessory(id) };
            found.insert(record, date);
        }
    }
    Ok(found)
}

/// A record's name as a message gives it (006 FR-005): a firearm "{make} {model}
/// “{nickname}”", an accessory "{make} {model} · {kind}".
pub fn record_name(label: &RecordLabel) -> String {
    let make_model = format!("{} {}", label.make, label.model);
    match label.record {
        RecordRef::Firearm(_) => match label.nickname.as_deref().filter(|n| !n.is_empty()) {
            Some(nickname) => format!("{make_model} “{nickname}”"),
            None => make_model,
        },
        RecordRef::Accessory(_) => format!("{make_model} · {}", label.type_name),
    }
}

/// One record's label.
pub fn label(conn: &Connection, record: RecordRef) -> Result<Option<RecordLabel>, CommandError> {
    Ok(labels(conn, &[record])?.remove(&record))
}

/// What a record page needs about mounts (006 FR-013): the chain above the
/// record, direct host first, and everything below it, depth-first. Both
/// are empty for a record that is in no mount, which is every disposed one.
pub fn detail(conn: &Connection, record: RecordRef) -> Result<MountDetail, CommandError> {
    let graph = MountGraph::load(conn)?;
    let chain = graph.chain(record);
    let mounted = mounted_entries(conn, &graph, record)?;
    if chain.is_empty() {
        return Ok(MountDetail { chain: Vec::new(), mounted });
    }
    let labels = labels(conn, &chain)?;
    Ok(MountDetail {
        chain: chain.iter().filter_map(|r| labels.get(r).cloned()).collect(),
        mounted,
    })
}

/// Everything below `record` in `graph`, depth-first, as the Mounted list
/// and the dispose choices show it: empty when nothing is mounted on it.
pub fn mounted_entries(
    conn: &Connection,
    graph: &MountGraph,
    record: RecordRef,
) -> Result<Vec<MountedEntry>, CommandError> {
    let below = graph.below(record);
    if below.is_empty() {
        return Ok(Vec::new());
    }
    let records: Vec<RecordRef> = below.iter().map(|(r, _, _)| *r).collect();
    let labels = labels(conn, &records)?;
    let acquired = acquisition_dates(conn, &records)?;
    Ok(below
        .iter()
        .filter_map(|(r, host, depth)| {
            labels.get(r).map(|label| MountedEntry {
                label: label.clone(),
                host: *host,
                depth: *depth,
                acquisition_date: acquired.get(r).cloned(),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn a_wide_record_moved_again_and_again_is_measured_once() {
        // A conflict resolution can mount one record many times (code
        // scanning alert 83's review): with 50,000 records on it, walking
        // them on every move was 2.5 × 10^9 steps.
        let (wide, left, right) =
            (RecordRef::Firearm(1), RecordRef::Accessory(1), RecordRef::Accessory(2));
        let mut graph = MountGraph::default();
        for id in 10..50_010 {
            graph.place(RecordRef::Accessory(id), Some(wide));
        }
        // And a firearm on one of them, moved off and back between each:
        // a change below the wide record, which a cache of heights would forget.
        let (below, firearm) = (RecordRef::Accessory(10), RecordRef::Firearm(2));
        graph.place(firearm, Some(below));
        let started = Instant::now();
        for n in 0..50_000 {
            let host = if n % 2 == 0 { left } else { right };
            assert!(!graph.too_deep(wide, host));
            graph.place(wide, Some(host));
            graph.place(firearm, None);
            graph.place(firearm, Some(below));
        }
        assert!(started.elapsed() < Duration::from_secs(10), "{:?}", started.elapsed());
        assert_eq!(graph.height(wide), 2);
    }

    #[test]
    fn heights_follow_every_move() {
        // A chain 0 <- 1 <- ... <- 15: record 15 is 15 levels down.
        let record = RecordRef::Accessory;
        let mut graph = MountGraph::default();
        for id in 1..16 {
            graph.place(record(id), Some(record(id - 1)));
        }
        assert_eq!(graph.height(record(0)), 15);
        assert!(!graph.too_deep(record(100), record(15)), "16 levels");
        // Something with one level below it can't go at 15.
        graph.place(record(101), Some(record(100)));
        assert!(graph.too_deep(record(100), record(15)));
        assert!(!graph.too_deep(record(100), record(14)));
        // Shortening the chain makes room again, and lengthening it takes it.
        graph.place(record(15), None);
        assert_eq!(graph.height(record(0)), 14);
        assert!(!graph.too_deep(record(100), record(14)));
        graph.place(record(100), Some(record(14)));
        assert_eq!(graph.height(record(0)), 16);
        assert!(graph.too_deep(record(15), record(101)));
        graph.place(record(100), None);
        assert_eq!(graph.height(record(0)), 14);
        assert_eq!(graph.height(record(100)), 1);

        // Worked out from scratch, as a load does, the heights are the same.
        let mut loaded = graph.clone();
        loaded.items_by_height.clear();
        loaded.count_heights();
        for id in 0..=101 {
            assert_eq!(loaded.height(record(id)), graph.height(record(id)), "{id}");
        }
    }

    #[test]
    fn a_loop_from_a_file_edited_by_hand_is_as_tall_as_heights_go() {
        let record = RecordRef::Accessory;
        let mut graph = MountGraph::default();
        graph.place(record(1), Some(record(2)));
        graph.place(record(2), Some(record(1)));
        graph.place(record(3), Some(record(1)));
        graph.items_by_height.clear();
        graph.count_heights();
        assert_eq!(graph.height(record(1)), HEIGHT_CAP);
        // A change on the loop dies out, and the heights stay at the cap.
        graph.place(record(4), Some(record(2)));
        graph.place(record(3), None);
        assert_eq!(graph.height(record(1)), HEIGHT_CAP);
        assert_eq!(graph.height(record(2)), HEIGHT_CAP);
    }
}
