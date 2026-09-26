import { invoke, listen } from "../../services/tauriClient";
import type {
  BackupList,
  BackupsDeleted,
  BackupSettingsInput,
  ChooserState,
  CollectionSettings,
  CountProgress,
  CreateDatabaseInput,
  DatabaseStatus,
  PassphraseChanged,
  PassphraseChangeProgress,
  PassphraseSaved,
  RecentDatabase,
  RestoreProgress,
} from "./types";

// Typed wrappers for choosing, creating and opening databases (specs/003
// contracts/tauri-commands.md). Passphrases are passed straight through as
// command arguments and never kept (FR-007).

export function getChooserState(): Promise<ChooserState> {
  return invoke<ChooserState>("get_chooser_state");
}

export function createDatabase(input: CreateDatabaseInput): Promise<DatabaseStatus> {
  return invoke<DatabaseStatus>("create_database", { ...input });
}

export interface OpenOptions {
  /** Save the typed passphrase in this computer's keyring once it opens the
   * database (FR-017), sent only after the confirmation. */
  rememberPassphrase?: boolean;
  /** Open a database marked open on another computer (FR-032), sent only
   * after the take-over confirmation. */
  takeOver?: boolean;
}

/** Opens the database at `path` with the typed passphrase, or, with `null`,
 * the one saved on this computer. */
export function openDatabase(
  path: string,
  passphrase: string | null,
  options: OpenOptions = {},
): Promise<DatabaseStatus> {
  return invoke<DatabaseStatus>("open_database", {
    path,
    ...(passphrase === null ? { useSavedPassphrase: true } : { passphrase }),
    ...(options.rememberPassphrase ? { rememberPassphrase: true } : {}),
    ...(options.takeOver ? { takeOver: true } : {}),
  });
}

/** Takes a database off this computer's recent list. The file is never
 * touched (FR-012). */
export function removeRecentDatabase(path: string): Promise<{ removed: true }> {
  return invoke<{ removed: true }>("remove_recent_database", { path });
}

/** Points an unavailable recent entry at where its file now is (FR-012). */
export function locateDatabase(path: string, newPath: string): Promise<RecentDatabase> {
  return invoke<RecentDatabase>("locate_database", { path, newPath });
}

/** Saves the open database's passphrase in this computer's keyring, once
 * the backend has checked it opens the database (FR-017). Sent only after
 * the confirmation. */
export function savePassphrase(passphrase: string): Promise<PassphraseSaved> {
  return invoke<PassphraseSaved>("save_passphrase", { passphrase });
}

/** Deletes the passphrase saved for the database at `path`, or for the open
 * one (FR-018). */
export function forgetSavedPassphrase(path?: string): Promise<PassphraseSaved> {
  return invoke<PassphraseSaved>("forget_saved_passphrase", path ? { path } : {});
}

/** Saves the open database's backup settings (FR-024, FR-026). */
export function updateBackupSettings(input: BackupSettingsInput): Promise<CollectionSettings> {
  return invoke<CollectionSettings>("update_backup_settings", { ...input });
}

/** The open database's backups, or, with `databasePath`, those of a
 * database that doesn't open (FR-028). */
export function listBackups(databasePath?: string): Promise<BackupList> {
  return invoke<BackupList>("list_backups", databasePath ? { databasePath } : {});
}

/** Replaces the open database, or the damaged one at `databasePath`, with a
 * backup, which is then open with that backup's passphrase (FR-028). */
export function restoreBackup(
  backupPath: string,
  backupPassphrase: string,
  databasePath?: string,
): Promise<DatabaseStatus> {
  return invoke<DatabaseStatus>("restore_backup", {
    backupPath,
    backupPassphrase,
    ...(databasePath ? { databasePath } : {}),
  });
}

/** Changes the open database's passphrase by copy, verify and replace
 * (FR-015, FR-016). */
export function changePassphrase(
  currentPassphrase: string,
  newPassphrase: string,
): Promise<PassphraseChanged> {
  return invoke<PassphraseChanged>("change_passphrase", { currentPassphrase, newPassphrase });
}

/** Securely deletes every backup of the open database (FR-029). */
export function deleteAllBackups(confirmed: boolean): Promise<BackupsDeleted> {
  return invoke<BackupsDeleted>("delete_all_backups", { confirmed });
}

export function onRestoreProgress(handler: (progress: RestoreProgress) => void): () => void {
  return listen<RestoreProgress>("restore:progress", handler);
}

export function onBackupsDeleteProgress(handler: (progress: CountProgress) => void): () => void {
  return listen<CountProgress>("backups_delete:progress", handler);
}

export function onPassphraseChangeProgress(
  handler: (progress: PassphraseChangeProgress) => void,
): () => void {
  return listen<PassphraseChangeProgress>("passphrase_change:progress", handler);
}
