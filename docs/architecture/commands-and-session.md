# Commands and the session

_Part of the [architecture docs](../architecture.md)._

## `commands/`

`commands/<area>.rs` holds each area's `#[tauri::command]` functions. They are
thin: they go through the `Session` state, then call the matching function in
that file's `pub mod ops`.

Every command that reaches the open database takes `session: ScopedSession`
(below), and collection commands use its `read(...)` or `write(...)`, which
refuse with `DATABASE_CLOSED` when nothing is open and
`PENDING_CHANGES_UNRESOLVED` while a lock's pending changes wait. `write` also
checks the file's fingerprint first, so another computer's take-over closes the
session without writing. Session housekeeping uses
`write_housekeeping`/`inspect`.

`ops` functions take `&Connection` and hold the actual logic. Integration tests
and `examples/human_seed.rs` call `ops` directly, so they exercise the same code
paths as the app.

`commands/troubleshooting_log.rs` (013) gets and sets the log's state and shows
its file: no database and no argument the web view can point elsewhere, since the
file is `logging.rs`'s.

`commands/databases.rs` (chooser, create, open, close, lock, settings, saved
passphrase) and `commands/backups.rs` (list, restore, passphrase change) are
feature 003's.

No command takes or returns a passphrase: `create_database`, `open_database`
(without `useSavedPassphrase`, or with `takeOver`, which the saved passphrase
never confirms), `save_passphrase`, `change_passphrase` and `check_backup` ask
for it in the native passphrase dialog (`services::passphrase_dialog`, in
[Models and services](models-and-services.md)) from `ops::*_typed`, which run on
a blocking thread (`spawn_blocking`), since the dialog waits for the user;
`restore_backup` takes the `checkId` that `check_backup` kept the passphrase
under, and `drop_backup_check` lets it go.

## `session/`

`session/` owns the open database (feature 003): `Session` holds at most one
`OpenDatabase`, a single `Connection` behind a `Mutex` with no pool,
deliberately. It holds the file's exclusive lock and SQLCipher's derived key,
never the passphrase.

It also holds `prompts` (`passphrase_dialog::Prompts`, the dialogs on the
screen, which `lifecycle::screen_locked`, `will_sleep` and `will_shut_down` take
down, and `Session::take` takes down those for the database it closes),
`backup_check` (one checked backup's passphrase, wiped when dropped, until the
restore's confirmation is answered; dropped as `prompts` are) and
`open_elsewhere` (the computer the last open, or an upgrade refused by
`upgrade.rs`, found a database marked open on, keyed by the path after
`Reach::check`, for the take-over dialog's words).

`lifecycle.rs` opens, closes (normal close with its automatic backup, lock,
immediate close on sleep/shutdown) and reacts to OS events; `operations.rs`
tracks the one long-running operation; `idle.rs` is the idle lock; `pending.rs`
stages the open form's unsaved input and writes it as pending changes on a lock;
`fingerprint.rs` detects take-over; `upgrade.rs` (009) answers an older layout:
`open_database` fails with `UPGRADE_NEEDED` and leaves the keyed, unwritten
database in `Session`'s `awaiting_upgrade` slot (separate from `OpenDatabase`,
so no collection command reaches it; one slot, dropped by any open, create or
restore, a quit, sleep, screen lock or shutdown), `upgrade_database` takes the
backup before the upgrade, runs `db::upgrade::upgrade` and installs the
database, and `cancel_upgrade` drops the slot or stops that backup.

Each open, unlock and restore gets a `SessionId` from a counter that only goes
up within a run (`Session::current_id()` reads it without the mutex; 0 is none;
a passphrase change keeps it), sent to the frontend as
`DatabaseStatus.sessionId` and in the `session:closing`/`session:closed` events.

`session/scoped.rs`'s `ScopedSession` is the `Session` state plus the id the
request named in its `HoploDex-Session` header (a missing or malformed one is
`DATABASE_CLOSED`): its `read`/`write`/`inspect` and the rest refuse a session
that isn't the open one, under the mutex, before anything runs, so a late call
from an ended session can't touch the next.

Only `UNSCOPED_COMMANDS` (`commands_list.rs`: the chooser's, `upgrade_database`,
`cancel_upgrade`, `get_database_status`, `quit_application`,
`quit_listener_ready`, `note_activity`, `set_idle_paused`, `skip_backup`,
`cancel_import`, the document-opening setting, `drop_backup_check`) take
`State<Session>`; a new command is scoped unless listed there, and
`tests/session_scope_test.rs` fails when a command's argument and the list
disagree.

`restore_backup`, `list_backups`, `check_backup` and `forget_saved_passphrase`
are unscoped too, because they take a `databasePath` with nothing open, but read
the header themselves when they act on the open database
(`session::scoped::Target`); `restore_backup` of an older layout fails with
`UPGRADE_NEEDED` unless sent `upgrade: true`.

`OpenDatabase.pending` is a `PendingState` (`None`, `Unresolved`, `Resuming`):
`resolve_pending_changes` takes `resume` (the row stays and collection commands
are served), `opened` (the form opened: deletes the row and keeps the draft as
the session's staged draft), `notOpened` (back to unresolved) or `discard`, and
a session that ends in either state leaves the row for the next open.
`change_passphrase` is refused in both, and `close_database` is served in every
state.
