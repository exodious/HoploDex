# HoploDex attack surface

This is the one definition of the application's attack surface (constitution,
"Development Workflow & Quality Gates"). The release security review reads it
at the release commit, and a per-pull-request AI review reads it with the
diff. Each entry says what to review for one surface:

- **Code:** the files to read, as backticked paths or globs from the
  repository root (`*` matches within a folder, `**` across folders). A note in
  parentheses says what in a path matters, and holds no path.
- **Ask:** the questions the review must answer.
- **Accepted:** residual risks the project has accepted, which the review
  reassesses, each linked to where it was accepted.
- **Rerun:** checks the review can't make by reading, run again before a
  release.
- **Since:** what added the surface.

Rules for the list:

1. **The list never carries an open finding.** What a review finds goes to
   code scanning, or to a private security advisory when it is in code a
   release already contains. An entry names what to review, never an unfixed
   weakness: "X lacks Y" is a finding, not an **Ask**.
2. **A risk enters Accepted only once it has been accepted**, with a link to
   where that was recorded: a feature's `research.md` section, or a code
   scanning alert dismissed with its reason.
3. **Surface ids are permanent.** The id is the heading's first word
   (`ipc-commands`). Code scanning alert rule ids start with it, so an id is
   never renamed or reused: change the title instead. A surface is removed only
   when the code it describes is gone, with the reason in the pull request.
4. **A pull request that adds, changes or removes surface updates this file in
   the same pull request:** a new entry, or a new **Code** path, **Ask**
   question or **Accepted** risk in an existing one, and fixes for paths it
   renamed or removed.

`scripts/attack-surface.mjs` (run by `npm test`) fails when a **Code** path or
glob matches nothing, when a production file (the backend, its build and
configuration, the workflows, the Flatpak and the build scripts) is claimed by
no entry, `not-surface` included, or when a command in `UNSCOPED_COMMANDS` or
a Cargo feature is named nowhere here. It can't see new surface inside a file
an entry already claims, such as a new argument or parser: the per-PR review
and peer review catch that.

## Threat model

- A compromised main web view.
- Hostile files the user brings in: spreadsheets, documents, photos and
  databases from elsewhere.
- Other local processes running as the same user, where the app claims to
  resist them (the WebDriver, the helpers, the Flatpak).
- Anything that can change what ships: CI, the build and dev servers.

## ipc-commands: what the web view can call

- **Code:** `src-tauri/src/main.rs` (`generate_handler!`); `src-tauri/src/lib.rs`; `src-tauri/src/commands_list.rs` (`COMMANDS`, `UNSCOPED_COMMANDS`); `src-tauri/build.rs` (the ACL manifest, generated into `permissions/`); `src-tauri/capabilities/**`; `src-tauri/src/commands/**`; `src-tauri/src/models/**`; `src-tauri/src/session/scoped.rs`; `src-tauri/src/session/operations.rs`; `src-tauri/src/services/mod.rs`; `src-tauri/src/services/cartridges/**`; `src-tauri/src/services/disk_space.rs`; `src-tauri/src/services/entry_text.rs`; `src-tauri/src/services/insurance_status.rs`; `src-tauri/src/services/mounts.rs`; `src-tauri/src/services/registration.rs`; `src-tauri/src/services/valuation.rs`; `src/services/tauriClient.ts`; `src-tauri/tests/acl_manifest_test.rs`; `src-tauri/tests/session_scope_test.rs`
- **Ask:**
  - Do the three command lists (`generate_handler!`, `COMMANDS`, the capabilities) agree, and does every `allow-<command>` belong only to the `main` web view, never the PDF surface? Is `core:default` or `dialog:default` broader than the app needs? Does the opener plugin have no permission at all?
  - For every command: is each argument checked in Rust (length, range, enum, an id belonging to the open session) as if the web view were hostile?
  - Is `UNSCOPED_COMMANDS` minimal? It holds `get_chooser_state`, `create_database`, `open_database`, `upgrade_database`, `cancel_upgrade`, `remove_recent_database`, `locate_database`, `get_database_status`, `quit_application`, `quit_listener_ready`, `note_activity`, `set_idle_paused`, `skip_backup`, `cancel_import`, `get_document_opening`, `set_document_opening`, `get_troubleshooting_log`, `set_troubleshooting_log`, `show_troubleshooting_log`, `restore_backup`, `check_backup`, `drop_backup_check`, `list_backups` and `forget_saved_passphrase`. Does `quit_listener_ready` only ever raise the flag that says the page listens for a quit, which `request_quit` acts on only while no database is open? Does `drop_backup_check` only ever drop a held check, by its id, and does `restore_backup` restore only the backup its `checkId` held, into the target that check was for? Do the ones that can act on the open database (`restore_backup`, `list_backups`, `check_backup`, `forget_saved_passphrase`) check `session::scoped::Target` themselves?
  - Can `set_document_opening` (unscoped) turn on opening without asking without `services::consent`'s native dialog?
  - Do the troubleshooting log's three commands (unscoped) touch no database, return a state and a path but never the file's contents, and leave `show_troubleshooting_log` no choice of what it shows? The file itself is `logging`'s.
  - Can `confirmedWarnings` skip a check that protects data? (`LOCATION_OUT_OF_REACH` must never be overridable.)
  - Do errors forward only `CommandError` codes and never a raw database error (`CommandError::from_db`)?
  - Regression: #64 (import conflicts bound to the database that made them).
- **Since:** 001; ACL manifest 007; scoping 008; three unscoped commands 009.

## webview-config: CSP, navigation and browser controls

- **Code:** `src-tauri/tauri.conf.json` (`app.security.csp`); `src-tauri/tests/csp_test.rs`; `src-tauri/src/main.rs` (the main window, built in `setup()`); `src-tauri/src/app_menu.rs`; `src-tauri/src/services/main_navigation.rs`; `src-tauri/src/services/crash_reports.rs` (`web_view_environment`, `main_browser_args`, `WRY_DEFAULT_ARGS`); `src-tauri/src/window_controls/**`; `src/features/app/browserControls.ts`; `src/main.tsx`; `src-tauri/examples/window_controls_check.rs`; `scripts/window-controls-check.sh`; `scripts/macos/window-controls-check.sh`; `scripts/windows/window-controls-check.ps1`
- **Ask:**
  - Is there no `unsafe-inline` or `unsafe-eval` for scripts? The CSP allows `asset:` and `http://asset.localhost` in `img-src`, but the asset protocol is off (no `assetProtocol`, no `protocol-asset` feature, no `convertFileSrc`). Should those sources go?
  - Can any top-level navigation or new window leave the app's origin (#73)? Does Windows' filter act only on document requests, so IPC works and nothing else loads?
  - In release builds, are dev tools, reload, history, print, save, find and view-source off, and is only the context-menu allowlist left?
  - Does `src/` have no HTML sink (`dangerouslySetInnerHTML`, `innerHTML`, `eval`, `new Function`) outside tests? Is everything the page shows from a database, a spreadsheet, a file name or `notices.json` rendered as text?
  - On Windows, the main window's WebView2 environment is HoploDex's own (`crash_reports::web_view_environment`): is it the one the main window actually uses, with its own data folder, and are its browser arguments (`main_browser_args`) only wry's defaults and the autoplay policy, with nothing that loosens the web view (a debugging port, a switch that turns off a protection)?
- **Accepted** ([008 research.md §22](../specs/008-hardening-batch/research.md)):
  - macOS's menu hook may miss items AppKit adds later (Writing Tools, Services).
  - AutoFill is kept (owner's decision).
  - Look-ups without a menu are verified only by manual check M1.
  - On Linux and Windows, only the page's guard stops the mouse's side buttons.
  - Linux's `Custom` spelling items are unverified.
- **Rerun:** the window controls check on each OS.
- **Since:** 001; CSP and navigation fixes #87; browser controls 008.

## paths: filesystem locations from the web view

- **Code:** `src-tauri/src/commands/databases.rs`; `src-tauri/src/commands/backups.rs`; `src-tauri/src/commands/import_export.rs`; `src-tauri/src/commands/photos.rs`; `src-tauri/src/commands/documents.rs`; `src-tauri/src/services/scratch.rs`; `src-tauri/src/services/file_swap.rs`; `src-tauri/src/services/backups.rs`; `src-tauri/src/services/attachments.rs`; `src-tauri/src/services/sandbox.rs`; `src-tauri/src/app_dirs.rs`; `src/features/media/useFileDrop.ts`
- **Ask:**
  - For each command that takes a location from the web view (`create_database`, `open_database`, `upgrade_database` for its backup folder, `locate_database`, `remove_recent_database`, `forget_saved_passphrase`, `update_backup_settings`, `list_backups`, `check_backup`, `restore_backup`, `export_collection`, `import_collection`, `add_photo_from_path`, `add_document_from_path`): does it accept only what its purpose needs (absolute; an existing directory or a regular file)? Where it writes, does it create exclusively and follow no symlink or reparse point?
  - Is a target the web view names authorized before it is replaced or removed (#62)? Are only files the app created recovered or removed (#63)?
  - Is a name from inside a database or a spreadsheet ever joined to a path (#68)? Does a colliding export leave no stale photos (#74)?
  - A database from another computer carries `collection_settings.backup_location`. Is that path checked like one the user typed before backups are written there?
  - Does every location go through `Reach::check` with the right `Need` before anything is written (011)?
  - Regression: #62, #63, #68, #74.
- **Since:** 001, 003; Flatpak reach 011.

## import-export: spreadsheet import and export

- **Code:** `src-tauri/src/services/import_reader/**`; `src-tauri/src/services/spreadsheet.rs`; `src-tauri/src/services/import_matching.rs`; `src-tauri/src/services/suggestions.rs`; `src-tauri/src/commands/import_export.rs`; `src-tauri/src/models/rules.rs`; `src-tauri/src/services/record_id.rs`
- **Ask:**
  - Is all parsing in `--import-helper` and none in the app?
  - Does the parent check every frame the child sends (sizes, counts, UTF-8, limits) as if the child were compromised?
  - Are the maximums in `import_reader/limits.rs` measured by inflating and enforced before any row is saved (#66)?
  - Are imported amounts and dates bounded so the collection's totals can't overflow (#67, `saturating_total`)?
  - Can a crafted sheet's `record_id`s (009) collide with existing records, overwrite them, or bind a mount to the wrong record?
  - Export: are CSV cells guarded against formula injection (`FORMULA_TRIGGERS` in `spreadsheet.rs`)? Are XLSX cells written as text, never as formulas? Does a cell over 32,767 characters still export (#95)?
  - Regression: #64, #66, #67, #68, #74.
- **Accepted** ([008 research.md §22](../specs/008-hardening-batch/research.md)):
  - The parent's memory bound rests on `MAX_CELLS` and the total-text limit together.
  - A collection at every field's maximum in 3- or 4-byte characters passes the 256 MB total-text limit.
- **Rerun:** `import_largest_test` in release (`docs/development/testing.md`). Open: #126 (the zip-bomb memory check fails under plain `cargo test`).
- **Since:** 001; helper and limits 008; identifiers 009.

## key-passphrase: the database key, the passphrase and saved passphrases

- **Code:** `src-tauri/src/db/cipher.rs` (`key`: every connection and the rekey attachment keyed through `sqlite3_key_v2`); `src-tauri/src/db/mod.rs` (`configure`'s keying, `check_copy`, `probe_first_page`); `src-tauri/src/allocator.rs`; `src-tauri/src/services/passphrase.rs`; `src-tauri/src/services/passphrase_dialog/**`; `src-tauri/src/services/keyring.rs`; `src-tauri/src/services/keyring/**`; `src-tauri/src/session/mod.rs`; `src-tauri/src/session/upgrade.rs`; `src-tauri/src/commands/databases.rs`; `src-tauri/src/commands/backups.rs`; `src-tauri/tauri.conf.json` (`bundle.macOS`: `signingIdentity`, `hardenedRuntime`)
- **Ask:**
  - Are the cipher settings pinned and unchanged (`CIPHER_SETTINGS`, `src-tauri/tests/portability_test.rs`)?
  - Is every database keyed through `cipher::key` (`sqlite3_key_v2`), never `PRAGMA key` or `ATTACH … KEY` with the passphrase, so it never becomes SQL text?
  - Is a passphrase typed only into the native dialog (`services::passphrase_dialog`: GTK, AppKit's `NSSecureTextField`, Win32 `ES_PASSWORD`), never into the page, and is no passphrase an argument or result of any command? Is every word of the dialog built in Rust, so the page chooses at most which database or backup it names? Is each name that reaches it from the page or a file (a database's or backup's file name, the machine name an open marker carries) cleaned and cut as `consent::display_name` does, and set as plain text, never markup?
  - Is the dialog's text copied once per press into a `Passphrase` (NFC, zeroized, never logged), and are the checks' reads borrowed (GTK) or wiped straight after (AppKit, Win32)? Is every field emptied when the dialog closes? What does each OS's toolkit still keep of what was typed (GTK's text layout of a shown passphrase; Win32's own buffers; AppKit's: reading `stringValue()` of the field being edited, at every change, pause and press and when Show is toggled, makes AppKit commit the field editor's text into the cell as a new `NSString`, and those copies are freed unwiped, though our own buffer is still wiped straight after), and what does SQLCipher leave of it in the app's heap?
  - Is every Rust block it passes through (the dialog's copies and `checks`' NFC copies, zxcvbn's scoring, a saved passphrase's D-Bus messages and zvariant values on Linux and oo7's decrypted keyring file in the Flatpak) zeroed when freed, by the one global allocator `main.rs` installs (`allocator.rs`)? What it can't reach (the toolkits' buffers, SQLCipher's, the OS keyring's on macOS and Windows) is the Ask above's.
  - Does the session hold only the derived key, never the passphrase, except one being saved until its upgrade ends (009 research.md §4) and a checked backup's until its restore (`backup_check`, dropped at Cancel, a screen lock, a sleep, a shutdown or a close)?
  - Does a screen lock, a sleep or the close of the database a dialog is for take the dialog down (`Prompts`)? Does the idle lock wait while one is up?
  - Is the E2E stand-in (`passphrase_dialog/e2e.rs`, which reads a queue file and logs what it shows) built only with `e2e`?
  - Are keyring entries only `passphrase:<database id>`? In the Flatpak, is the Secret portal's store used, and never the Secret Service or `oo7::Keyring::new()`?
  - The macOS `.app` is signed ad-hoc without the hardened runtime (`signingIdentity: "-"`, `hardenedRuntime: false`, #19): its designated requirement is its cdhash, and dyld honours `DYLD_*` variables and loads libraries nobody validated. Can a same-user process run code inside the genuine HoploDex that way (or with a debugger) and so read a saved passphrase from the keychain without a prompt? Should the hardened runtime be on?
  - Can `test-kdf` (1,000 iterations) never reach a shipped or E2E build (the `compile_error!` in `db/cipher.rs`)?
- **Accepted:**
  - [011 research.md §20](../specs/011-flatpak/research.md): a compromised Flatpak process can read HoploDex's own saved passphrases.
  - [011 research.md §20](../specs/011-flatpak/research.md): on a `.deb`, `.rpm` or AppImage, any unlocked Secret Service item is reachable.
  - [003 research.md §1](../specs/003-database-protection-management/research.md) (amended 2026-11-04, the owner's decision): while "Show passphrase" is ticked, the user has chosen to let the typed text be read by other processes. On macOS the dialog swaps each field's `NSSecureTextField` for a plain `NSTextField`, which the Accessibility API can read, and Secure Event Input goes off; on Windows `EM_SETPASSWORDCHAR` 0 lets a process at the same integrity level read the text with `WM_GETTEXT`; on Linux GTK exposes it through AT-SPI. Another process can tick it too: on Windows any process of the same user (which could already log the keys), on macOS one the user has granted Accessibility.
- **Since:** 001, 003; Flatpak store 011; `test-kdf` #105; strength worker #106, gone with the native dialog (2026-11-04); wipe-on-free allocator.

## plaintext-lifetime: secure deletion, locking and what outlives a session

- **Code:** `src-tauri/src/services/secure_delete.rs`; `src-tauri/src/db/mod.rs` (`secure_delete`, `reclaim_deleted_record`); `src-tauri/src/main.rs` (`end_web_processes`, `#[global_allocator]`, the core-limit and fatal-signal calls, the page-load hook); `src-tauri/src/allocator.rs`; `src-tauri/packaging/apport-ignore`; `src-tauri/tauri.conf.json` (`bundle.linux.deb.files`); `src-tauri/tests/core_dumps_test.rs`; `src-tauri/src/services/crash_reports.rs`; `src-tauri/src/session/lifecycle.rs`; `src-tauri/src/session/idle.rs`; `src-tauri/src/session/clock.rs`; `src-tauri/src/session/pending.rs`; `src-tauri/src/session/fingerprint.rs`; `src-tauri/src/platform/**`; `src/features/session/**`; `src/components/Toast.tsx`
- **Ask:**
  - Is a deleted record's text gone from the file, both search indexes and the journal (`src-tauri/tests/deletion_wipe_test.rs`)?
  - Is a partial file securely deleted?
  - Does the database lock on idle, sleep, screen lock and shutdown on each OS, and inside the Flatpak (logind `session/auto`)?
  - On a lock, are the key dropped, the session's web-view state ended (`SessionScope.end()`) and late responses dropped (#69)?
  - Is a pending draft written only inside the encrypted file?
  - Is every block Rust frees zeroed first, in the app and both helpers, by a wipe the optimizer can't drop, and an old block on `realloc` too (`allocator.rs`)?
  - Does any copy of the passphrase or the derived key outlive the command that used it, in the app or a web view process, and after a lock? A typed passphrase never reaches the web view (`key-passphrase`; `e2e/specs/passphrase-dialog.e2e.ts` scans the web process on Linux); what the app process keeps is `key-passphrase`'s.
  - Are core dumps off, soft and hard limit, for the app and every process it starts on Linux and macOS, WebKit's web and network processes included on Linux (on macOS they are launchd's) (#166)? Inside the Flatpak, is the web process the portal starts (which doesn't inherit the app's limit) lowered at each page load of either web view, as the Flatpak check asserts?
  - On Linux, does a fatal signal end the app, its helpers and the probe at once with no core (`platform/core_dumps.rs`)? apport, the kernel's core handler on Ubuntu 24.04 and 26.04 (both desktops install `apport-core-dump-handler`, not systemd-coredump), reads a core whatever the limit. Does the `.deb` keep `/usr/bin/hoplodex`, and nothing else, on apport's ignore list?
  - On Linux, is the core dump filter 0 for the app and every process it starts, inherited through `fork` and `exec` (`turn_off_core_dumps`), so a report apport writes of WebKit's web or network process holds registers and the names of mapped files, not memory? That matters most on Ubuntu 24.04: does the web process run outside WebKit's sandbox there, which apport skips (#180)? Inside the Flatpak, the portal starts the sandboxed web process, not the app, so it doesn't inherit the filter and has only the lowered core limit: does that process still run in the user's `user.slice` cgroup, and does apport still skip its crash there, so that its memory reaches no report?
  - What else can keep a crashed process's memory on each OS: macOS crash reports (stacks and registers only), Windows Error Reporting and WebView2's crash reports?
  - On Windows, does every web view's environment have custom crash reporting on, are WebView2's Crashpad reports deleted at startup, close and exit, and does a panic end the app or a helper before abort's fail-fast reaches Windows Error Reporting? On Windows, does a failed allocation end the process with `TerminateProcess` rather than abort's fail-fast, which reaches Windows Error Reporting (`allocator.rs`)?
  - Regression: #69.
- **Accepted** ([008 research.md §22](../specs/008-hardening-batch/research.md)):
  - The last-saved version is recorded after the commit.
  - Notifications raised by the session layer (none today) would escape the keyed `ToastProvider`.
- **Since:** 001, 003; scope 008; wipe-on-free allocator.

## document-copies: decrypted copies for another app (001 FR-035)

- **Code:** `src-tauri/src/commands/documents.rs` (`OpenerRoute::Portal`); `src-tauri/src/services/consent.rs`; `src-tauri/src/services/document_types.rs`; `src-tauri/src/services/machine_settings.rs`; `src-tauri/src/services/sandbox.rs`; `src-tauri/src/app_dirs.rs`
- **Ask:**
  - Is each copy written under the session lock, into the session that asked (#65)?
  - Is it `0700`/`0600` from creation, following no link and entering no other owner's folder, with a protected DACL on Windows (#71)? Does it get the canonical extension and a `Zone.Identifier` or `com.apple.quarantine` mark?
  - Are copies cleared at close, on signals and at startup?
  - Is consent asked by the OS, never by the web view?
  - Does `classify` run at attach, preview and open? At attach, does `classify_header` judge the extension and first bytes before the rest of the file is read?
  - Is an OOXML or ODF package refused when a name is listed twice or (OOXML) equivalent, a local header disagrees with its central record on what decides the content, two entries overlap, or a local header sits where the directory lists none? Does the local-header reading (`unambiguous`, `local_span`, `zip64_values`) stay within the file's bytes?
  - Flatpak's OpenURI route counts no answer within 5 s, or a closed chooser, as opened (011 research.md §8). Is that acceptable?
  - Regression: #65, #71.
- **Accepted:**
  - [007 research.md §1](../specs/007-document-preview/research.md): once the user confirms the first, a compromised web view can open the session's other documents.
  - [007 research.md §1](../specs/007-document-preview/research.md): content checks are signature-level (XLM sheets, embedded objects, field codes, chart workbooks).
  - [011 research.md §20](../specs/011-flatpak/research.md): the Documents portal remembers a copy's path and name after it is deleted.
- **Since:** 001; hardening 007; portal 011.

## document-preview: the PDF surface, TIFF and text preview

- **Code:** `src-tauri/src/services/preview/**`; `src-tauri/src/services/crash_reports.rs` (`web_view_environment`); `src-tauri/src/commands/preview.rs`; `src/features/media/DocumentPreview.tsx`; `src/features/media/PreviewSurface.tsx`; `src-tauri/examples/pdf_surface_check.rs`; `src-tauri/examples/pdf_surface_dns.c`; `scripts/pdf-surface-check.sh`; `scripts/macos/pdf-surface-check.sh`; `scripts/windows/pdf-surface-check.ps1`
- **Ask** (007 research.md §4 to §8, §11):
  - Does the surface have no network, no disk and no running code? On Windows, does the environment HoploDex builds for it (`surface/windows.rs`'s `customize`) carry all of `browser_args` and the surface's own data folder?
  - Is the `hdpreview` token one-time, and refused to every web view but the preview's?
  - On macOS, are the WebKit SPI switches (`webkit_switches`) and their startup check, the watch and the `pdfPreviewHold` in place?
  - Is `tripwire.rs` intact?
  - Does `sandbox_probe.rs` fail closed and resist steering by the environment? (It skips only containers, and runs in Flatpak since 011.)
  - Does the main web view receive only PNGs HoploDex encoded? A helper answers a page with raw pixels: does the app check them against the request (`helper_handle.rs`'s `fits_the_request`) before it encodes them, and is that all it reads of them?
  - The app reads and encodes a helper's page outside the helper's memory cap, and encodes it after `exchange`'s time limit: what can a hostile helper make it spend, from `read_frame_by_tag` up to `MAX_FRAME` for a `Page` frame (every other response is held to `max_response`'s smaller limit before its payload is read) before `fits_the_request` looks, through `encode_png`?
- **Accepted:**
  - [007 research.md §1](../specs/007-document-preview/research.md), macOS: WebKit's PDF code runs where Tauri injects IPC, and the ACL manifest is the guard.
  - [007 research.md §1](../specs/007-document-preview/research.md), Linux without WebKit's sandbox (a container, or where the system forbids it): a PDF exploit runs with the user's access.
  - [007 research.md §1](../specs/007-document-preview/research.md), Windows: Chromium's own sandbox is what contains Edge's viewer.
  - [011 research.md §9, §20](../specs/011-flatpak/research.md): inside Flatpak the sandbox is a layer, not a wall (the portal answers in the sub-sandbox).
- **Rerun:** the PDF surface check on each OS, and again after any web view engine update.
- **Since:** 007.

## helpers: the confined child processes

- **Code:** `src-tauri/src/services/helper/**`; `src-tauri/src/services/preview/helper.rs`; `src-tauri/src/services/preview/helper_handle.rs`; `src-tauri/src/services/preview/helper_protocol.rs`; `src-tauri/src/services/crash_reports.rs` (`quiet_error_reporting`, `end_process_on_panic`); `src-tauri/src/main.rs` (`--render-helper`, `--import-helper`, `--webkit-sandbox-probe`, the helper profile sweep)
- **Ask:**
  - Does no database, key or IPC reach a child? Are core dumps off, and on Windows are Windows Error Reporting's box, heap and snapshot off (`quiet_error_reporting`)? On Windows, does a child that hits its job's memory cap end without Windows Error Reporting (`allocator.rs`)?
  - Linux: a seccomp filter (`confine.rs`) that allows by default and answers a list of calls with `EPERM`: sockets, other processes and their memory, a file's owner, starting a program, namespaces and mounts, kernel interfaces, shared memory, metadata changes, opening files and path lookups. It refuses a call from another architecture or x32, and a helper that can't install it reads no document. It is installed so as to leave the CPU speculation settings as they were, which is what a filter does by default from Linux 5.16: before that a filter tied them to itself and a helper ran over three times slower. Under it, as a second layer, Landlock where the kernel has it (no opening a file to read or write it, no creating or removing one, no starting a program; TCP refused from ABI 4, kernel 6.7; abstract Unix sockets from ABI 6, kernel 6.12; pathname Unix sockets from ABI 9, kernel 7.1), and `RLIMIT_DATA`. macOS: inherited descriptors closed from 3 up, then `sandbox_init` with the profile `(version 1)(deny default)`. Windows: an AppContainer with no capabilities, set by `CreateProcessW` before the helper's first instruction, with `LOCALAPPDATA` the only variable given and no fallback if it can't start, one container per helper, and that container's profile the one place a helper can write: deleted when the helper ends, behind a bounded wait so a close or lock doesn't wait on it, or, if HoploDex ended first or the delete failed, at the next start (`sweep_profiles`), until which it stays on disk; a job object (kill on close, one process, UI limits, 2 GiB for the render helper and 512 MiB for the import helper); and process mitigations.
  - What can a confined helper still reach on each OS: files, file metadata, TCP and UDP, sockets, credential stores, and on kernels without Landlock? On Windows, can one helper reach another, and what can a helper leave in its AppContainer profile, and for how long?
  - Does the filter's list cover the system calls of current kernels (a call it doesn't name is allowed), and does its allowance for `clone`, `mmap`, `fcntl`, `kill`, `tgkill` and `prlimit64` still hold only for what a helper needs?
  - Does each child die with the app (`PR_SET_PDEATHSIG`, the job, `kqueue`)?
  - Are frames bounded both ways, and is each render-helper response refused from its length and tag alone (`helper_protocol::max_response` through `frames::read_frame_by_tag`) before its payload is read?
  - Can anything but the app start a helper mode (for example a desktop file or shell association passing `--import-helper`)? Is the mode harmless if one does?
- **Accepted** ([008 research.md §22](../specs/008-hardening-batch/research.md)): the macOS helper has no enforced memory cap.
- **Since:** 007; shared with import 008.

## untrusted-images: photos

- **Code:** `src-tauri/src/models/photo.rs`; `src-tauri/src/commands/photos.rs`; `src-tauri/src/services/attachments.rs`; `src/features/media/**`
- **Ask:**
  - JPEG and PNG are decoded in the app's own process, unconfined, with the `image` crate's default limits (`models/photo.rs`, and `image` in `src-tauri/Cargo.toml`). Is that bounded against decompression bombs and decoder bugs? Should it move into the render helper as TIFF did?
  - Originals are shown in the main web view, which holds IPC. Is the type checked by content, not by name?
  - Beside the 50-per-record cap (#98), is every photo and document judged against its byte cap (15 MiB, 30 MiB) and the record's 1 GiB total from the opened file's size before any of it is read, on every path (a drop's path, the picker's bytes), and is the read bounded at the size admitted (`services::attachments::read_attachment_file`)? Is a photo's type taken from its first bytes, not its name, and anything but JPEG or PNG refused before it is decoded?
- **Since:** 001 (in the baseline review's scope, but on no list until this one).

## database-files: opening, upgrading and restoring database files

- **Code:** `src-tauri/src/db/mod.rs`; `src-tauri/src/db/upgrade.rs`; `src-tauri/src/db/migrations/**`; `src-tauri/src/db/raw_file.rs`; `src-tauri/src/session/upgrade.rs`; `src-tauri/src/session/pending.rs`; `src-tauri/src/commands/databases.rs`; `src-tauri/src/commands/backups.rs`; `src-tauri/src/services/backups.rs`; `src-tauri/src/services/machine_settings.rs`
- **Ask:**
  - With an older or crafted database whose passphrase the user has, does the open or upgrade trust anything beyond data? That covers schema objects (triggers, views) that could fire during a step; `schema_migrations` names (an unknown one must mean `NewerVersion`); and the `app_state` and `collection_settings` values read before any step (009 research.md §13).
  - Is a pending draft from another HoploDex version (#78) parsed safely?
  - Is the awaiting-upgrade slot unreachable by collection commands, and dropped on any open, lock, sleep, shutdown or quit?
  - Does the before-upgrade backup's folder go through the `paths` checks, with an exclusive create and secure deletion of a partial file?
  - Are test-only steps impossible in a shipped build (`upgrade-test-steps`, the `compile_error!` in `db/upgrade.rs`)? Is `9000_e2e_upgrade_check` E2E-only?
  - Is `machine.json` (recent paths, `pdfPreviewHold`, the opening setting, the log's `troubleshootingLog` and `troubleshootingLogNoticeShown`) read as untrusted?
- **Accepted** ([009 research.md §19](../specs/009-upgrade-readiness/research.md)): an `fsync`-ignoring filesystem; the lock held while the upgrade question is open; the restored-from backup rotating out; case folding only for A to Z; a backup opened directly becoming its own database. These are integrity limits; reassess whether any is a security issue.
- **Since:** 003; upgrade 009.

## flatpak: the Flatpak sandbox and its portals

- **Code:** `flatpak/io.github.exodious.HoploDex.yml`; `scripts/flatpak/manifest.test.ts`; `scripts/flatpak/contents.mjs`; `src-tauri/src/services/sandbox.rs`; `src-tauri/src/services/keyring/portal.rs`; `src-tauri/src/platform/linux.rs`; `src-tauri/src/services/preview/sandbox_probe.rs`; `src-tauri/src/commands/error.rs`; `src-tauri/src/commands/documents.rs` (`portal_open_directory`)
- **Ask:**
  - Does the manifest grant no permission beyond 011 research.md §4's table, as `manifest.test.ts` enforces?
  - Does Show log file reach only `OpenURI.OpenDirectory`, with a read-only descriptor of the log file (`commands/documents.rs`, `portal_open_directory`)?
  - Is every write location checked against `Reach` first? Can `HostPaths`' mapping of a Documents-portal path be steered outside the grants?
  - Is the Secret portal used, and never the Secret Service?
  - Does the web process match 011 research.md §9's arguments with the sandbox on?
  - Does the bundle carry only what `contents.mjs` allows?
- **Accepted** ([011 research.md §20](../specs/011-flatpak/research.md)):
  - What a compromised process reaches: `<Documents>/HoploDex`, the drives, its own saved passphrases, logind and the screen savers.
  - A user's `flatpak override` widens that.
  - The Documents portal remembers copies.
  - The runtime ends with GNOME 51 (#129).
  - The `.flatpak` file is unsigned (#19).
- **Rerun:** the Flatpak check (`docs/development/flatpak.md`, "The Flatpak check").
- **Since:** 011.

## logging: what the log carries

- **Code:** `src-tauri/src/logging.rs` (and every `log::` call in the backend); `src-tauri/src/logging/**`; `src-tauri/src/commands/troubleshooting_log.rs`; `src-tauri/src/models/troubleshooting_log.rs`; `src-tauri/tests/log_contents_test.rs`; `src-tauri/tests/log_lines_test.rs`; `src-tauri/src/commands/documents.rs` (`Opener::reveal`); `README.md` (what it says about the log); `flatpak/io.github.exodious.HoploDex.metainfo.xml` (what it says about the log); `src/features/app/TroubleshootingLogSection.tsx`; `src/features/databases/DatabaseChooser.tsx` (the log's notice)
- **Ask:**
  - Does each line hold only fixed text, HoploDex's own codes, OS error text and codes, durations and path shapes (013 FR-016)? `tests/log_lines_test.rs` catches a path printed the usual ways and `tests/log_contents_test.rs` covers the journey it walks; for every other line, and every failure path neither reaches, what can each interpolated `{err}` or value print? Could any carry a record's value, a name of a database, backup, folder, photo or document, a user name, a draft's text, a passphrase, a count of records or a row id?
  - Do serde errors go through `logging::json_error`, database errors through `logging::db_error`, `OpenError::for_log` or `UpgradeError::for_log`, and paths through `logging::place`? Can `place`'s shape show a name: a user name under a kept root, a server's or share's name, an extension off its short list?
  - Does the file take only HoploDex's own targets, whatever the level, and do debug lines stay out of standard error and the journal (013 FR-019, FR-026)?
  - With no `HOPLODEX_LOG`, can a failed start write to or delete the log's files before the saved choice is read (`logging::start_for_variable`)? What can a failed `setup`'s error print through `logging::startup_failed`?
  - Does each line about a portal's reply, and the Secret portal's reason that the keyring logs, give only fixed text and the D-Bus error's name (`logging::portal_error`, `DesktopPortal::retrieve_secret`), and never a path, a secret or an error's message? Does any line repeat once per record, which would count them?
  - Does a release build's panic reach every output as its location alone (`logging::panic_location_only`, first in `main()`), with no hook that prints its message and, on Windows, still ending the process (013 FR-027)?
  - Can the default outlive 0.x (`logging::default_on`), or a raise save a choice or outlast the run (013 FR-001, FR-004, contracts/tauri-commands.md)?
  - Are the file's limits kept: at most 2 MiB and one rotated file, with no line longer than 4 KiB, deleted 14 days after it was made, deleted when turned off and at a start with the log off? Is it created exclusively and never followed through a planted link? Is it created only readable by this user on Unix?
  - Can the page read the file's contents, or choose what `show_troubleshooting_log` shows? It takes no argument, and the only path is Rust's.
  - Is the file out of every backup and export?
  - Outside a Flatpak, does Show log file reach only `tauri_plugin_opener::reveal_item_in_dir` (`FileManager1.ShowItems` on Linux, the OS's file manager on macOS and Windows) or, on Linux and macOS where that fails, the log's folder through `xdg-open` or `open`, with only Rust's path? On any route, can a link planted at the log's name make it show or hand over anything else?
  - `HOPLODEX_LOG` is read from the environment at each start and outranks the saved choice for that run, `off` included (`logging::decide`). Can it save a choice or delete a file? Who else can set it (a launcher, a desktop file, `flatpak override --env`), and what do they gain from a raised level the user turned off and isn't told about?
  - Do the helpers and the sandbox probe log nothing, open no file and not receive `HOPLODEX_LOG` (`env_clear` in `services/helper/handle.rs`)? The parent logs for them (013 FR-031).
  - Does what the app, the README and the store listing say about the log stay no wider than the tests and this review support: what it is designed to leave out, not what it never contains (013 FR-021, FR-022)?
- **Accepted** ([013 research.md §9](../specs/013-troubleshooting-log/research.md); standard error and the journal since [011 research.md §20](../specs/011-flatpak/research.md)):
  - The log reaches standard error or the journal on every OS.
  - The file is plain text, readable by the same OS user and anything running as them.
  - OS error text is written as it comes, with only the home folder replaced.
  - A compromised main web view can turn the log on or off and raise the level, without reading it.
  - A file's age is checked only at a start, so one past 14 days stays while the app is closed or a run is left open, and through a run under `HOPLODEX_LOG=off`. Where the file system doesn't record when a file was made, each start's new lines reset its age. The notice's and store listing's "up to 14 days" doesn't say so; the README says only that an old file goes at the next start.
- **Since:** 011; the file 013.

## shipped-build: what only test and E2E builds may contain

- **Code:** `src-tauri/Cargo.toml` (`[features]`, `[patch.crates-io]`); `src-tauri/build.rs`; `src-tauri/src/db/cipher.rs`; `src-tauri/src/db/upgrade.rs`; `src-tauri/src/app_dirs.rs`; `src-tauri/src/session/idle.rs`; `src-tauri/src/services/keyring.rs`; `src-tauri/vendor/**`; `scripts/check-no-webdriver.mjs`; `scripts/check-package.mjs`; `scripts/build-appimage.sh`
- **Ask:**
  - Is `e2e` still the only thing that brings `tauri-plugin-wdio-webdriver` (`cargo tree`)? `flatpak-check` implies it.
  - Is every marker absent from every shipped binary and installer, on each OS? The package jobs check the release binary they built on each OS, and `check-package.mjs` the binary inside the `.dmg` (#19). Is every published package built by a job that runs one of these checks?
  - Should the markers in `check-no-webdriver.mjs` cover every E2E-only `HOPLODEX_E2E_*` name in the backend?
  - Does anything stop a shipped build with `mock-keyring` alone, beside `custom-protocol`?
  - Do `test-kdf` and `upgrade-test-steps` still refuse `custom-protocol` and `e2e` at compile time?
- **Since:** #29, #76; seams 007, #76, 009; `test-kdf` #105.

## unsafe-ffi: unsafe Rust and platform FFI

- **Code:** (every backend file with `unsafe`) `src-tauri/src/allocator.rs`; `src-tauri/src/commands/documents.rs`; `src-tauri/src/db/cipher.rs`; `src-tauri/src/db/raw_file.rs`; `src-tauri/src/platform/core_dumps.rs`; `src-tauri/src/platform/macos.rs`; `src-tauri/src/platform/windows.rs`; `src-tauri/src/services/backups.rs`; `src-tauri/src/services/crash_reports.rs`; `src-tauri/src/services/helper/confine.rs`; `src-tauri/src/services/helper/job.rs`; `src-tauri/src/services/main_navigation.rs`; `src-tauri/src/services/preview/availability.rs`; `src-tauri/src/services/preview/helper.rs`; `src-tauri/src/services/preview/sandbox_probe.rs`; `src-tauri/src/services/preview/surface/linux.rs`; `src-tauri/src/services/preview/surface/macos.rs`; `src-tauri/src/services/preview/surface/windows.rs`; `src-tauri/src/services/passphrase_dialog/native/gtk.rs`; `src-tauri/src/services/passphrase_dialog/native/appkit.rs`; `src-tauri/src/services/passphrase_dialog/native/win32.rs`; `src-tauri/src/services/sandbox.rs`; `src-tauri/src/services/scratch.rs`; `src-tauri/src/services/secure_delete.rs`; `src-tauri/src/window_controls/macos.rs`; `src-tauri/src/window_controls/windows.rs`
- **Ask:**
  - Does each `unsafe` block's `SAFETY` comment hold? Does the global allocator (`allocator.rs`) only pass `System` the caller's layout, and wipe no byte outside the block it is freeing?
  - Check ObjC and WebKit SPI on macOS; COM and Win32 (DACLs, job objects, AppContainers, WebView2); GTK and WebKitGTK; and libc (Landlock, the seccomp filter, `prctl`, descriptors, the `NonNull` in `db/raw_file.rs` from CodeQL alert 68).
  - Is `set_var` called only before any thread starts?
- **Since:** grew from 4 files at the baseline review to 26 (007, 008, 011, #166, the allocator, the native passphrase dialog, `sqlite3_key_v2`).

## build-ci: what can change what ships, or the repository

- **Code:** `.github/workflows/**`; `.github/actions/**`; `.github/dependabot.yml`; `.github/codeql/**`; `.github/release-notes.md`; `scripts/ai-security-review.mjs`; `scripts/check-package.mjs`; `scripts/release.mjs`; `src-tauri/tauri.conf.json` (`bundle`: `targets`, `macOS`); `src-tauri/tauri.bundle.conf.json`; `src-tauri/build.rs`; `vite.config.ts`; `scripts/licenses/vitePlugin.mjs`; `scripts/build-appimage.sh`; `flatpak/**`; `scripts/flatpak/build.sh`; `scripts/flatpak/sources.mjs`; `tools/plate-tuner/**`; `package.json` (scripts); `audit-ci.jsonc`; `audit-deprecated.json`; `audit-openssl.json`; `scripts/check-openssl-advisories.mjs`; `scripts/windows/vcpkg/vcpkg.json`; `src-tauri/deny.toml`
- **Ask:**
  - Do workflows take least privilege, avoid `pull_request_target`, receive untrusted input only through `env`, pin actions (#72), set `persist-credentials: false`, and expose no secrets on pull request events?
  - Can no code change skip the audit (`audit.yml`'s skip rule, #142), or `ci.yml`'s jobs (`.github/actions/scope`'s Markdown-only rule), including the packages jobs' `check-no-webdriver.mjs`? Does `FLATPAK_PATHS` still cover what the Flatpak check protects?
  - Are caches saved only from `main` (`.github/actions/setup`)? Do the runs that only restore them (a pull request, the release workflow's tag run and its `workflow_dispatch` dry run) restore only what `main` saved, so no pull request or other branch can plant a dependency, toolchain or OpenSSL build that `main` or a release uses? Is the Windows OpenSSL that `main` cached all a published package takes from a cache? Is vcpkg fetched and built only at the commit `builtin-baseline` pins (`scripts/windows/vcpkg/vcpkg.json`)?
  - CodeQL doesn't scan workflows (no `actions` language), `vite.config.ts`, `vitePlugin.mjs` or `scripts/` (`.github/codeql/codeql-config.yml`). Does this review cover them?
  - Does every dev server that can write to the tree need a per-run token and bind to `localhost` (#70)?
  - Is the vendored crate built only under `e2e`?
  - Is the Flatpak built offline from the committed source lists?
  - Does `scripts/ai-security-review.mjs` change code scanning only as the skill says: uploads only to a branch or `refs/pull/<n>/head`, under the fixed `TOOL` and `CATEGORY`, with only introduced findings on a pull request (its annotations are public); dismissals only through `carry-dismissal`, from an alert already dismissed? Does `upload` report an open alert an upload left out (which closes it)? Does `gate` count every tool's open critical and high alerts, Dependabot, secret scanning and draft or triage advisories?
  - The release workflow (`release.yml`, #19): does it draft a release only on a `v*` tag, which only the repository's admins can create (a `workflow_dispatch` run is a dry run that drafts nothing), and never publish it? Does only its draft job hold `contents: write`, and only after every other job passed? Does it refuse to draft unless `ai-security-review.mjs reviewed` finds the release review's upload for exactly the tagged commit, on a branch, with no critical or high code scanning or Dependabot alert open? Does `ci.yml`'s `release` input change only what `docs/development/ci.md` says (the E2E profile, macOS E2E counting, both Flatpak checks, keeping the packages)? Does every published file come from a package job of this run, never a cache?
  - Does `scripts/release.mjs` tag only in its last step, after every earlier step passed and `reviewed` accepted the commit, and make no GitHub release?
  - Regression: #70, #72.
- **Since:** the baseline review (#70, #72); CodeQL #82; audit #85; pr-body #134; code scanning uploads and the release gate, #21's review skill; the per-pull-request CI and its pinned OpenSSL #25; the release workflow and the package checks #19.

## not-surface: checked and left out

- **Code:** `src-tauri/tests/**`; `src-tauri/examples/**`; `e2e/**`; `src/**/*.test.*`; `scripts/windows/setup-*.ps1`; `scripts/windows/gui.ps1`; `scripts/tart-vm.sh`; `scripts/human-testing.sh`; `scripts/dev-container.sh`; `.claude/**`; `tools/icons/**`; `docs/**`; `specs/**`
- **Why:** test and test-machine code, agent tooling, and documentation. None of it ships, builds what ships, or runs in CI. The check programs under `src-tauri/examples/` that a surface depends on are named in that surface. Test isolation from real data is constitution principle II's, not this review's. The Windows test machine's remote session (`scripts/windows/gui.ps1`, #138) is reached only over that machine's own sshd, and is not in the app, its build or the repository's CI.

## The constitution's seven areas

Until 1.5.0 the constitution listed seven areas inline. Each is an entry here:

- the commands exposed to the frontend: `ipc-commands`
- the Tauri capabilities and content security policy: `ipc-commands` and `webview-config`
- filesystem path handling: `paths`
- spreadsheet import parsing and export (including formula injection): `import-export`
- handling of the database key and passphrase: `key-passphrase`
- secure deletion: `plaintext-lifetime`
- decrypted document copies: `document-copies`

The other nine entries are what the seven missed: `document-preview`, `helpers`, `untrusted-images`, `database-files`, `flatpak`, `logging`, `shipped-build`, `unsafe-ffi` and `build-ci`.
