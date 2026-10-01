// Mirrors the "Shared shapes" of specs/006-accessory-links/contracts/tauri-commands.md
// (camelCase over IPC).

export type RecordKind = "firearm" | "accessory";
export type RecordRef = { kind: RecordKind; id: number };

/** Enough to name a record the way it is named everywhere (FR-005; a
 *  firearm by make, model and nickname, 001 FR-031) and link to it. */
export type RecordLabel = {
  record: RecordRef;
  /** Always set for a firearm. */
  make: string | null;
  /** Always set for a firearm. */
  model: string | null;
  /** Firearms only. */
  nickname: string | null;
  /** The firearm type's or the accessory kind's name. */
  typeName: string;
  /** Shown only where FR-012 searches by it. */
  serialNumber: string | null;
  status: "active" | "disposed";
};

/** One entry of a Mounted section or a dispose dialog's list (FR-013,
 *  FR-014): depth-first, below the record asked about. */
export type MountedEntry = {
  label: RecordLabel;
  /** What it is mounted on (the record itself at depth 1). */
  host: RecordRef;
  /** 1 = mounted directly on the record. */
  depth: number;
};

/** What a record page needs about mounts (FR-013). */
export type MountDetail = {
  /** Direct host first, then its host, ...; [] when not mounted. */
  chain: RecordLabel[];
  /** Everything below; [] on a disposed record. */
  mounted: MountedEntry[];
};
