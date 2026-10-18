# Contract: Tauri IPC Commands (feature 009 deltas)

A delta against the IPC contract as features 001 to 008 left it (003's
`contracts/tauri-commands.md` for databases and backups, 008's for session
scoping). Unchanged commands keep their input, output and errors.

## Shared types (amended)

```ts
type BackupInfo = {
  // …unchanged fields…
  beforeUpgradeTo: string | null;  // NEW: "0.2.0" for a backup made before upgrading to it (FR-013)
};

type OpenedBackupNote = {
  backupOfName: string;
  madeAt: string;
  beforeUpgradeTo: string | null;  // NEW: from the backup's stamp (FR-013)
};

type UpgradedNote = {               // NEW (FR-017)
  version: string;                  // the version it was upgraded for
  backupPath: string;               // the backup made before it; for a restore, the backup restored from
  atBackupLocation: boolean;        // false when saved in a folder the user chose (FR-012)
};

type DatabaseNotes = {
  // …unchanged fields…
  upgraded: UpgradedNote | null;    // NEW: once, after an open or restore that upgraded it
};

type OperationKind = /* …unchanged values… */ | "upgrade";  // NEW
```

`InsurancePolicy`, `Photo` and `DocumentAttachment` are unchanged. The new
identifiers are in no IPC type (FR-003).

## Errors (added and amended)

| Code | When | `details` |
|---|---|---|
| `UPGRADE_NEEDED` (new) | `open_database` found steps due; the database waits, keyed and locked, with nothing written | `{ awaitingId: number, version: string, backupFolder: string }`. `version` is the build's `APP_VERSION`; `backupFolder` is the backup location resolved on this computer |
| `UPGRADE_NEEDED` (new) | `restore_backup` without `upgrade: true` for a backup that needs one; nothing copied or written | `{ version: string }` |
| `UPGRADE_FAILED` (new) | the upgrade transaction failed (a step, the link check, the commit); rolled back, the database as it was | `{ backupPath: string, atBackupLocation: boolean }` |
| `UPGRADE_BACKUP_FAILED` (new) | the backup before the upgrade failed while it was being written (I/O), at the location or a chosen folder; the database still waits | `{ folder: string }` |
| `BACKUP_LOCATION_UNAVAILABLE` (amended) | also from `upgrade_database`: the backup location, or the chosen folder, is missing, not writable or full; the database still waits | unchanged: `{ path, reason }` (003's field name; _amended 2026-10-17: was written `folder` here_) |
| `INSUFFICIENT_SPACE` (amended) | also from `upgrade_database`; the database still waits | unchanged |
| `OPERATION_STOPPED` (amended) | also `operation: "upgrade"`: the backup before the upgrade was cancelled or stopped by a sleep or shutdown; partial removed, the database closed untouched | unchanged shape |
| `DISK_FULL` (amended) | also from `upgrade_database`, when the database's own disk fills during the upgrade; rolled back, the database as it was, dropped | `{ path: string }` as before, and now `backupPath: string, atBackupLocation: boolean`: the backup made before the upgrade, which was finished before any step ran (_amended 2026-10-17, the owner's decision D1: the chooser says where the backup is, FR-015_) |
| `DATABASE_NEWER_VERSION` (amended, FR-018) | unchanged condition | NEW: `{ backupFolder?: string }`, the backup folder this computer cached for the database, when it has one |
| `VALIDATION_ERROR` (amended, FR-008) | also from `create_insurance_policy` and `update_insurance_policy` when another policy has the same name | `fieldErrors.name`: "“<other name>” is already the name of another policy. Give this one a different name, such as “<other name> 2026”." The year is the current one |

## Commands (amended)

### `open_database` (amended: FR-009, FR-010)

- **Input**: unchanged.
- **Output**: `DatabaseStatus`, as today, when no step is due.
- **New refusal** `UPGRADE_NEEDED`, when steps are due.
  - It comes after the passphrase, the newer-version check and the open
    marker (a database open elsewhere is still `DATABASE_OPEN_ELSEWHERE`
    first, and the take-over resend reaches `UPGRADE_NEEDED` in turn).
  - Nothing has been written to the file. The connection waits, holding the
    file's exclusive lock, until `upgrade_database` or `cancel_upgrade` names
    its `awaitingId`, or until it is dropped (research.md §4).
- **Remembering the passphrase.** `rememberPassphrase` (and the refresh of a
  saved passphrase that failed, 003 FR-018) takes effect when the upgrade
  succeeds, not at this refusal.
- **Any database already waiting is dropped first**, as an open switches
  away from an open database.

### `restore_backup` (amended: FR-019)

- **Input**: adds `upgrade?: boolean`.
- **Without `upgrade: true`**: a backup whose layout is older than the
  build's is refused with `UPGRADE_NEEDED` before anything is copied, closed
  or written.
- **With it**: the restore upgrades the restored copy at `.<file>.new`, after
  the copy has been checked and stamped and before anything is replaced
  (research.md §9). A failure there removes the copy and returns
  `UPGRADE_FAILED` with `backupPath` set to the backup restored from. The
  database is as it was, and the "before restoring" backup has been made.
  _Amended 2026-10-17: no "before restoring" backup has been made yet, since
  `upgrading` comes before `savingCurrent` (below); nothing is written._
  `upgrade: true` for a backup in the current layout changes nothing.
- **Output**: `DatabaseStatus`, whose `notes.upgraded` is set when it was
  upgraded.
- **Progress**: `restore:progress` gains the phase `"upgrading"`, between
  `"checking"` and `"savingCurrent"`.

### `list_backups` (amended: FR-013)

- Each `BackupInfo` carries `beforeUpgradeTo`.

### `dismiss_note` (amended: FR-017)

- **Input**: `note` also takes `"upgraded"`, which is session-only like
  `"openedBackup"` and `"restored"`.

### `create_insurance_policy`, `update_insurance_policy` (amended: FR-007, FR-008)

- **New refusal**: `VALIDATION_ERROR` with `fieldErrors.name`, as above, when
  another policy (current, expired or future; for an update, not the one
  being edited) has the same name, ignoring A–Z letter case and surrounding
  spaces. Nothing is saved.

### `import_collection` (amended: FR-006)

- **Input and output**: unchanged.
- **Row errors**: a firearm or accessory row's `insurance_policy_name` is
  looked up as `trim(name) = trim(cell) COLLATE NOCASE`. No match is the
  row error `Unknown insurance policy: <cell>`, unchanged; with unique names
  there is never more than one match (contracts/spreadsheet-format.md).

## Commands (added)

All three are unscoped (`UNSCOPED_COMMANDS`): no session exists when they
run. Each is listed in `generate_handler!`, `commands_list.rs`'s `COMMANDS`
and `capabilities/default.json` as `allow-<command>` for the main web view
only.

### `upgrade_database` (FR-011 to FR-017)

- **Input**: `{ awaitingId: number; backupFolder?: string }`.
  - `backupFolder` is a folder the user chose for this one backup (FR-012).
    It must be absolute, an existing directory, writable and roomy. It is
    never created.
  - Without it, the backup goes to the database's backup location.
- **Output**: `DatabaseStatus`, the database open and upgraded, with
  `notes.upgraded` set.
- **Order**:
  1. take the waiting database (`DATABASE_CLOSED` when `awaitingId` isn't the
     one waiting: it was cancelled, dropped, or replaced);
  2. read the open marker again (`DATABASE_OPEN_ELSEWHERE` if another
     computer has marked it open since; dropped);
  3. check the backup folder and its room (refusals as above; the database
     waits again);
  4. make the backup (`upgrade:progress` phase `backingUp`; cancellable);
  5. upgrade (`upgrade:progress` phase `upgrading`; not cancellable);
  6. open as every open does.
- **Errors**: `DATABASE_CLOSED`, `DATABASE_OPEN_ELSEWHERE`,
  `BACKUP_LOCATION_UNAVAILABLE`, `INSUFFICIENT_SPACE`,
  `UPGRADE_BACKUP_FAILED`, `OPERATION_STOPPED`, `UPGRADE_FAILED`, `DISK_FULL`
  (the database's own disk filled during the upgrade; rolled back; dropped).
  _Amended 2026-10-17: that `DISK_FULL` carries `backupPath` and
  `atBackupLocation` beside `path`, as `UPGRADE_FAILED` does._
  - The first refusal (`DATABASE_CLOSED`) leaves nothing to drop.
  - `BACKUP_LOCATION_UNAVAILABLE`, `INSUFFICIENT_SPACE` and
    `UPGRADE_BACKUP_FAILED` keep the database waiting.
  - Every other error drops it.
  - _Amended 2026-10-17: a sleep, screen lock or shutdown that comes while the
    slot is empty (during the open's key derivation, or while the backup's
    refusal is on its way back to the slot) still wins. The slot has a
    generation that dropping it raises; a database is put in the slot, or back
    in it, only if the generation is the one read before its key was derived
    or when it was taken out. Otherwise it is dropped: the open fails with
    `DATABASE_CLOSED`, and `upgrade_database` with `OPERATION_STOPPED`
    (research.md §4)._
  - _Amended 2026-10-17: when a sleep or shutdown stops an upgrade that then
    finishes, `OPERATION_STOPPED` is returned after this computer's open marker
    is cleared, so the database is upgraded, closed, and not shown as open
    here (003 FR-032, FR-040)._

### `cancel_upgrade` (FR-010, FR-016)

- **Input**: `{ awaitingId: number }` → **Output**: `null`.
- **While the question is up**, it drops the waiting database: the
  connection closes and nothing is written.
- **While its backup is being made**, it stops the `upgrade` operation, and
  `upgrade_database` then returns `OPERATION_STOPPED` with the partial file
  removed.
- **Once the upgrade has started**, it does nothing.
- An `awaitingId` that isn't current does nothing.

### `check_backup` (FR-019)

- **Input**: `{ backupPath: string; passphrase: string; databasePath?: string }`.
- **Output**: `{ upgradeTo: string | null }`: the version the backup will be
  upgraded for when restored, or `null` when it is in the current layout.
- **Errors**: as `restore_backup` gives them before it copies anything:
  - `NOT_FOUND`, `PASSPHRASE_INCORRECT`, `DATABASE_DAMAGED`;
  - `DATABASE_NEWER_VERSION`;
  - with `databasePath`, the damaged restore's authorization refusals (#62).
- **Reads only**: the backup is opened read-only and closed before the
  command returns, and the passphrase is wiped then (003 FR-007).
- **Scope**: without `databasePath` it requires the `HoploDex-Session`
  header naming the open session, as `restore_backup` does (008
  contracts/tauri-commands.md).

## Events (added and amended)

| Event | Payload | When |
|---|---|---|
| `upgrade:progress` (new) | `{ phase: "backingUp" \| "upgrading"; processed: number; total: number; showNow: boolean }` | First with nothing done, then after each 1 MiB of the backup. `upgrading` is sent once with `processed = total = 0` and again after each step. `showNow` uses the backup's estimate, as `backup:progress` does (003 SC-005). |
| `restore:progress` (amended) | phase also `"upgrading"` | while the restored copy is upgraded |

## Process modes and builds

No new process mode. An `e2e` build applies one extra step,
`9000_e2e_upgrade_check`, after `MIGRATIONS`, and reads
`HOPLODEX_E2E_UPGRADE_PHASE_SECONDS`, which holds each upgrade phase on
screen that long (research.md §14). No other build has either. The Rust
tests' test-only steps and fault points need the `upgrade-test-steps`
feature, which no shipped or E2E build compiles.
