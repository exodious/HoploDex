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
  | "backup"
  | "passphraseChange"
  | "restore"
  | "import"
  | "export"
  | "deleteBackups";

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
      reason: "locationUnavailable" | "insufficientSpace" | "interrupted" | "io" | "databaseUnreachable";
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
