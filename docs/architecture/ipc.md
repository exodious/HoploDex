# IPC boundary

_Part of the [architecture docs](../architecture.md)._

**IPC boundary.** The frontend reaches the backend only through `invoke()` in
`src/services/tauriClient.ts`. It turns a backend rejection into a typed
`CommandFailure { code, message, fieldErrors }`, which mirrors `CommandError` in
`src-tauri/src/commands/error.rs`. That is the only error shape a command
returns.

Codes are stable strings (`VALIDATION_ERROR`, `NOT_FOUND`, `INTERNAL_ERROR`,
domain codes such as `ORIGINAL_MARKS_MATCH` and `IMPORT_LIMIT_EXCEEDED`, and the
session's `DATABASE_CLOSED`, `DATABASE_TAKEN_OVER`, `PENDING_CHANGES_UNRESOLVED`
and friends, `DISK_FULL` (a write ran out of room: SQLite's `SQLITE_FULL` in
`CommandError::from_db`, or an I/O error `services::disk_space::is_disk_full`
recognises via `CommandError::from_io`; the session adds the database's path
with `located_at`; unlike `INSUFFICIENT_SPACE`, which is a pre-check's refusal
with sizes),

- and for 007's preview `PREVIEW_UNSUPPORTED`, `PDF_PREVIEW_UNAVAILABLE`,
  `PREVIEW_DAMAGED`, `PREVIEW_FAILED`, `PREVIEW_PAGE_FAILED`, `PREVIEW_CLOSED`
  and `NO_APP_FOR_DOCUMENT`,
- and for 009's upgrade `UPGRADE_NEEDED` (an open or a restore of an older
  layout; an open's carries the `awaitingId` to answer), `UPGRADE_FAILED`
  (rolled back, the backup made before it kept) and `UPGRADE_BACKUP_FAILED`
  (that backup couldn't be written, so nothing was upgraded),
- and for 011's Flatpak `LOCATION_OUT_OF_REACH` (a location outside what the
  Flatpak may reach, refused before anything is written and never overridable by
  `confirmedWarnings`; `CommandError::out_of_reach(path, folder, documents,
  grants, advice)` is the one place its wording is built: it names the grants
  unless they are the shipped default of three (`<Documents>/HoploDex`,
  `/media`, `/run/media`, each writable), gives the `flatpak override --user
  --filesystem=<folder>` command with the folder shell-quoted, and, for
  `add_photo_from_path` and `add_document_from_path` (`Need::AttachPhoto`,
  `Need::AttachDocument`), says to attach the file with its button; `details`
  are `path`, `folder` and `grants`).

`BACKUP_LOCATION_UNAVAILABLE` gains the reason `outOfReach` beside `missing`,
`notWritable` and `insufficientSpace`, and the three listings that say whether a
location is usable (`RecentDatabase`, the backup settings' folders and the
backups list) carry `unavailableReason: "outOfReach"` when `available` is false
for that reason alone).

Raw DB errors are logged and never forwarded (`CommandError::from_db`).

Some warnings can be overridden: the command fails with a warning code, and the
UI resends it with `confirmedWarnings: true`.
