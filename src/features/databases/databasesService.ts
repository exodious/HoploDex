import { CommandFailure, invoke, listen } from "../../services/tauriClient";
import type { SessionScope } from "../session/sessionScope";
import type {
  BackupList,
  BackupProgress,
  BackupsDeleted,
  BackupSettingsInput,
  BackupSettingsSaved,
  ChooserState,
  CollectionSettings,
  CountProgress,
  CreateDatabaseInput,
  DatabaseStatus,
  LockSettingsInput,
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
export function savePassphrase(scope: SessionScope, passphrase: string): Promise<PassphraseSaved> {
  return scope.invoke<PassphraseSaved>("save_passphrase", { passphrase });
}

/** Deletes the passphrase saved for the database at `path`, or for the open
 * one (FR-018). */
export function forgetSavedPassphrase(
  scope: SessionScope,
  path?: string,
): Promise<PassphraseSaved> {
  return scope.invoke<PassphraseSaved>("forget_saved_passphrase", path ? { path } : {});
}

/** Saves the open database's backup settings (FR-024, FR-026). A changed
 * location with backups at the old one is refused until `existingBackups`
 * says what to do with them, which may move or delete them first. */
export function updateBackupSettings(
  scope: SessionScope,
  input: BackupSettingsInput,
): Promise<BackupSettingsSaved> {
  return scope.invoke<BackupSettingsSaved>("update_backup_settings", { ...input });
}

/** Saves the open database's lock settings (FR-034, FR-038). The idle time
 * starts again. */
export function updateLockSettings(
  scope: SessionScope,
  input: LockSettingsInput,
): Promise<CollectionSettings> {
  return scope.invoke<CollectionSettings>("update_lock_settings", { ...input });
}

/** Notices came for a chooser already showing, such as the operation a
 * sleep stopped (FR-037): `getChooserState` has them. */
export function onChooserNotices(handler: () => void): () => void {
  return listen<unknown>("chooser:notices", () => handler());
}

/** Sends a command that works on the open database when it names no
 * `databasePath` (through `scope`, which then is required) and on a database
 * that isn't open when it does (sent with no session: none is open, or it is
 * not the one meant; contracts/tauri-commands.md "Every request names its
 * session"). */
function invokeOnOpenOrPath<T>(
  scope: SessionScope | null,
  command: string,
  args: Record<string, unknown>,
  databasePath: string | undefined,
): Promise<T> {
  if (databasePath) return invoke<T>(command, { ...args, databasePath });
  if (!scope) {
    return Promise.reject(
      new CommandFailure({ code: "DATABASE_CLOSED", message: "The database was closed." }),
    );
  }
  return scope.invoke<T>(command, args);
}

/** The open database's backups (through `scope`), or, with `databasePath`,
 * those of a database that doesn't open (FR-028), for which `scope` is
 * `null`. */
export function listBackups(
  scope: SessionScope | null,
  databasePath?: string,
): Promise<BackupList> {
  return invokeOnOpenOrPath<BackupList>(scope, "list_backups", {}, databasePath);
}

/** Replaces the open database (through `scope`), or the damaged one at
 * `databasePath` (`scope` is `null`), with a backup, which is then open with
 * that backup's passphrase (FR-028). */
export function restoreBackup(
  scope: SessionScope | null,
  backupPath: string,
  backupPassphrase: string,
  databasePath?: string,
): Promise<DatabaseStatus> {
  return invokeOnOpenOrPath<DatabaseStatus>(
    scope,
    "restore_backup",
    { backupPath, backupPassphrase },
    databasePath,
  );
}

/** Changes the open database's passphrase by copy, verify and replace
 * (FR-015, FR-016). */
export function changePassphrase(
  scope: SessionScope,
  currentPassphrase: string,
  newPassphrase: string,
): Promise<PassphraseChanged> {
  return scope.invoke<PassphraseChanged>("change_passphrase", { currentPassphrase, newPassphrase });
}

/** Securely deletes every backup of the open database (FR-029). */
export function deleteAllBackups(scope: SessionScope, confirmed: boolean): Promise<BackupsDeleted> {
  return scope.invoke<BackupsDeleted>("delete_all_backups", { confirmed });
}

export function onRestoreProgress(handler: (progress: RestoreProgress) => void): () => void {
  return listen<RestoreProgress>("restore:progress", handler);
}

export function onBackupsDeleteProgress(handler: (progress: CountProgress) => void): () => void {
  return listen<CountProgress>("backups_delete:progress", handler);
}

export function onBackupsMoveProgress(handler: (progress: BackupProgress) => void): () => void {
  return listen<BackupProgress>("backups_move:progress", handler);
}

export function onPassphraseChangeProgress(
  handler: (progress: PassphraseChangeProgress) => void,
): () => void {
  return listen<PassphraseChangeProgress>("passphrase_change:progress", handler);
}
