/**
 * Mirrors src-tauri/src/models/database.rs: the shared types of
 * specs/003-database-protection-management/contracts/tauri-commands.md.
 */

export type CloseReason =
  | "closed"
  | "switched"
  | "quit"
  | "lockedByUser"
  | "idle"
  | "screenLocked"
  | "sleep"
  | "shutdown"
  | "takenOver";

export type OperationKind =
  "backup" | "passphraseChange" | "restore" | "import" | "export" | "deleteBackups";

export interface RecentDatabase {
  path: string;
  /** The file name without its extension. */
  name: string;
  /** ISO-8601 UTC. */
  lastOpenedAt: string;
  /** The file exists at `path` (FR-012). */
  available: boolean;
  /** FR-017, on this computer. */
  passphraseSaved: boolean;
}

export type ChooserNotice =
  | { kind: "closed"; reason: CloseReason; databasePath: string }
  | {
      kind: "operationStopped";
      databasePath: string;
      operation: OperationKind;
      importedCount?: number;
      deletedCount?: number;
    }
  | { kind: "pendingChangesLost"; databasePath: string }
  | {
      kind: "backupFailed";
      databasePath: string;
      reason:
        "locationUnavailable" | "insufficientSpace" | "interrupted" | "io" | "databaseUnreachable";
    }
  | { kind: "takenOver"; databasePath: string };

export interface CollectionSettings {
  backups: {
    enabled: boolean;
    /** 1–100. */
    keepCount: number;
    /** `path` is resolved on this computer. */
    location: { kind: "default" | "custom"; path: string; available: boolean };
  };
  /** `idleMinutes` is 1–240. */
  lock: { idleEnabled: boolean; idleMinutes: number; onScreenLock: boolean };
}

export interface PendingSummary {
  kind: "firearm" | "policy";
  mode: "add" | "edit" | "dispose" | "restore" | "coverage";
  targetId: number | null;
  label: string;
  savedAt: string;
  /** False when the target no longer exists or `formVersion` is unknown. */
  resumable: boolean;
}

export interface Draft {
  formVersion: number;
  kind: PendingSummary["kind"];
  mode: PendingSummary["mode"];
  targetId: number | null;
  label: string;
  /** The form's own state; at most 1 MiB serialized. */
  values: unknown;
}

export interface DatabaseStatus {
  path: string;
  name: string;
  passphraseSaved: boolean;
  settings: CollectionSettings;
  pendingChanges: PendingSummary | null;
  notes: {
    /** FR-008, until dismissed. */
    diskEncryption: boolean;
    /** research.md §9, shown once. */
    openedBackup: { backupOfName: string; madeAt: string } | null;
    /** ISO time of the backup, once after a restore. */
    restoredWithPassphraseOf: string | null;
    /** Where a damaged database was set aside, once after a restore. */
    damagedFileKeptAt: string | null;
  };
}

export interface CloseOutcome {
  backup: "made" | "notDue" | "alreadyToday" | "off" | "skipped" | "failed" | "notAttempted";
  failureReason?: "locationUnavailable" | "insufficientSpace" | "io" | "databaseUnreachable";
}

export interface BackupInfo {
  path: string;
  fileName: string;
  madeAt: string;
  sizeBytes: number;
}

/** Where the create dialog suggests putting a new database (FR-009). */
export interface SuggestedLocation {
  folder: string;
  name: string;
}

/** `get_chooser_state`: everything the chooser shows before a database is
 * open. */
export interface ChooserState {
  /** Most recent first. */
  recent: RecentDatabase[];
  /** The row selected when the chooser appears (FR-021, FR-033). */
  selectedPath: string | null;
  /** FR-019. */
  keyringAvailable: boolean;
  /** FR-038. */
  screenLockSupported: boolean;
  suggested: SuggestedLocation;
  /** Shown once, then gone. */
  notices: ChooserNotice[];
}

/** `create_database`'s input. */
export interface CreateDatabaseInput {
  folder: string;
  name: string;
  passphrase: string;
  /** FR-004: the user has stored the passphrase and knows it can't be
   * recovered. */
  acknowledgedUnrecoverable: boolean;
}

/** A note shown once in the collection (contracts/ui-databases.md §10). */
export type NoteKind = "diskEncryption" | "openedBackup" | "restored";
