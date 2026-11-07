# Startup and the platform

_Part of the [architecture docs](../architecture.md)._

## `main.rs`

`main.rs` registers plugins and state, and lists every command in
`generate_handler!`.

A new command must also go in `commands_list.rs`'s `COMMANDS` (which `build.rs`
turns into the app's ACL manifest) and `capabilities/default.json` as
`allow-<command>`, for the main web view only; `tests/acl_manifest_test.rs`
fails when the three differ.

It installs `allocator.rs`'s `Wiping` around `System` as the global allocator of
every process the executable runs, the helpers' included: each freed block (and
a reallocated block's old one) is zeroed first, through a memset the optimizer
can't drop, so no Rust buffer that held the passphrase or a record's text stays
in freed memory (WebKit's, GLib's and WebView2's own allocations are beyond it),
and on Windows a failed allocation ends the process with `TerminateProcess`,
since abort's fail-fast reaches Windows Error Reporting; a fallible allocation
that fails ends it too instead of returning an error (the import helper's
up-front reserve, `std::fs::read`'s `try_reserve_exact`, `read_to_end`'s
`try_reserve`), since a global allocator can't tell the two apart;
`tests/performance_test.rs` installs it too, so the budgets are timed with it.

On Linux it first makes a fatal signal (SIGSEGV, SIGBUS, SIGILL, SIGFPE,
SIGABRT, SIGTRAP) end the process at once with exit status 128 plus the signal
and no core (`platform/core_dumps.rs`), so the helpers and the probe are covered
too: apport, the kernel's core handler on Ubuntu 24.04 and 26.04
(`apport-core-dump-handler`, which their desktop images install rather than
systemd-coredump), reads a crashed process's core whatever its core limit
(#166).

In a release build it first makes a panic print only its location, never its
message (013 FR-027). It handles `--render-helper`, `--import-helper` and
`--webkit-sandbox-probe` before anything else, then starts `logging.rs`'s logger (the `log` macros to
standard error: our crates at `info`, the others at `warn`; nothing installed
one before 011, so the helpers' and probe's lines the docs call "the log" went
nowhere; the helpers log nothing and open no file, the parent logs for them; and
to the troubleshooting log's file, for HoploDex's own lines only, once `setup` has
read `machine.json` and can name the folder, with what came before held in memory
until then: 013 research.md §1, §2), on Linux and macOS
sets the core limit to 0, soft and hard, which every process it starts inherits,
WebKit's included on Linux (macOS's WebKit processes are launchd's), since a
core holds decrypted records and the passphrase (#166); inside the Flatpak,
where the portal starts the web process and it doesn't inherit that, each page
load of either web view lowers the web process's limit with `prlimit`
(`Builder::on_page_load`); on Windows first turns off Windows Error Reporting's
error box, heap and snapshot and makes a panic end the process with
`TerminateProcess` rather than abort's fail-fast, which Windows Error Reporting
reports to Microsoft (`services/crash_reports.rs`, which the helpers use too;
#166; a failed allocation is `allocator.rs`'s, above), and builds the main
window in `setup()` (so it has its own web view data folder; on Windows with a
WebView2 environment HoploDex builds itself, as the PDF surface's is, with
custom crash reporting on, so WebView2 sends no crash report to Microsoft, wry
having no setting for it; the Crashpad minidumps WebView2 still writes into both
web views' data folders are securely deleted with the decrypted document copies,
at startup, at every close and at exit, #166) rather than from
`tauri.conf.json`.

No database is open at startup: it manages `Session` and `MachineSettings` (with
the system keyring), sweeps unfinished backups, and starts the system-event
listener and the idle clock's 1 s tick.

Closing the window or quitting emits `app:quit-requested` so the frontend can
ask about unsaved changes, except that `request_quit` exits at once while no
database is open and the page hasn't yet said it listens
(`quit_listener_ready`), since nothing would answer the event and the app would
never exit; an OS shutdown or SIGTERM/SIGHUP/SIGINT closes at once, keeping
unsaved input as pending changes.

It clears decrypted document copies on exit, on those signals and at startup
(001 FR-035).

On Linux its exit handler first ends each web view's web process
(`terminate_web_process`, a SIGKILL from WebKit), because a WebKitGTK web
process before 2.54.1 that runs its own exit handlers can corrupt its heap and
crash (#166, WebKit bug 305909); a SIGKILL of the app itself skips that, as in
E2E.

On macOS `setup()` sets `app_menu.rs`'s application menu (Tauri's default with
the predefined About item replaced by one of id `about`), and `on_menu_event`
emits `app:about-requested` (no payload) for it; Linux and Windows set no menu
(010).

## `app_dirs.rs`

`app_dirs.rs` is where the startup and every command get the app's config and
cache directories and the documents and home folders. Don't call `app.path()`
for them directly: an E2E build (and a `test-kdf` one) takes them from the
sandbox's `HOPLODEX_E2E_*` variables instead of the OS, and won't start without
them, because Windows' known folders ignore `APPDATA` and the like.

It also names the web views' data folders (the main window's and the PDF
surface's, under the cache directory), so `WEBVIEW2_USER_DATA_FOLDER` is never
set.

## `platform/`

`platform/` turns the OS's sleep, wake, screen-lock and shutdown notices into
`SystemEvent`s for `session::lifecycle`.

`core_dumps.rs` keeps a crash from writing the app's memory to disk (#166): the
core limit (Linux, macOS), the core dump filter of 0 that the app's children
inherit, the fatal-signal handler and the Flatpak web process's limit (Linux).

The `.deb` also lists `/usr/bin/hoplodex` in apport's ignore list
(`/etc/apport/blacklist.d/hoplodex`, from `src-tauri/packaging/apport-ignore`),
which apport 2.28 and 2.34 read before they take a core; WebKit's own processes
aren't listed, since every WebKitGTK app shares them, so apport still writes a
report of one, but the filter keeps its memory out.

Inside the Flatpak the portal, not the app, starts the sandboxed web process, so
it doesn't inherit the filter and has only the lowered core limit; since the
portal is a user service, that process runs in the user's `user.slice`, where
apport writes no report of it, finding no `/run/apport.socket` in that sandbox
(apport 2.28 and 2.34).

`linux.rs` names logind's `session/auto` for the lock listeners, which logind
resolves from the caller's own credentials, so it is right inside a Flatpak
where a `GetSessionByPID` lookup would name another process (011).

## `window_controls/`

`window_controls/` (one file per OS) takes the browser out of the main window in
release and E2E builds only (`INSTALLED`; `tauri dev` keeps Inspect Element and
reload): a context-menu allowlist (WebKitGTK on Linux; WebView2's
`ContextMenuRequested` on Windows, which also turns off its browser accelerator
keys and swipe navigation; on macOS `willOpenMenu:` and `quickLookWithEvent:`
hooks that keep the system's AutoFill, an owner decision, and cancel a menu the
filter emptied).

`main.rs` applies it to `main` in `setup()`. Windows' navigation filter
(`services/main_navigation.rs`) acts only on document requests so the IPC custom
protocol still works.

`scripts/window-controls-check.sh` and its per-OS siblings check it with real
input (`docs/development/surface-checks.md`).
