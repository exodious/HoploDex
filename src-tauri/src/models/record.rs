//! specs/006-accessory-links research.md §7: the shared record shapes
//! (`RecordKind`, `RecordRef`, `RecordLabel` and the mount shapes), as
//! contracts/tauri-commands.md "Shared shapes" gives them.

use serde::de::Deserializer;
use serde::ser::{SerializeStruct, Serializer};
use serde::{Deserialize, Serialize};

/// Which table a record lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecordKind {
    Firearm,
    Accessory,
}

/// A record of either kind, named over IPC as `{ kind, id }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecordRef {
    Firearm(i64),
    Accessory(i64),
}

impl RecordRef {
    pub fn new(kind: RecordKind, id: i64) -> Self {
        match kind {
            RecordKind::Firearm => Self::Firearm(id),
            RecordKind::Accessory => Self::Accessory(id),
        }
    }

    pub fn kind(&self) -> RecordKind {
        match self {
            Self::Firearm(_) => RecordKind::Firearm,
            Self::Accessory(_) => RecordKind::Accessory,
        }
    }

    pub fn id(&self) -> i64 {
        match self {
            Self::Firearm(id) | Self::Accessory(id) => *id,
        }
    }

    /// The `(firearm_id, accessory_id)` column pair that holds this
    /// record: exactly one is set.
    pub fn owner_columns(&self) -> (Option<i64>, Option<i64>) {
        match self {
            Self::Firearm(id) => (Some(*id), None),
            Self::Accessory(id) => (None, Some(*id)),
        }
    }

    /// Reads the owner of a `photos`, `document_attachments` or
    /// `disposition_history` row from its `firearm_id` / `accessory_id`
    /// pair; the tables' `CHECK` allows exactly one to be set.
    pub fn from_owner_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        let firearm_id: Option<i64> = row.get("firearm_id")?;
        let accessory_id: Option<i64> = row.get("accessory_id")?;
        match (firearm_id, accessory_id) {
            (Some(id), None) => Ok(Self::Firearm(id)),
            (None, Some(id)) => Ok(Self::Accessory(id)),
            _ => Err(rusqlite::Error::InvalidQuery),
        }
    }

    /// What the user calls it: "firearm" or "accessory" (issue #56).
    pub fn noun(&self) -> &'static str {
        match self {
            Self::Firearm(_) => "firearm",
            Self::Accessory(_) => "accessory",
        }
    }

    /// The table the record lives in.
    pub fn table(&self) -> &'static str {
        match self {
            Self::Firearm(_) => "firearms",
            Self::Accessory(_) => "accessories",
        }
    }

    /// The owner column on `photos`, `document_attachments` and
    /// `disposition_history` that holds this kind of record.
    pub fn owner_column(&self) -> &'static str {
        match self {
            Self::Firearm(_) => "firearm_id",
            Self::Accessory(_) => "accessory_id",
        }
    }
}

/// The IPC form of [`RecordRef`].
#[derive(Deserialize)]
struct RecordRefWire {
    kind: RecordKind,
    id: i64,
}

impl Serialize for RecordRef {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("RecordRef", 2)?;
        state.serialize_field("kind", &self.kind())?;
        state.serialize_field("id", &self.id())?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for RecordRef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = RecordRefWire::deserialize(deserializer)?;
        Ok(Self::new(wire.kind, wire.id))
    }
}

/// How many firearms and accessories a set holds. Issue #56: the
/// application doesn't call either a "record" on its own, so a count says
/// what it counts ("1 firearm and 2 accessories").
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RecordCounts {
    pub firearms: usize,
    pub accessories: usize,
}

impl RecordCounts {
    pub fn of(records: impl IntoIterator<Item = RecordRef>) -> Self {
        let mut counts = Self::default();
        for record in records {
            counts.add(record);
        }
        counts
    }

    pub fn add(&mut self, record: RecordRef) {
        match record {
            RecordRef::Firearm(_) => self.firearms += 1,
            RecordRef::Accessory(_) => self.accessories += 1,
        }
    }

    pub fn total(&self) -> usize {
        self.firearms + self.accessories
    }

    /// "1 firearm", "2 accessories", "1 firearm and 2 accessories"; "no
    /// firearms or accessories" when both are 0. The frontend's
    /// `describeCounts` says the same.
    pub fn describe(&self) -> String {
        let plural =
            |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        match (self.firearms, self.accessories) {
            (0, 0) => "no firearms or accessories".to_owned(),
            (f, 0) => plural(f, "firearm", "firearms"),
            (0, a) => plural(a, "accessory", "accessories"),
            (f, a) => format!(
                "{} and {}",
                plural(f, "firearm", "firearms"),
                plural(a, "accessory", "accessories")
            ),
        }
    }
}

impl RecordCounts {
    /// The noun for the set without its count: "firearm" or "accessory"
    /// for one, "firearms", "accessories" or "firearms and accessories" for
    /// more. The frontend's `kindNoun` says the same.
    pub fn noun(&self) -> &'static str {
        match (self.firearms, self.accessories) {
            (1, 0) => "firearm",
            (0, 1) => "accessory",
            (_, 0) => "firearms",
            (0, _) => "accessories",
            _ => "firearms and accessories",
        }
    }
}

impl std::ops::AddAssign for RecordCounts {
    fn add_assign(&mut self, other: Self) {
        self.firearms += other.firearms;
        self.accessories += other.accessories;
    }
}

/// Enough to name a record the way it is named everywhere and link to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordLabel {
    pub record: RecordRef,
    pub make: String,
    pub model: String,
    /// Firearms only.
    pub nickname: Option<String>,
    /// The firearm type's or the accessory kind's name.
    pub type_name: String,
    pub serial_number: Option<String>,
    pub status: crate::models::firearm::FirearmStatus,
}

/// One entry of a Mounted section or a dispose dialog's list, depth-first
/// below the record asked about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MountedEntry {
    pub label: RecordLabel,
    /// What it is mounted on (the record itself at depth 1).
    pub host: RecordRef,
    /// 1 = mounted directly on the record.
    pub depth: u32,
    /// The record's acquisition date (`YYYY-MM-DD`), so the dispose dialog can
    /// check the disposition date against it for each record it disposes of
    /// (FR-014).
    pub acquisition_date: Option<String>,
}

/// What a record page needs about mounts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MountDetail {
    /// Direct host first, then its host, and so on; empty when not mounted.
    pub chain: Vec<RecordLabel>,
    /// Everything below; empty on a disposed record.
    pub mounted: Vec<MountedEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(firearms: usize, accessories: usize) -> RecordCounts {
        RecordCounts { firearms, accessories }
    }

    #[test]
    fn a_count_names_what_it_counts() {
        assert_eq!(counts(1, 0).describe(), "1 firearm");
        assert_eq!(counts(0, 2).describe(), "2 accessories");
        assert_eq!(counts(2, 1).describe(), "2 firearms and 1 accessory");
        assert_eq!(counts(0, 0).describe(), "no firearms or accessories");
    }

    #[test]
    fn a_noun_names_the_kinds_in_the_set() {
        assert_eq!(counts(1, 0).noun(), "firearm");
        assert_eq!(counts(0, 1).noun(), "accessory");
        assert_eq!(counts(3, 0).noun(), "firearms");
        assert_eq!(counts(0, 2).noun(), "accessories");
        assert_eq!(counts(1, 1).noun(), "firearms and accessories");
    }

    #[test]
    fn counts_are_taken_by_kind() {
        let of = RecordCounts::of([
            RecordRef::Firearm(1),
            RecordRef::Accessory(1),
            RecordRef::Accessory(2),
        ]);
        assert_eq!(of, counts(1, 2));
        assert_eq!(of.total(), 3);
    }
}
