# Contract: Tauri IPC Commands (feature 003 deltas)

This document amends `specs/001-firearms-inventory/contracts/tauri-commands.md`
(as already amended by feature 002). Everything there still holds unless a
section below says otherwise. When this feature lands, 001's document gets a
one-line "amended by 003" pointer at its introduction (every command now
needs an open database) and at its error shape (the new `details` field).

All commands stay `async` and off the UI thread. Passphrases cross IPC only as
arguments of the commands marked 🔑, are never returned, logged or echoed in
an error, and are zeroized in the backend when the command returns
(research §1).

---

## Changes to the common shape

```ts
type CommandError = {
  code: string;
  message: string;                          // safe to show; never contains a passphrase
  fieldErrors?: Record<string, string>;
  details?: Record<string, unknown>;        // NEW: structured data for specific codes (below)
};
```

`src/services/tauriClient.ts`'s `CommandFailure` gains `details`.

## Changes to every existing command

Every command in 001's and 002's contracts now runs against **the open
database**, and may additionally fail with:

| Code | When | Frontend response |
|---|---|---|
| `DATABASE_CLOSED` | no database is open (a race with a lock or close) | ignore; the `session:closed` event has already switched to the chooser |
| `PENDING_CHANGES_UNRESOLVED` | the open database has pending changes not yet resumed or discarded (FR-039) | show the pending-changes prompt |
| `DATABASE_TAKEN_OVER` | write commands only: the file was replaced or changed by another computer (FR-032, research §6); nothing was written | the session closes with `reason: "takenOver"` |
| `DATABASE_DAMAGED` | SQLite reported corruption (research §2) | show the damaged message with "Restore from a backup…" |
| `DATABASE_UNAVAILABLE` | write commands only: the database file can't be reached (its storage has disappeared), or a write hit an I/O error (FR-032, research §6); nothing was written, and every later write is refused the same way until the database is opened again | show the message; the form keeps its input; the session stays open until the user closes it |

`import_collection` and `export_collection` may also fail with
`OPERATION_STOPPED` (`details.operation`, plus `details.importedCount` for an
import) when the computer went to sleep mid-run (FR-037). `delete_all_backups`
may fail the same way, with `details.deletedCount`, and
`update_backup_settings` with `existingBackups` set, with `details.deletedCount`
or `details.leftBehindCount` and `details.folder`. The session is
already closed by then, so the chooser shows the notice.

---

## Error codes added by this feature

| Code | `details` | Message (summary) |
|---|---|---|
| `PASSPHRASE_INCORRECT` | `{ savedPassphraseFailed?: boolean, backupsAvailable?: boolean }` | "The passphrase is incorrect, or this file is not a HoploDex database or is damaged." (FR-006) |
| `DATABASE_NOT_FOUND` | `{ path }` | the file is not at that location |
| `DATABASE_UNREADABLE` | `{ path }` | no permission, or the storage is unavailable |
| `DATABASE_IN_USE` | — | "…is open in another copy of HoploDex on this or another computer." (FR-014) |
| `DATABASE_NEWER_VERSION` | — | "…was last used by a newer version of HoploDex. Update HoploDex to open it." (FR-014) |
| `DATABASE_OPEN_ELSEWHERE` | `{ machineName, since }` (`since` ISO-8601 UTC) | FR-032 refusal; the UI offers take-over |
| `DATABASE_EXISTS` | `{ path }` | create would overwrite an existing file |
| `DATABASE_DAMAGED` | `{ backupsAvailable: boolean }` | FR-028 |
| `INSUFFICIENT_SPACE` | `{ bytesNeeded, bytesAvailable, path }` | FR-016 |
| `BACKUP_LOCATION_UNAVAILABLE` | `{ path, reason: "missing" \| "notWritable" \| "insufficientSpace" }` | FR-027 |
| `KEYRING_UNAVAILABLE` | — | FR-019 |
| `OPERATION_STOPPED` | `{ operation, importedCount?, deletedCount?, leftBehindCount?, folder? }` | FR-037 |
| `BACKUPS_AT_OLD_LOCATION` | `{ folder, count, totalBytes }` | a changed backup location needs the move / leave / delete choice first (FR-026, research §22); nothing was saved |
| `BACKUPS_NOT_ALL_DELETED` | `{ deletedCount, failedPaths }` | changing the location with `delete`: some backups at the old location couldn't be deleted, so the old location was kept and they are still the database's (research §22) |
| `OLD_BACKUP_LOCATION_UNAVAILABLE` | `{ folder }` | the old backup location can't be read, so its backups can't be moved or deleted from here; resend with `existingBackups: "leave"` to continue (FR-026); nothing was saved |
| `OPERATION_IN_PROGRESS` | `{ operation }` | another long-running operation is already running |
| `DATABASE_UNAVAILABLE` | `{ path }` | "HoploDex can't reach <path>. Nothing already saved was lost. Close the database and open it again once the drive or network is back." (FR-032) |
| `REPLACE_FAILED` | `{ path }` | the final rename was refused (for example the file was held by another program); the original is unchanged |
| `RESTORE_CANCELLED` | — | "The current database couldn't be backed up, so the restore was cancelled. Nothing has been changed." (FR-028: the "before restoring" backup failed) |
| `CONFIRMATION_REQUIRED` | — | existing code, reused for delete-all-backups and take-over |

`BACKUP_LOCATION_UNAVAILABLE` and `INSUFFICIENT_SPACE` are also how a move of
backups to a new location is refused before anything is written (FR-026).

---

## Shared types

```ts
type RecentDatabase = {
  path: string;
  name: string;                 // file name without extension
  lastOpenedAt: string;         // ISO-8601 UTC
  available: boolean;           // the file exists at path (FR-012)
  passphraseSaved: boolean;     // FR-017, on this computer
};

type ChooserNotice =
  | { kind: "closed"; reason: CloseReason; databasePath: string; idleMinutes?: number }  // a lock; idleMinutes for the idle lock
  | { kind: "operationStopped"; databasePath: string; operation: OperationKind; importedCount?: number; deletedCount?: number;
      leftBehindCount?: number; folder?: string }                                  // the last two for moveBackups
  | { kind: "backupsLeftBehind"; databasePath: string; folder: string; count: number } // a move cut short by a crash (research §22)
  | { kind: "pendingChangesLost"; databasePath: string }
  | { kind: "backupFailed"; databasePath: string; reason: "locationUnavailable" | "insufficientSpace" | "interrupted" | "io" | "databaseUnreachable" }
  | { kind: "takenOver"; databasePath: string };

type CloseReason =
  "closed" | "switched" | "quit" | "lockedByUser" | "idle" | "screenLocked" | "sleep" | "shutdown" | "takenOver";

type OperationKind =
  "backup" | "passphraseChange" | "restore" | "import" | "export" | "deleteBackups" | "moveBackups";

type CollectionSettings = {
  backups: {
    enabled: boolean;
    keepCount: number;                                  // 1–100
    location: { kind: "default" | "custom"; path: string; available: boolean };  // path resolved here
  };
  lock: { idleEnabled: boolean; idleMinutes: number; onScreenLock: boolean };   // 1–240
};

type PendingSummary = {
  formVersion: number;  // the frontend offers Resume only for a form version it knows
  kind: "firearm" | "policy";
  mode: "add" | "edit" | "dispose" | "restore" | "coverage";
  targetId: number | null;
  label: string;
  savedAt: string;
  resumable: boolean;   // false when the target no longer exists or formVersion is unknown
};

type Draft = {
  formVersion: number;
  kind: PendingSummary["kind"];
  mode: PendingSummary["mode"];
  targetId: number | null;
  label: string;
  values: unknown;      // the form's own state; ≤ 1 MiB serialized
};

type DatabaseStatus = {
  path: string;
  name: string;
  passphraseSaved: boolean;
  keyringAvailable: boolean;    // FR-019, for the settings' "This computer" section
  screenLockSupported: boolean; // FR-038, for the settings' "Locking" section
  settings: CollectionSettings;
  pendingChanges: PendingSummary | null;
  notes: {
    diskEncryption: boolean;                                       // FR-008, until dismissed
    openedBackup: { backupOfName: string; madeAt: string } | null; // research §9, once
    restoredWithPassphraseOf: string | null;                       // ISO time of the backup, once after a restore
    damagedFileKeptAt: string | null;                              // where a damaged database was set aside, once after a restore
  };
};

type CloseOutcome = {
  backup: "made" | "notDue" | "alreadyToday" | "off" | "skipped" | "failed" | "notAttempted";
  failureReason?: "locationUnavailable" | "insufficientSpace" | "io" | "databaseUnreachable"; // databaseUnreachable: research §6
};

type BackupInfo = { path: string; fileName: string; madeAt: string; sizeBytes: number };

type ExistingBackupsOutcome =
  | { action: "leave" }
  | { action: "delete"; deletedCount: number }
  | { action: "move"; movedCount: number;
      leftBehind: { count: number; folder: string;
                    reason: "nameTaken" | "locationUnavailable" | "insufficientSpace" | "io" } | null };
```

---

## Commands: choosing, creating and opening (User Stories 1, 2, 5)

### `get_chooser_state`
- **Input**: none
- **Output**: `{ recent: RecentDatabase[]; selectedPath: string | null; keyringAvailable: boolean; screenLockSupported: boolean; suggested: { folder: string; name: string }; notices: ChooserNotice[] }`
- Notices are returned once and then removed from `machine.json`. `selectedPath` is the most recent entry at startup (FR-021), or the database just locked or closed (FR-033).

### `create_database` 🔑
- **Input**: `{ folder: string; name: string; passphrase: string; acknowledgedUnrecoverable: boolean }`
- **Output**: `DatabaseStatus` (the new database is open)
- **Errors**: `VALIDATION_ERROR` (`fieldErrors.name`, `.folder`, `.passphrase`; `.acknowledgedUnrecoverable` when false, FR-004), `DATABASE_EXISTS`
- Remembering the passphrase is offered at the prompt and in the settings (US5-1), not at creation.
- Creates `<folder>/<name>.hoplodex` with pinned settings, default `collection_settings`, a new `database_id`, and the open marker set. Adds the recent entry.

### `open_database` 🔑
- **Input**: `{ path: string; passphrase?: string; useSavedPassphrase?: boolean; rememberPassphrase?: boolean; takeOver?: boolean }`. Exactly one of `passphrase` or `useSavedPassphrase: true` is given.
- **Output**: `DatabaseStatus`
- **Errors**: `DATABASE_NOT_FOUND`, `DATABASE_UNREADABLE`, `DATABASE_IN_USE`, `PASSPHRASE_INCORRECT` (with `savedPassphraseFailed` when the saved one was used, US5-5), `DATABASE_NEWER_VERSION`, `DATABASE_OPEN_ELSEWHERE` (unless `takeOver: true`), `DATABASE_DAMAGED`
- `useSavedPassphrase` finds the saved passphrase through the recent entry's cached `databaseId`; when there is none, or it no longer opens the file, the open fails with `PASSPHRASE_INCORRECT { savedPassphraseFailed: true }`. `rememberPassphrase` on a computer without a keyring still opens the database, with `passphraseSaved: false`.
- Any other database that is open is closed first, as a `switched` close. The frontend only opens from the chooser, where nothing is open (FR-010: switching is a lock, then an open). On success with a typed passphrase: the recent entry is added or refreshed, and the keyring entry is written when `rememberPassphrase` is set, or refreshed when it was saved but failed (FR-018). Nothing is written to the file on any failure (FR-006, FR-014).

### `remove_recent_database`
- **Input**: `{ path: string }` → **Output**: `{ removed: true }`
- Also deletes the saved keyring passphrase for it (FR-018). Never touches the file.

### `locate_database`
- **Input**: `{ path: string; newPath: string }` → **Output**: `RecentDatabase`
- Replaces an unavailable entry's path (FR-012) and keeps its other fields.

---

## Commands: the open session

### `get_database_status`
- **Input**: none → **Output**: `DatabaseStatus` · **Errors**: `DATABASE_CLOSED`

### `resolve_pending_changes`
- **Input**: `{ action: "resume" | "discard" }`
- **Output**: `{ draft: Draft | null }`. `draft` is present for `resume` and absent for `discard`.
- Removes the `pending_changes` row either way (FR-039). Collection commands are accepted from then on.

### `stage_pending_changes`
- **Input**: `{ draft: Draft | null }` → **Output**: `null`
- Keeps the draft in backend memory only (research §16). `null` clears it. Validated: `VALIDATION_ERROR` with `fieldErrors.values` over 1 MiB, `fieldErrors.mode` for a kind and mode that don't go together (`coverage`, `dispose` and `restore` are firearm-only), `fieldErrors.targetId` when a target is missing (or given for an add), `fieldErrors.label` over 200 characters.

### `dismiss_note`
- **Input**: `{ note: "diskEncryption" | "openedBackup" | "restored" }` → **Output**: `null`
- `diskEncryption` sets `app_state.disk_encryption_note_dismissed` (housekeeping, FR-008). The other two are session-only.

### `note_activity`
- **Input**: none → **Output**: `null`. Called at most once per second while there is input (research §15).

### `set_idle_paused`
- **Input**: `{ reason: "nativeDialog"; paused: boolean }` → **Output**: `null`

### `close_database`
- **Input**: `{ reason: "closed" | "switched" }` → **Output**: `CloseOutcome`
- The normal close (FR-010, FR-022, FR-025, FR-032). Emits `session:closing`, then, if a backup runs, `backup:progress`, then `session:closed`. Within the application a database is closed by locking it (`lock_database`); this command remains for the "couldn't be shown" panel (contracts/ui-databases.md §13, research §17).

### `lock_database`
- **Input**: `{ draft: Draft | null }` → **Output**: `CloseOutcome`
- "Lock now" (FR-033, FR-035). No confirmation. The draft, if any, becomes pending changes, and then the same normal close runs with `reason: "lockedByUser"`. The idle and screen-lock locks run the same procedure from the backend, using the staged draft.

### `skip_backup`
- **Input**: none → **Output**: `null`. Stops the backup the current close is making (FR-027). The changes stay waiting. Anything else running is left alone.

### `quit_application`
- **Input**: none → **Output**: never returns normally (the process exits)
- Normal close with `reason: "quit"` if a database is open, then exit. The frontend calls it after `app:quit-requested` and the unsaved-changes question.

---

## Commands: settings (User Stories 3, 5, 6)

### `update_backup_settings`
- **Input**: `{ enabled: boolean; keepCount: number; location: { kind: "default" } | { kind: "custom"; path: string }; existingBackups?: "move" | "leave" | "delete" }`
- **Output**: `{ settings: CollectionSettings; existingBackups: ExistingBackupsOutcome | null }` (`null` when the location did not change, or the old folder held no backups; `{ action: "leave" }` after `leave` for an old folder that couldn't be read, whose backups weren't counted)
- **Errors**: `VALIDATION_ERROR` (`fieldErrors.keepCount`, `fieldErrors.location`); for a changed location, `BACKUPS_AT_OLD_LOCATION` or `OLD_BACKUP_LOCATION_UNAVAILABLE` when `existingBackups` is missing (or, for an unreadable old folder, is not `leave`); for `move`, `BACKUP_LOCATION_UNAVAILABLE` and `INSUFFICIENT_SPACE`, checked before anything is written; for `delete`, `BACKUPS_NOT_ALL_DELETED`; `OPERATION_IN_PROGRESS`; `OPERATION_STOPPED` (`move` or `delete` stopped by a sleep)
- A collection change (FR-025). Lowering `keepCount` does not delete anything straight away; the next successful backup rotates.
- **A changed location** (FR-026, research §22) is one whose resolved folder differs from the current one's. With backups of this database in the old folder, nothing is saved until the user has chosen:
  - `leave`: the settings are saved; the backups stay where they are and are no longer the database's.
  - `delete`: sent only after FR-029's destructive confirmation, which it stands for, as `takeOver: true` does for a take-over. The old folder's backups are deleted as by `delete_all_backups` (secure deletion, `backups_delete:progress`, stopping between files), and the settings are saved only if every one was deleted. Otherwise nothing is saved, and the command fails with `BACKUPS_NOT_ALL_DELETED`, or with `OPERATION_STOPPED` at a sleep.
  - `move`: the new folder's availability and space are checked, the settings are saved, then the backups are moved oldest first as a long-running operation (`moveBackups`), each hard-linked on the same drive, or copied, verified byte for byte and the original securely deleted. A name already taken at the new folder is never overwritten. The move ends at the first failure, keeping the new location; `leftBehind` reports what stayed in the old folder and why (`nameTaken` when names were the only reason). Nothing is rotated.
  - The cached backup folder in this computer's recent entry is refreshed whenever the location is saved (research §8, §11).
- **Progress** (`move`): `backups_move:progress` `{ processed; total; showNow }` in bytes, copying and reading back each counted (research §22); `showNow` as for `backup:progress`. The first event, with the total, is sent within 100 ms, once the checks have passed. A same-drive move reports each file's bytes as it is linked.

### `update_lock_settings`
- **Input**: `{ idleEnabled: boolean; idleMinutes: number; onScreenLock: boolean }`
- **Output**: `CollectionSettings` · **Errors**: `VALIDATION_ERROR`
- A collection change (FR-025, FR-034). Takes effect at once: the idle clock restarts.

### `save_passphrase` 🔑
- **Input**: `{ passphrase: string }` → **Output**: `{ passphraseSaved: true }`
- **Errors**: `PASSPHRASE_INCORRECT` (checked with the page-1 probe, research §1a; `fieldErrors.passphrase`), `KEYRING_UNAVAILABLE`
- The UI sends it only after the FR-017 confirmation.

### `forget_saved_passphrase`
- **Input**: `{ path?: string }` (defaults to the open database) → **Output**: `{ passphraseSaved: false }`
- **Errors**: `KEYRING_UNAVAILABLE` (the keyring can't be reached to delete it; nothing is changed)
- The keyring entry is per database id, so every recent entry for that database stops being marked saved.

---

## Commands: long-running operations (User Stories 3, 4)

Each registers in the operations registry (research §13): at most one at a
time (`OPERATION_IN_PROGRESS`); the idle clock pauses while it runs; and a
sleep stops it (FR-037).

### `change_passphrase` 🔑
- **Input**: `{ currentPassphrase: string; newPassphrase: string }`
- **Output**: `{ oldFileRemoved: boolean; oldFilePath?: string; passphraseSaved: boolean }`
- **Errors**: `PASSPHRASE_INCORRECT` (current, `fieldErrors.currentPassphrase`, checked on the file's first page, research §1a), `VALIDATION_ERROR` (new, `fieldErrors.newPassphrase`: too short, a NUL, or the same as the current passphrase once both are NFC-normalized), `INSUFFICIENT_SPACE` (checked before anything is written), `PENDING_CHANGES_UNRESOLVED` (the copy would drop them, FR-039), `REPLACE_FAILED` (the original is unchanged and is reopened with the current passphrase, so the session stays open), `OPERATION_STOPPED`, and `INTERNAL_ERROR` when the new copy fails its checks (research §4; nothing is changed)
- **Progress**: `passphrase_change:progress` `{ phase: "copying" | "checking" | "replacing"; processed: number; total: number }` (bytes; `checking` and `replacing` are indeterminate, with `total: 0`). The first `copying` event, with the total, is sent once the space check has passed and before the current passphrase is checked, whose key derivation takes longer than SC-005's 100 ms; a `PASSPHRASE_INCORRECT` refusal can follow it
- Copy, verify, then replace (FR-015, FR-016, research §3, §4). Sets `changes_waiting` in the new copy (FR-025). Updates the saved keyring passphrase when there is one (FR-018). If the old file cannot be removed, `oldFileRemoved: false` and `oldFilePath` are returned (US4-5).

### `list_backups`
- **Input**: `{ databasePath?: string }` (defaults to the open database; given for a damaged database that cannot be opened, research §8)
- **Output**: `{ folder: string; available: boolean; backups: BackupInfo[] }` (newest first). `BackupInfo.madeAt` is the local time in the backup's name, without a zone (`2026-09-25T14:30:05`)
- **Errors**: `NOT_FOUND` when `databasePath` is not in this computer's recent list, which is where its backup folder and id are cached

### `restore_backup` 🔑
- **Input**: `{ backupPath: string; backupPassphrase: string; databasePath?: string }` (`databasePath` only when restoring a damaged database with nothing open)
- **Output**: `DatabaseStatus` (the restored database is open), with `notes.restoredWithPassphraseOf` set
- **Errors**: `PASSPHRASE_INCORRECT` (for the backup, checked on its first page before anything is copied), `DATABASE_DAMAGED` (the backup itself fails verification), `BACKUP_LOCATION_UNAVAILABLE` (checked before anything is written; not for a damaged database), `INSUFFICIENT_SPACE` (checked before anything is written: room for the restored copy in the database's folder and for the "before restoring" backup in the backup folder), `RESTORE_CANCELLED` (the "before restoring" backup failed), `NOT_FOUND` (the backup is gone), `REPLACE_FAILED` (the database is then closed, since the backend holds no passphrase for it: `session:closed` is emitted and the user opens it again), `OPERATION_STOPPED`
- **Progress**: `restore:progress` `{ phase: "copying" | "checking" | "savingCurrent" | "replacing"; processed; total }`. The first `copying` event, with the total, is sent once the location and space checks have passed and before the backup's passphrase is checked, as for `change_passphrase`; a `PASSPHRASE_INCORRECT` refusal can follow it
- Steps in research §8. `savingCurrent` is the "before restoring" backup, made whatever the once-a-day limit says and even when automatic backups are off (FR-028); if it fails or is stopped, the restore is abandoned and the database is unchanged. A damaged database is renamed aside, not deleted, and its new path is returned in `notes.damagedFileKeptAt`.

### `delete_all_backups`
- **Input**: `{ confirmed: boolean }` → **Output**: `{ deletedCount: number; failedPaths: string[] }`
- **Errors**: `CONFIRMATION_REQUIRED` when `confirmed` is false (FR-029);
  `OPERATION_STOPPED { operation: "deleteBackups", deletedCount }` when the
  computer went to sleep mid-run (FR-037)
- Deletes only this database's backups (the `<id8>` listing of research §7,
  never another database's files in a shared custom folder), each by secure
  deletion (research §12). It stops between files; a file whose overwrite
  was under way when stopped is removed without finishing it
- **Progress**: `backups_delete:progress` `{ processed; total }` (files)

---

## Events (backend → frontend)

| Event | Payload | Meaning |
|---|---|---|
| `session:closing` | `{ reason: CloseReason }` | A normal close has started. The frontend shows the closing screen (a backup may follow) |
| `backup:progress` | `{ processed: number; total: number; showNow: boolean }` | bytes. `showNow` is true when the estimate is over 1 s (research §7). Otherwise the frontend shows the bar only if the close is still running 1 s later |
| `session:closed` | `{ reason: CloseReason; databasePath: string; outcome?: CloseOutcome; stoppedOperation?: OperationKind }` | The database is closed. The frontend drops **all** collection state and shows the chooser with `databasePath` selected (FR-020, FR-033). For `sleep`, `shutdown` and `takenOver` it is emitted **before** the backend finishes closing (research §14), so what is only known later (the stopped operation's count, pending changes that could not be kept) comes as chooser notices |
| `chooser:notices` | `{}` | A chooser notice was kept after the chooser may already be showing (an immediate close's later steps). The chooser calls `get_chooser_state` and adds its notices |
| `passphrase_change:progress`, `restore:progress`, `backups_delete:progress`, `backups_move:progress` | see above | |
| `import_collection:progress`, `export_collection:progress` | unchanged from 001 | |
| `system:clear-passphrase-fields` | `{}` | The screen locked or the computer is going to sleep. Every passphrase field resets (FR-007), whatever the state |
| `app:quit-requested` | `{}` | The window's close button or an application quit. The frontend asks about unsaved changes, then calls `quit_application` |

---

## `main.rs` registration

Added to `generate_handler!`: `get_chooser_state`, `create_database`,
`open_database`, `remove_recent_database`, `locate_database`,
`get_database_status`, `resolve_pending_changes`, `stage_pending_changes`,
`dismiss_note`, `note_activity`, `set_idle_paused`, `close_database`,
`lock_database`, `skip_backup`, `quit_application`,
`update_backup_settings`, `update_lock_settings`, `save_passphrase`,
`forget_saved_passphrase`, `change_passphrase`, `list_backups`,
`restore_backup`, `delete_all_backups`. `setup` no longer opens a database.
It manages `Session` (which holds the operations registry and the
`IdleClock`) and `MachineSettings`, starts the system-events listener
(research §14) and the idle clock's 1 s tick, and runs the startup sweeps:
decrypted document copies (001 FR-035), unfinished backup (research §7), and
interrupted swaps (research §4).
