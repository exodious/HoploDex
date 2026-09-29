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
  "backup" | "passphraseChange" | "restore" | "import" | "export" | "deleteBackups" | "moveBackups";

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
  /** Its backups, by file name (FR-040); `null` when this computer doesn't
   * know where they are, can't read the folder, or a chosen folder isn't
   * there (a missing default folder means none yet). */
  backups: BackupSummary | null;
  /** ISO-8601 UTC: the file changed at this time, after this computer last
   * closed it (FR-040). */
  changedSinceLeftAt: string | null;
}

export interface BackupSummary {
  count: number;
  /** Local date and time, without a zone. `null` when there are none. */
  latestMadeAt: string | null;
  oldestMadeAt: string | null;
}

export type ChooserNotice =
  /** A lock; `idleMinutes` for the idle lock. */
  | { kind: "closed"; reason: CloseReason; databasePath: string; idleMinutes?: number }
  | {
      kind: "operationStopped";
      databasePath: string;
      operation: OperationKind;
      importedCount?: number;
      deletedCount?: number;
      /** For `moveBackups`: how many are still in `folder`. */
      leftBehindCount?: number;
      folder?: string;
    }
  /** A move of backups cut short by a crash (research.md §22). */
  | { kind: "backupsLeftBehind"; databasePath: string; folder: string; count: number }
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
  /** The form's draft version, which must be one this frontend knows. */
  formVersion: number;
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
  /** FR-017, on this computer. */
  passphraseSaved: boolean;
  /** Passphrases can be saved on this computer (FR-019). */
  keyringAvailable: boolean;
  /** This desktop reports a screen lock (FR-038). */
  screenLockSupported: boolean;
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

/** `list_backups`: a database's backup folder and what is in it (FR-028). */
export interface BackupList {
  folder: string;
  /** The folder exists on this computer. */
  available: boolean;
  /** Newest first. */
  backups: BackupInfo[];
}

/** `delete_all_backups`'s answer (FR-029). */
export interface BackupsDeleted {
  deletedCount: number;
  /** Backups that could not be deleted, left in place. */
  failedPaths: string[];
}

/** Where backups go, as `update_backup_settings` takes it. */
export type BackupLocationInput = { kind: "default" } | { kind: "custom"; path: string };

/** What to do with the backups at the old location when it changes
 * (FR-026). */
export type ExistingBackupsChoice = "move" | "leave" | "delete";

/** `update_backup_settings`'s input (FR-024, FR-026). */
export interface BackupSettingsInput {
  enabled: boolean;
  /** 1–100. */
  keepCount: number;
  location: BackupLocationInput;
  /** Sent once the user has been asked, after `BACKUPS_AT_OLD_LOCATION` or
   * `OLD_BACKUP_LOCATION_UNAVAILABLE`. */
  existingBackups?: ExistingBackupsChoice;
}

/** What was done with the backups at the old location (FR-026). */
export type ExistingBackupsOutcome =
  | { action: "leave" }
  | { action: "delete"; deletedCount: number }
  | {
      action: "move";
      movedCount: number;
      /** `nameTaken` only when taken names were the only reason. */
      leftBehind: {
        count: number;
        folder: string;
        reason: "nameTaken" | "locationUnavailable" | "insufficientSpace" | "io";
      } | null;
    };

/** `update_backup_settings`'s answer. `existingBackups` is `null` when the
 * location did not change or no backups were at the old one. */
export interface BackupSettingsSaved {
  settings: CollectionSettings;
  existingBackups: ExistingBackupsOutcome | null;
}

/** `BACKUPS_AT_OLD_LOCATION`'s details: the backups the question is
 * about. */
export interface OldLocationBackups {
  folder: string;
  count: number;
  totalBytes: number;
}

/** `backup:progress`: bytes copied by the backup a close is making; also
 * `backups_move:progress`, bytes copied and read back by a move of backups
 * (FR-026). */
export interface BackupProgress {
  processed: number;
  total: number;
  /** The copy is expected to take over a second: show the bar at once. */
  showNow: boolean;
}

/** `restore:progress` (contracts/tauri-commands.md). `checking` and
 * `replacing` are indeterminate, with `total: 0`. */
export interface RestoreProgress {
  phase: "copying" | "checking" | "savingCurrent" | "replacing";
  processed: number;
  total: number;
}

/** `passphrase_change:progress`: bytes while copying; `checking` and
 * `replacing` are indeterminate, with `total: 0`. */
export interface PassphraseChangeProgress {
  phase: "copying" | "checking" | "replacing";
  processed: number;
  total: number;
}

/** `change_passphrase`'s answer (FR-016, FR-018). */
export interface PassphraseChanged {
  /** The previous file was securely deleted. */
  oldFileRemoved: boolean;
  /** Where the previous file still is, when it couldn't be deleted. */
  oldFilePath?: string;
  /** The passphrase saved in this computer's keyring was updated. */
  passphraseSaved: boolean;
}

/** `save_passphrase`'s and `forget_saved_passphrase`'s answer. */
export interface PassphraseSaved {
  passphraseSaved: boolean;
}

/** `backups_delete:progress`: files deleted so far. */
export interface CountProgress {
  processed: number;
  total: number;
}

/** `update_lock_settings`'s input (FR-034, FR-038). */
export type LockSettingsInput = CollectionSettings["lock"];
