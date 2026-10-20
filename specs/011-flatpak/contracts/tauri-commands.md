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

_Amended 2026-10-19 (011 T078): the message is built in one place,
`CommandError::out_of_reach(path, folder, home, grants)`. Its first sentence
names the grants (each root, `(read only)` where it is) unless they are the
shipped four (the home folder, `/media`, `/run/media`, `/mnt`, all writable),
for which it says "your home folder and removable drives"; the override
command shell-quotes the folder when a shell would split it. `details` is
`{ path, folder, grants }` as above, `grants` always the roots
(data-model.md, "Error `LOCATION_OUT_OF_REACH`")._

_Amended 2026-10-19 (owner's decision; FR-003, FR-007): the shipped default is
three grants, `<Documents>/HoploDex`, `/media` and `/run/media`, and the
message's first sentence and its "move" advice name those
(data-model.md). `out_of_reach` takes which advice to give. Only
`add_photo_from_path` and `add_document_from_path` ask for the attach advice,
which adds: "To attach it from where it is, use Add photo (or Add document) and
choose it there." `details` are unchanged. The frontend's own availability
sentence (`src/features/databases/outOfReach.ts`) says "by default it can reach
only the HoploDex folder in your Documents folder and removable drives"._

_Amended 2026-10-19 (011 T118): the attach advice names each command's real
button, "Add photos" for `add_photo_from_path` and "Attach" for
`add_document_from_path`, in place of "Add photo (or Add document)"; see
data-model.md._

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
| `update_backup_settings` | `location.path` when `kind: "custom"` and it differs from the saved location (an unchanged one writes nothing, so saving the other settings isn't stopped) | `WriteFolder` | `VALIDATION_ERROR` with `fieldErrors.location` set to the out-of-reach message, as other unusable locations are; for `move`, `BACKUP_LOCATION_UNAVAILABLE` with `reason: "outOfReach"` |
| `close_database` (its backup), and the "before" backups of `restore_backup`, `change_passphrase`, `upgrade_database` | the database's backup folder when custom | `WriteFolder` | the existing failed-backup outcomes with `failureReason: "outOfReach"`; a "before" backup that can't be made stops its operation as it already does, with `BACKUP_LOCATION_UNAVAILABLE` and `reason: "outOfReach"` |
| `list_backups` | the backup folder | `WriteFolder` (for `available`) | not an error: `available: false`, `unavailableReason: "outOfReach"` |
| `restore_backup`, `check_backup` | the backup file | `ReadFile` | `LOCATION_OUT_OF_REACH` (`BackupCheck` has no `available`, so `check_backup` has no folder to report) |
| `restore_backup` with nothing open | `databasePath` | `DatabaseFolder` | `LOCATION_OUT_OF_REACH`, before the "before" backup or any scratch file |
| `export_collection` | the destination folder | `WriteFolder` | `LOCATION_OUT_OF_REACH` |
| `read_import_files`, `import_collection` | each file | `ReadFile` | `LOCATION_OUT_OF_REACH`, before the import helper starts |
| `add_photo_from_path`, `add_document_from_path` (the chooser and drops) | the file | `ReadFile` | `LOCATION_OUT_OF_REACH`; for a drop, the record page's existing drop-error toast shows its message |

_Amended 2026-10-19 (011 T078): the one place a backup folder is judged,
`services/backups.rs`'s `check_location(folder, make_if_missing, &Reach)`, asks
`Need::WriteFolder` before it makes the folder or writes its probe file, and
answers `LocationProblem::OutOfReach`, which every row above that names a
backup folder maps as written. Commands take `State<Arc<Reach>>` (or reach it
through `Session::reach()`) and hand `&Reach` to their `ops`._

_Amended 2026-10-19 (011 T107): `change_passphrase` makes no backup before it
re-keys the file (`commands/backups.rs`, `ops::change_passphrase` writes a
re-keyed copy beside the database and swaps it in; it only marks a change as
waiting to be backed up, 003 FR-025), so it has no backup folder to check and
the row above does not apply to it: it asks `Reach` nothing, and a backup
location out of reach does not stop it
(`sandbox_reach_test.rs`,
`change_passphrase_is_not_stopped_by_a_backup_location_out_of_reach`). Only
`close_database`'s backup and the "before" backups of `restore_backup` and
`upgrade_database` check the backup folder._

_Amended 2026-10-19 (011 T096, T097): `read_import_files`, `import_collection`,
`add_photo_from_path`, `add_document_from_path`, `restore_backup` and
`check_backup` (`ReadFile`) accept a path that is not a Documents-portal one only
where a grant covers it, so a drop from outside reach is refused whether or not
the file shows up in the sandbox. Every row above whose `Need` is `WriteFolder`
and names a stored backup folder (`update_backup_settings`, `close_database`'s
backup, the "before" backups, `list_backups`) asks `BackupFolder` instead
(data-model.md, "`Need`"): a Documents-portal path outside reach is refused there,
with the override advice for its host folder, and is kept for the one-off writes,
`export_collection`'s destination and `upgrade_database`'s chosen folder, which
stay `WriteFolder` (a chosen folder that is the database's backup location itself
is the stored one)._

`change_passphrase` and `restore_backup`'s replacement of the open database
act on its own path, which `open_database` already found within reach in
this run, so they check only the backup folder above. With nothing open,
`restore_backup` acts on the `databasePath` it is given, whose reach may
have changed since that database was last open (an override reset between
runs), so it checks that path too.

_Amended 2026-10-19 (011 T107): for `change_passphrase` the paragraph above
holds only as far as the path: it checks no backup folder, because it makes
no backup (see the note before it)._

## `open_document` (amended inside Flatpak only)

Input, output and errors unchanged. Inside a Flatpak, the copy is handed to
the OpenURI portal instead of `xdg-open` (research.md §8). Its answers map as
data-model.md's table, so `NO_APP_FOR_DOCUMENT` keeps its meaning: the
portal said no application could open it, or there is no portal.

_Amended 2026-10-19 (011 T078): the route is `AppOpener`'s
`OpenerRoute::Portal { timeout }`, chosen once at startup from
`Reach::is_flatpak()` and passed to `AppOpener::new(app, route)`; outside a
Flatpak it is `OpenerRoute::XdgOpen`, as before (data-model.md, "The portal
opener")._

## Unchanged

- `get_database_status`: no new field. The guide's Linux paragraph is chosen
  by the frontend's platform, not by the backend (research.md §11).
- Every scoped command's session rules (008), every event.
- The capabilities and ACL manifest: no command added, so
  `commands_list.rs`, `capabilities/default.json` and `main.rs`'s
  `generate_handler!` don't change.
