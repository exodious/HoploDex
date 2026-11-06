import { CommandFailure, invoke, listen } from "../../services/tauriClient";
import type { SessionScope } from "../session/sessionScope";
import type {
  BackupCheck,
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
  UpgradeProgress,
} from "./types";

// Typed wrappers for choosing, creating and opening databases (003
// contracts/tauri-commands.md). No passphrase passes through here: the
// backend asks for each one in the native passphrase dialog, so it never
// reaches the page (amended 2026-11-04 to 003 FR-007).

export function getChooserState(): Promise<ChooserState> {
  return invoke<ChooserState>("get_chooser_state");
}

export function createDatabase(input: CreateDatabaseInput): Promise<DatabaseStatus> {
  return invoke<DatabaseStatus>("create_database", { ...input });
}

export interface OpenOptions {
  /** Open with the passphrase saved on this computer, without asking. */
  useSavedPassphrase?: boolean;
  /** Save the typed passphrase in this computer's keyring once it opens the
   * database (003 FR-017), sent only after the confirmation. */
  rememberPassphrase?: boolean;
  /** Open a database marked open on another computer (003 FR-032), sent only
   * after the take-over confirmation. */
  takeOver?: boolean;
}

/** Opens the database at `path` with the passphrase saved on this computer,
 * or with one the backend asks for in the native dialog, which stays up
 * until it opens the database or is cancelled (`PASSPHRASE_CANCELLED`, or
 * the last refusal after a wrong one). */
export function openDatabase(path: string, options: OpenOptions = {}): Promise<DatabaseStatus> {
  return invoke<DatabaseStatus>("open_database", {
    path,
    ...(options.useSavedPassphrase ? { useSavedPassphrase: true } : {}),
    ...(options.rememberPassphrase ? { rememberPassphrase: true } : {}),
    ...(options.takeOver ? { takeOver: true } : {}),
  });
}

/** Takes a database off this computer's recent list. The file is never
 * touched (003 FR-012). */
export function removeRecentDatabase(path: string): Promise<{ removed: true }> {
  return invoke<{ removed: true }>("remove_recent_database", { path });
}

/** Points an unavailable recent entry at where its file now is (003 FR-012). */
export function locateDatabase(path: string, newPath: string): Promise<RecentDatabase> {
  return invoke<RecentDatabase>("locate_database", { path, newPath });
}

/** Saves the open database's passphrase in this computer's keyring (003
 * FR-017): the native dialog says what that gives away and asks for it, and
 * the backend checks it opens the database first. */
export function savePassphrase(scope: SessionScope): Promise<PassphraseSaved> {
  return scope.invoke<PassphraseSaved>("save_passphrase");
}

/** Deletes the passphrase saved for the database at `path`, or for the open
 * one (003 FR-018). */
export function forgetSavedPassphrase(
  scope: SessionScope,
  path?: string,
): Promise<PassphraseSaved> {
  return scope.invoke<PassphraseSaved>("forget_saved_passphrase", path ? { path } : {});
}

/** Saves the open database's backup settings (003 FR-024, FR-026). A changed
 * location with backups at the old one is refused until `existingBackups`
 * says what to do with them, which may move or delete them first. */
export function updateBackupSettings(
  scope: SessionScope,
  input: BackupSettingsInput,
): Promise<BackupSettingsSaved> {
  return scope.invoke<BackupSettingsSaved>("update_backup_settings", { ...input });
}

/** Saves the open database's lock settings (003 FR-034, FR-038). The idle time
 * starts again. */
export function updateLockSettings(
  scope: SessionScope,
  input: LockSettingsInput,
): Promise<CollectionSettings> {
  return scope.invoke<CollectionSettings>("update_lock_settings", { ...input });
}

/** Notices came for a chooser already showing, such as the operation a
 * sleep stopped (003 FR-037): `getChooserState` has them. */
export function onChooserNotices(handler: () => void): () => void {
  return listen<unknown>("chooser:notices", () => handler());
}

/** Sends a command that works on the open database when it names no
 * `databasePath` (through `scope`, which then is required) and on a database
 * that isn't open when it does (sent with no session: none is open, or it is
 * not the one meant; 008 contracts/tauri-commands.md "Every request names its
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
 * those of a database that doesn't open (003 FR-028), for which `scope` is
 * `null`. */
export function listBackups(
  scope: SessionScope | null,
  databasePath?: string,
): Promise<BackupList> {
  return invokeOnOpenOrPath<BackupList>(scope, "list_backups", {}, databasePath);
}

/** Replaces the open database (through `scope`), or the damaged one at
 * `databasePath` (`scope` is `null`), with the backup `checkBackup` checked
 * under `checkId`, which is then open with that backup's passphrase (003
 * FR-028). A backup in an older layout is refused with `UPGRADE_NEEDED`
 * unless `upgrade` is set, which upgrades the restored copy (009 FR-019). */
export function restoreBackup(
  scope: SessionScope | null,
  checkId: number,
  databasePath?: string,
  upgrade?: boolean,
): Promise<DatabaseStatus> {
  return invokeOnOpenOrPath<DatabaseStatus>(
    scope,
    "restore_backup",
    { checkId, ...(upgrade ? { upgrade: true } : {}) },
    databasePath,
  );
}

/** Asks for the backup's passphrase in the native dialog, opens it
 * read-only and says what restoring it would do, before anything is copied:
 * through `scope` for the open database, or with `databasePath` for a
 * damaged one (`scope` is `null`), as `restoreBackup` (009 FR-019). The
 * passphrase stays in the backend under the returned `checkId` until the
 * restore or `dropBackupCheck`. */
export function checkBackup(
  scope: SessionScope | null,
  backupPath: string,
  databasePath?: string,
): Promise<BackupCheck> {
  return invokeOnOpenOrPath<BackupCheck>(scope, "check_backup", { backupPath }, databasePath);
}

/** Lets go of a checked backup and the passphrase kept for it: the restore's
 * confirmation was cancelled. */
export function dropBackupCheck(checkId: number): Promise<null> {
  return invoke<null>("drop_backup_check", { checkId });
}

/** Upgrades the database waiting under `awaitingId` (from `UPGRADE_NEEDED`)
 * after backing it up, in the backup location or, when that couldn't be used,
 * `backupFolder`; the database is then open (009 FR-011, FR-012). No session
 * exists yet, so none is named. */
export function upgradeDatabase(
  awaitingId: number,
  backupFolder?: string,
): Promise<DatabaseStatus> {
  return invoke<DatabaseStatus>("upgrade_database", {
    awaitingId,
    ...(backupFolder ? { backupFolder } : {}),
  });
}

/** Drops the database waiting under `awaitingId`, or stops its backup;
 * nothing once the upgrade has started (009 FR-010, FR-016). */
export function cancelUpgrade(awaitingId: number): Promise<null> {
  return invoke<null>("cancel_upgrade", { awaitingId });
}

/** Changes the open database's passphrase by copy, verify and replace
 * (003 FR-015, FR-016), both passphrases asked for in the native dialog. */
export function changePassphrase(scope: SessionScope): Promise<PassphraseChanged> {
  return scope.invoke<PassphraseChanged>("change_passphrase");
}

/** Securely deletes every backup of the open database (003 FR-029). */
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

export function onUpgradeProgress(handler: (progress: UpgradeProgress) => void): () => void {
  return listen<UpgradeProgress>("upgrade:progress", handler);
}
