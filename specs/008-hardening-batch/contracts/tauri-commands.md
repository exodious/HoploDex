# Contract: Tauri IPC Commands (feature 008 deltas)

A delta against the IPC contract as features 001 to 007 left it (003's
`contracts/tauri-commands.md` for the session, 006's for import, 007's for
the preview). Unchanged commands keep their input, output and errors. They
gain the session header below.

## Every request names its session (FR-004; research.md §2)

- **Header**: every command not in the unscoped list below is sent with
  `HoploDex-Session: <sessionId>`, where `sessionId` is the open session's
  `DatabaseStatus.sessionId`, as a decimal integer. The frontend sends it
  only through `SessionScope.invoke` (research.md §3).
- **The check**: the backend compares the header with the open database's
  session id under the session's lock, before anything else the command
  does. For `close_database` and `lock_database` the check is made where the
  close takes the open database from the session, so a late close or lock
  can't end the session opened after it. On a missing or malformed header, nothing open, or another session
  open:
  - **Error**: `DATABASE_CLOSED`;
  - nothing is read, changed or returned, of any database.
- **Unscoped commands** (`commands_list.rs`'s `UNSCOPED_COMMANDS`):
  - `get_chooser_state`, `create_database`, `open_database`,
    `remove_recent_database`, `locate_database`;
  - `get_database_status`;
  - `quit_application`, `note_activity`, `set_idle_paused`;
  - `skip_backup`, `cancel_import`;
  - `get_document_opening`, `set_document_opening`;
  - `restore_backup`, which requires the header only when it restores the
    open database (no `databasePath`). The header must name the open
    session; anything else is `DATABASE_CLOSED`.
- **Scoped**: everything else, including `close_database`,
  `lock_database`, `stage_pending_changes`, `resolve_pending_changes`,
  `dismiss_note`, `update_backup_settings`, `update_lock_settings`,
  `save_passphrase`, `forget_saved_passphrase`, `list_backups`,
  `delete_all_backups`, `change_passphrase`, and every collection, media,
  preview, mount, entry, insurance, import and export command.

`tests/session_scope_test.rs` fails when a command takes `State<Session>`
but isn't unscoped, or is unscoped but takes `ScopedSession`.

## Shared types (amended)

```ts
type DatabaseStatus = {
  // …unchanged fields…
  sessionId: number;   // NEW: this open's session id (research.md §1); never 0
};

type PendingSummary = {
  formVersion: number;
  kind: "firearm" | "policy" | "accessory";
  mode: "add" | "edit" | "dispose" | "restore" | "coverage";
  targetId: number | null;
  label: string;
  savedAt: string;
  resumable: boolean;            // the target still exists (unchanged)
  savedByVersion: string | null; // NEW: app_state.last_saved_version (FR-015, FR-016)
  appVersion: string;            // NEW: this build's version, e.g. "1.3.0"
};
```

## Commands (amended)

### `resolve_pending_changes` (amended: FR-003, research.md §6)

```ts
input:  { action: "resume" | "opened" | "notOpened" | "discard" }
output: { draft?: Draft; pending?: PendingSummary }
```

| Action | When | Effect | Output |
|---|---|---|---|
| `resume` | pending changes are unresolved | Moves to resuming; collection commands are served; **the row stays** | `{ draft }` |
| `opened` | resuming | The form has opened with the draft: deletes the row and keeps the draft as the session's staged draft, so a lock, sleep or shutdown before the form's first staging writes it back | `{}` |
| `notOpened` | resuming | The form couldn't open: collection commands are refused again (`PENDING_CHANGES_UNRESOLVED`) | `{ pending }`, for the dialog |
| `discard` | unresolved | Deletes the row (unchanged) | `{}` |

**Errors**:
- `VALIDATION_ERROR` for an action in the wrong state, with nothing
  changed;
- `DATABASE_CLOSED` as for every scoped command.

A session that ends while resuming leaves the row, and the next open reports
it in `pendingChanges`. Every action that deletes the row records the
last-saved version (FR-015).

### `close_database` (amended: FR-017)

Unchanged in shape. Served while pending changes are unresolved or
resuming: the normal close, with its automatic backup when due. Pending
changes are left exactly as they were, and no backup carries them.

### `import_collection` (amended: FR-006 to FR-009; research.md §8, §9, §11)

Input and `ImportResult` are unchanged. The backend now:
1. registers the `Import` operation (as before);
2. **reads the file or files in the import helper, without holding the
   session's lock**, emitting progress with `phase: "reading"`;
3. only then saves rows under the session's lock, as before
   (`phase: "importing"`).

**New and changed errors**, all before any row is saved:

| Code | Details | When |
|---|---|---|
| `IMPORT_LIMIT_EXCEEDED` | `{ file: string; sheet?: string; limit: ImportLimit }` | A file passed one of the limits (data-model.md "Import limits"). The message names the file, the sheet of a workbook, and the limit, for example `inventory.xlsx, sheet "Sheet3": more than 100,000 rows. HoploDex imports at most 100,000 rows per table.` |
| `VALIDATION_ERROR` | — | As before: the file couldn't be read (`<file>: could not read the file.`), now also when the helper died, passed its time limit or sent a malformed frame, and when a workbook's declared sizes don't match its content. No parser detail is shown. |
| `OPERATION_STOPPED` | `{ operation: "import", importedCount: 0 }` | `cancel_import` stopped the reading, and only that. Nothing was imported. |
| `DATABASE_CLOSED` | — | The session ended, or began an immediate close, while the file was read (a lock, screen lock, close, switch, take-over, sleep, shutdown or quit). The helper was killed and nothing was imported. The frontend's scope has ended, so nothing is shown. |

`ImportLimit` = `"fileSize" | "unpackedSize" | "zipEntries" | "sheets" |
"rows" | "columns" | "cells" | "cellText" | "totalText" | "sharedStrings"`.

Row-level errors are unchanged (FR-009), except that the free-text field
maximums (below) now give row errors.

### `create_firearm`, `update_firearm`, `dispose_firearm`, `create_accessory`, `update_accessory`, `dispose_accessory`, `create_insurance_policy`, `update_insurance_policy` (amended: research.md §10)

`VALIDATION_ERROR` with `fieldErrors[<field>] = "<Label> can be at most <n>
characters."` for a free-text field over its maximum (data-model.md
"Validation: free-text field maximums"). The import's rows get the same
message as a row error.

## Commands (added)

### `cancel_import`

- **Input**: none → **Output**: `null`. Unscoped.
- Stops the running import if it is still reading its files. The reading
  ends within 1 s (SC-004), and `import_collection` then fails with
  `OPERATION_STOPPED` (`importedCount: 0`).
- Does nothing if no import is running, or once rows are being saved: that
  phase is stopped only by a sleep, as before.

Registered in `generate_handler!`, `COMMANDS` and
`capabilities/default.json` (`allow-cancel_import`, main web view only), so
`acl_manifest_test.rs` holds.

## Events (amended)

| Event | Payload | Change |
|---|---|---|
| `session:closing` | `{ reason: CloseReason; sessionId: number }` | `sessionId` added: the frontend ends only that session's scope. |
| `session:closed` | `{ reason; databasePath; outcome?; stoppedOperation?; sessionId: number }` | `sessionId` added, as above. |
| `import_collection:progress` | `{ phase: "reading" \| "importing"; processed: number; total: number }` | `phase` added. In `reading`, bytes across the files. In `importing`, rows (as before). |

## Process modes (amended)

`main.rs` checks, before anything else:
- `--render-helper` (TIFF, 007, unchanged);
- `--webkit-sandbox-probe` (007, unchanged);
- **`--import-helper`** (new): the import helper (research.md §8). It reads
  frames on stdin and writes them on stdout, confined, with a 512 MiB data
  limit where the OS enforces one. It is never started by anything but
  `import_collection`.
