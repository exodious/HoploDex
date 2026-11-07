# Test isolation

_Part of the [development docs](../../DEVELOPMENT.md)._

Your real application data is more than one file. It is every `.hoplodex`
database you have created or opened, wherever you keep it, and its backups
(in `HoploDex backups` next to it, or the folder you chose);
`machine.json`, with the recent-databases list, in the app's config
directory (`~/.config/io.github.exodious.HoploDex/` on Linux); the suggested
`<Documents>/HoploDex` folder; and each saved passphrase, a
`passphrase:<database id>` entry under `io.github.exodious.HoploDex` in the
OS keyring.
The database from before the passphrase model,
`~/.local/share/com.hoplodex.app/hoplodex.db`, and its key in the keyring
count too; nothing reads them any more. None of the tooling here touches any
of it:

- Rust tests use `tests/support::TestDb`, a real SQLCipher database in a temp
  directory, created by `db::create_database` with a fixed test passphrase
  and the production cipher settings. Tests never mock the database, and
  take every path (database, config directory) as a parameter.
  `MachineSettings::load` starts with the keyring off, so they never reach the
  OS keyring. The tests that need a keyring run only with
  `--features mock-keyring`, against keyring-core's in-memory store:
  `tests/keyring_test.rs`, `create_and_open_write_nothing_to_the_keyring` in
  `tests/passphrase_protection_test.rs`, the `remembering` module of
  `tests/upgrade_test.rs`, and `portability_test` (which checks the keyring
  stays empty). One filter runs them all; a test that gains
  `#[cfg(feature = "mock-keyring")]` joins it, here and in `scripts/tart-vm.sh`:
  `cargo nextest run --manifest-path src-tauri/Cargo.toml --features test-kdf,mock-keyring -E 'test(/^keyring_test::/) | test(/^passphrase_protection_test::.*::create_and_open_write_nothing_to_the_keyring$/) | test(/^upgrade_test::remembering::/) | binary(portability_test)'`.
- The troubleshooting log (`src-tauri/src/logging.rs`, 013) writes under the app's
  cache directory, so an E2E run's file is in its sandbox like the rest. The
  sandbox also sets `HOPLODEX_LOG=off`, so no run gets the log's notice on the
  chooser or writes the file, unless a journey changes it. Rust tests name their own
  folders, and the process-wide logger is only installed in
  `tests/log_contents_test.rs`'s child processes.
- `e2e/wdio.conf.ts` gives each session (one spec file, in a worker of its
  own) a throwaway sandbox (`e2e/support/sandbox.ts`, which `npm run session`
  uses too). An E2E build takes its config and cache
  directories and its documents folder (the suggested place for a new
  database) from `HOPLODEX_E2E_CONFIG_HOME`, `HOPLODEX_E2E_CACHE_HOME` and
  `HOPLODEX_E2E_DOCUMENTS`, never from the OS, and won't start without them
  (`src-tauri/src/app_dirs.rs`; every command gets its directories there).
  That's what isolates Windows, whose known folders ignore `APPDATA` and the
  like. The webview's own data goes into the sandbox too: through `XDG_*`
  directories on Linux (with a `user-dirs.dirs`) and `HOME` on macOS. On
  Windows the app puts each web view's data folder (`main-webview` and
  `preview-webview2`) under `HOPLODEX_E2E_CACHE_HOME`, so no
  `WEBVIEW2_USER_DATA_FOLDER` is set. One thing is shared on purpose: on
  Linux, Mesa's shader cache (`MESA_SHADER_CACHE_DIR`) is
  `node_modules/.cache/hoplodex-e2e-shaders` for every sandbox and run. In
  software rendering (the dev container's Xvfb), Mesa compiles the dialog
  shadow's shader the first time one is drawn, which stops the window
  painting for about a second; with a cache per sandbox, every spec paid that
  (#102). It holds compiled shaders only. Each spec starts at
  a first run and creates its database by typing a location in the sandbox
  (`createDatabase()` in `e2e/support/ui.ts`), or unlocks the seeded one
  with its passphrase (`unlock()`). There is no database key in the
  environment: a database opens with its passphrase alone, as in the app.
  E2E builds use the
  `e2e` feature, which includes `mock-keyring`, an in-memory keyring for saved
  passphrases, since a headless session can't unlock a real one. The harness keeps it in
  `keyring.json` in the sandbox (`HOPLODEX_E2E_KEYRING_FILE`) so a remembered
  passphrase survives a relaunch. A computer without a keyring is tested in
  Rust and Vitest, not in E2E (`tests/keyring_test.rs`,
  `DatabaseChooser.test.tsx`, `DatabaseSettingsDialog.test.tsx`).
- A sandbox is a `hoplodex-e2e-XXXXXX` directory in the system's temp
  folder with an owner file (`.hoplodex-e2e-owner`, the creating process's
  id) written at creation (#111). When a session ends, `removeSandbox()`
  retries for up to 30 s, because on Windows WebView2's processes keep the
  folder open for a moment after the app is killed, and logs a folder it could
  not remove. A run that is interrupted (Ctrl+C, a crash) never gets there, so
  `onPrepare` and `onComplete` in `wdio.conf.ts` and the `npm run session`
  launcher sweep first (`sweepStaleSandboxes`): a directory directly in the
  temp folder, with that name and an owner file, whose owner process is no
  longer running. Anything else there is left alone (the screenshots' plain
  `hoplodex-e2e-screen-<pid>.png` files, a folder from before the owner file
  existed, a sandbox another run is using), so several runs can be live at
  once. Delete old unmarked ones by hand.
- E2E steps don't sleep. The frontend counts its backend calls and pending
  search debounces (`src/lib/busy.ts`, `window.__hoplodexBusy`), and the
  helpers in `e2e/support/ui.ts` call `settle()` after each action, which
  waits for that count to reach 0, for finite animations shorter than a
  second to end, and for two painted frames. It leaves the chooser plate's
  animations running (its draw-in as well as its endless cycle), so a test
  about the plate calls `settleChooserPlate()`. Anything it can't see (a smooth
  scroll, a thing appearing) gets a `waitUntil` on that condition, not a
  `browser.pause()`. `waitUntil` and `waitForExist` ask every 20 ms
  (`waitforInterval` in `wdio.conf.ts`), not WebdriverIO's 100.
- `scripts/human-testing.sh` and `src-tauri/examples/human_seed.rs` point the
  app at `.human-testing/`: via `XDG_*_HOME` on Linux, and on macOS via a
  `HOME` of its own there (cargo, rustup and npm keep the real one). The seed writes only into a
  directory that is new, empty or holds the `.hoplodex-sandbox` marker it
  left there, refuses anything inside the real data, config or documents
  directory, and never touches the keyring.
- The [development container](dev-container.md) has
  none of the host's app data at all.
