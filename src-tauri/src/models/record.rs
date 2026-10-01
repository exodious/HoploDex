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

/// Enough to name a record the way it is named everywhere and link to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordLabel {
    pub record: RecordRef,
    /// Always set for a firearm.
    pub make: Option<String>,
    /// Always set for a firearm.
    pub model: Option<String>,
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
