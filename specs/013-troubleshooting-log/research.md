# Research: Troubleshooting Log

## §1 One function decides

`logging::decide(version, saved, variable)` returns what applies at start, including whether to delete leftovers. **Decision:** the default is `major == 0`, read from `CARGO_PKG_VERSION`. **Why:** the owner wants the 1.0.0 change impossible to forget, and a value worked out from the version has no step to miss. **Rejected:** a constant plus a failing test, which adds a release step.

A raise never saves anything, so a user following the default doesn't become opted in by using the button, and at 1.0.0 they still go off.

## §2 The saved choice is read late, so early lines wait

`logging::init()` runs first in `main()`, before Tauri can name a folder or `machine.json` is loaded. **Decision:** it reads `HOPLODEX_LOG` at once, holds up to 256 lines in memory, and `logging::start` either writes them to the file or drops them. **Rejected:** starting the logger later, which would lose the sandbox probe's and core-dump lines from stderr too.

A run the variable decides needs no saved choice, so its file starts first in `setup` (`logging::start_for_variable`), and a run started for a problem at startup keeps its lines if `setup` then fails. Any other run waits for the choice, because a start on a guess could delete a saved On's files at 1.0.0. Tauri panics with a failed `setup`'s error, and a release build's panic gives only its location, so `main()` logs the reason first (`logging::startup_failed`), on standard error if there is no file yet.

## §3 Location

**Decision:** `app_dirs::cache_dir(app)/logs/hoplodex.log`, so an E2E build is already in its sandbox and no new path logic exists. Backups and exports copy named files, so neither reaches this folder. `Library/Caches` is excluded from Time Machine (checked on a macOS 27 machine: `tmutil isexcluded`).

## §4 Creating, appending, rotating

- **Create or append:** the file is created with `create_new` and mode 0600, in a 0700 folder on Unix. An existing one must pass `scratch::is_plain_file`, and is opened with `scratch::open_no_follow`. Anything else leaves the file log off for the run and says so in About. Either way the handle appends, so two copies of the app add whole lines rather than overwrite each other's.
- **Rotate:** at 2 MiB by the open file's own length, which counts another copy's lines and is at the cap for a copy whose file was rotated away. `rename` to `.1` replaces a symlink rather than following it. A current file that has gone leaves nothing to rename, and a new one starts.
- **Delete:** `secure_delete::secure_delete_file`, the way the app deletes other files it no longer wants. It logs, so the logger's lock is never held across it.
- **Age:** when the file was made, or its last change where the file system does not record that, at start only (013 FR-008). A new file on Windows sets its own creation time, since NTFS can give a reused name the old one.

## §5 The shape of a path

`logging::place(path)` gives the root and depth, nothing else. Roots kept: `~`, `C:`-style drive letters, `/run/media`, `/media`, `/mnt`, `/Volumes`, `/run/user/*/doc` (the Flatpak's document portal). A UNC prefix becomes `\\*\*`. Every other component is `*`. The last keeps its extension only if it is one of the app's own kinds of file: `hoplodex`, `json`, `log`, `partial`, `new`, `old` or `bad`. This replaces `folder_of`, whose job it does more strictly.

**Why:** the owner's "sensitive" is wider than the old rule, and a path can carry a user name and a database's name. What a path usually tells us in a report is local or removable and how deep, which survives. A database called `Guns.hoplodex` under home is `~/*.hoplodex`.

## §6 The rest of the rule, on every output

The logger only sees formatted text, so two versions of a line would need every call site to say more. **Decision:** one rule for all outputs. Lines change as follows:

- 64 calls move to `place`.
- `reach.describe()` uses `place`, so the Flatpak's reach line reads `[~/*/*, /media, /run/media]`; `flatpak.e2e.ts` and `scripts/flatpak/check.sh` follow.
- `commands/backups.rs` row counts become "the counts differ"; `db/mod.rs`'s integrity text is dropped; `db/upgrade.rs` logs each table with a broken link once, with no row id or count; opened-document copies that can't be deleted get one line, not one each; `services/sandbox.rs` stops quoting a raw entry.
- `logging::db_error` gives a `rusqlite::Error`'s kind and SQLite result code without its text. `lifecycle.rs` logs a `CommandError`'s `code`, not its `message`, which can name a file.
- `clean` replaces the home folder's path in a line and escapes control characters, as a backstop for OS error text. It can't catch everything, which is why the wording says "designed to leave out".

A panic's message follows the same rule. The file gets the location alone in every build. A release build replaces Rust's default hook first in `main()` (`logging::panic_location_only`), so standard error and the journal get the location alone too, and the hooks installed after it, Windows' `end_process_on_panic` included, still run. A debug build's standard error keeps the message for development.

## §7 Library lines stay out of the file

`tracing` is built without its `log` feature (`cargo tree -e features -i tracing`), so `zbus`, `oo7` and `wry`'s `tracing` events never reach `log`. Tauri, tao and rfd do use `log`, and some of their lines quote URLs and paths. **Decision (owner):** the file takes HoploDex's targets only (`hoplodex`, `hoplodex::*`, `hoplodex_*`). What it loses is in the spec's "What this leaves out".

## §8 Volume and cost

A line costs one `write` on an unbuffered file behind a mutex. No debug line goes in search, in a per-row loop or in the 1 s idle tick. Measured: the journey in `tests/log_contents_test.rs` (create, export, import, close with a backup, failed opens, a passphrase change, a restore, a lock) wrote 24 lines and 1.2 KiB at the raised level, about 50 bytes a line. So 2 MiB holds about 40,000 such lines, and a file reaches the cap only over many runs or a failure that repeats.

## §9 Accepted risks (for `docs/attack-surface.md`)

- **The file is plain text, readable by the same OS user** and anything running as them. It relies on what is left out, not on protecting it.
- **OS error text is written as it comes**, with only the home folder replaced. It could name something the backstop doesn't catch.
- **A compromised main web view can turn the log on or off and raise the level.** It gets no read of the contents; the cap limits the disk use; the lines it can provoke are all HoploDex's own.
- **The tests cover the paths they walk and the patterns they match**, which is why the user-facing wording says "designed to leave out" (owner's decision).

## §10 Show log file

`tauri_plugin_opener::reveal_item_in_dir` covers macOS and Windows, and Linux outside the Flatpak through `FileManager1.ShowItems`, with the parent folder through `xdg-open` where no file manager answers. Its Linux fallback to the portal calls `OpenDirectory` with a URI on the wrong interface, so inside the Flatpak the app uses its own: `OpenURI.OpenDirectory` with a read-only descriptor of the log file, on the connection `AppOpener` already holds. `Opener` gains `reveal`; `E2eOpener` records it in the sandbox. The path comes from Rust: the command takes no argument.
