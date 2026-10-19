# Contract: Tauri IPC Commands (feature 011 deltas)

A delta against the IPC contract as features 001 to 010 left it (003's
`contracts/tauri-commands.md` for databases and backups, 007's for
documents, 008's for session scoping, 009's for upgrades). No command is
added or removed, no input changes, and no command's success output changes.
Outside a Flatpak nothing here can happen: `Reach` is `Unconfined` and every
check passes (data-model.md).

## Errors (added and amended)

| Code | `details` | When |
|---|---|---|
| `LOCATION_OUT_OF_REACH` *(new)* | `{ path, folder, grants: string[] }` | Inside a Flatpak, a location the operation needs is outside what this copy may reach (FR-004, research.md §5). Returned before anything is written: no scratch file, probe, backup or partial file exists afterwards. The message is data-model.md's, naming the location, what the Flatpak can reach, and the two fixes (move it, or `flatpak override` and restart). |
| `BACKUP_LOCATION_UNAVAILABLE` *(amended)* | `reason` gains `"outOfReach"` | The custom backup location is outside reach. Same message parts as above, for the backup folder. |

`LOCATION_OUT_OF_REACH` is never overridable (`confirmedWarnings` has no
effect on it).

## Shared types (amended)

```ts
type BackupLocation = {             // models/database.rs; in the collection's settings
  kind: "default" | "custom";
  path: string;
  available: boolean;
  unavailableReason?: "outOfReach"; // NEW: set only when available is false because of reach
};

type RecentDatabase = {             // get_chooser_state's recent list
  // …unchanged fields…
  available: boolean;
  unavailableReason?: "outOfReach"; // NEW: the file's folder is out of reach, so the row
                                    // says so instead of "not found"
};

type BackupList = {                 // list_backups
  folder: string;
  available: boolean;
  unavailableReason?: "outOfReach"; // NEW
  backups: BackupInfo[];
};

type CloseOutcome = {
  backup: "made" | "notDue" | "alreadyToday" | "off" | "skipped" | "failed" | "notAttempted";
  failureReason?:
    | "locationUnavailable" | "insufficientSpace" | "io" | "databaseUnreachable"
    | "outOfReach";                 // NEW: the custom backup location is out of reach
};
```

## Commands that check reach

Each command checks before it writes anything, with the `Need` of
data-model.md. A Documents-portal path whose host path is within reach is
used as that host path: what is stored (the recent entry, the backup
location) is the host path.

| Command | Location checked | `Need` | Refusal |
|---|---|---|---|
| `create_database` | `folder` + `name` | `DatabaseFolder` | `LOCATION_OUT_OF_REACH` with `fieldErrors.folder` (the create dialog's existing field-error path puts it on the folder field) |
| `open_database` | `path` (from Open… or a recent entry) | `DatabaseFolder` | `LOCATION_OUT_OF_REACH`; the chooser's open-failure line shows its message (its `default` case) |
| `locate_database` | the new `path` | `DatabaseFolder` | `LOCATION_OUT_OF_REACH` |
| `update_backup_settings` | `location.path` when `kind: "custom"` | `WriteFolder` | `VALIDATION_ERROR` with `fieldErrors.location` set to the out-of-reach message, as other unusable locations are; for `move`, `BACKUP_LOCATION_UNAVAILABLE` with `reason: "outOfReach"` |
| `close_database` (its backup), and the "before" backups of `restore_backup`, `change_passphrase`, `upgrade_database` | the database's backup folder when custom | `WriteFolder` | the existing failed-backup outcomes with `failureReason: "outOfReach"`; a "before" backup that can't be made stops its operation as it already does, with `BACKUP_LOCATION_UNAVAILABLE` and `reason: "outOfReach"` |
| `list_backups`, `check_backup` | the backup folder | `WriteFolder` (for `available`) | not an error: `available: false`, `unavailableReason: "outOfReach"` |
| `restore_backup` | the backup file | `ReadFile` | `LOCATION_OUT_OF_REACH` |
| `restore_backup` with nothing open | `databasePath` | `DatabaseFolder` | `LOCATION_OUT_OF_REACH`, before the "before" backup or any scratch file |
| `export_collection` | the destination folder | `WriteFolder` | `LOCATION_OUT_OF_REACH` |
| `read_import_files`, `import_collection` | each file | `ReadFile` | `LOCATION_OUT_OF_REACH`, before the import helper starts |
| `add_photo_from_path`, `add_document_from_path` (the chooser and drops) | the file | `ReadFile` | `LOCATION_OUT_OF_REACH`; for a drop, the record page's existing drop-error toast shows its message |

`change_passphrase` and `restore_backup`'s replacement of the open database
act on its own path, which `open_database` already found within reach in
this run, so they check only the backup folder above. With nothing open,
`restore_backup` acts on the `databasePath` it is given, whose reach may
have changed since that database was last open (an override reset between
runs), so it checks that path too.

## `open_document` (amended inside Flatpak only)

Input, output and errors unchanged. Inside a Flatpak, the copy is handed to
the OpenURI portal instead of `xdg-open` (research.md §8). Its answers map as
data-model.md's table does, so `NO_APP_FOR_DOCUMENT` keeps its meaning: the
portal said no application could open it, or there is no portal.

## Unchanged

- `get_database_status`: no new field. The guide's Linux paragraph is chosen
  by the frontend's platform, not by the backend (research.md §11).
- Every scoped command's session rules (008), every event.
- The capabilities and ACL manifest: no command added, so
  `commands_list.rs`, `capabilities/default.json` and `main.rs`'s
  `generate_handler!` don't change.
