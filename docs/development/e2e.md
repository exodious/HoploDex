# E2E tests

_Part of the [development docs](../../DEVELOPMENT.md)._

## Building and running

`npm run test:e2e` builds the app with `cargo build --profile e2e --features
custom-protocol,e2e` and drives it through the WebDriver server compiled
into it. That binary embeds whatever is in `dist/`, so **run `npm run build`
first** (the same goes for `npm run screenshots`).

Spec files run in parallel, each in a WebdriverIO worker of its own: up to
`HOPLODEX_E2E_WORKERS` at once, by default half the CPUs and at most 8 on
Linux, and 1 on Windows and macOS (see the display section below). Each
worker runs its own app (a few hundred MB for WebKitGTK) and several hundred
threads, which is why `scripts/dev-container.sh` raises the container's
process limit. A run takes at least as long as its longest spec file, and
each file has a start-up cost of its own; "E2E files" below says how tests are
grouped into files. WebdriverIO hands spec files to workers in
the order `specs` lists them, so `wdio.conf.ts` names the longest files first
(`LONGEST_FIRST`) and then globs the rest; otherwise file name order can leave
a long file to start last and run alone. When a spec file is split, renamed or
grows long, update that list from the per-spec times (issue #77's
"Measuring"); a name that matches no file fails the run at start-up. `--spec`
bypasses the order. `wdio.conf.ts` builds the app and
the human-testing seed once, in `onPrepare`, before any worker starts. For
debugging, run one spec file at a time, so the log isn't interleaved:

```bash
HOPLODEX_E2E_WORKERS=1 npm run test:e2e
```

## Writing a spec

Two WebdriverIO 10 rules to keep in mind when writing a spec or a helper:

- **`$` is strict.** It fails (`StrictSelectorError`) when its selector
  matches more than one element. Narrow the selector to the element the test
  means (`button.hd-policy__open=<name>`, `aria/<name>`, a chain from its
  row), or say you want the first one with `$$(selector)[0]`. To assert a
  count or an absence, use `expect($$(selector)).toBeElementsArrayOfSize(n)`.
  Don't turn strictness off.
- **The support modules load twice in a worker:** once with `wdio.conf.ts`
  and once with the spec files. A module's own variables aren't shared
  between the copies, so state the config's hooks and the specs both use
  lives on `globalThis` (the worker's port and app in `e2e/support/app.ts`).

## The `e2e` profile and features

The `e2e` profile (`[profile.e2e]` in `src-tauri/Cargo.toml`) is the release
profile without fat LTO, with 16 codegen units and incremental builds, so
touching the backend rebuilds in seconds instead of a minute. It keeps
`panic = "abort"`. The trade-off: E2E doesn't drive a byte-identical copy of
the shipping binary. (It never did, because of the in-memory keyring.) Before
a release, run the suite once against the shipping profile:

```bash
npm run build && HOPLODEX_E2E_PROFILE=release npm run test:e2e
```

`HOPLODEX_E2E_PROFILE` is `e2e` (the default) or `release`; it picks the
profile `wdio.conf.ts` builds the app and the seed with, and the binary it
launches (`target/<profile>/hoplodex`).

The `e2e` Cargo feature is for builds that must never reach a shipped one. It
compiles in `tauri-plugin-wdio-webdriver`, a W3C WebDriver server on
`127.0.0.1` at `TAURI_WEBDRIVER_PORT` (#29). It has no authentication and runs
any script in the page, so `scripts/check-no-webdriver.mjs` checks that a
shipped build has none of it: the dependency graph in `npm run audit`, and the
release binary in `scripts/build-appimage.sh`. The feature also implies
`mock-keyring`, and lets
`HOPLODEX_E2E_IDLE_MINUTE_SECONDS` set how many seconds the idle lock counts
as one minute (default 60). The harness sets it to 3 for
`locking.e2e.ts`, so its idle-lock test waits seconds, not a real minute;
the notice and the settings still say "1 minute". The scaling's unit tests
(`session/idle.rs`) run in the default Rust test run.

The `flatpak-check` feature builds on `e2e` for the Flatpak check alone
(`scripts/flatpak/checkManifest.mjs` turns it on; the `e2e` suite, the `.deb`
and every other build never do). It keeps the WebDriver server and the
`HOPLODEX_E2E_*` seams, and leaves out two of `e2e`'s stand-ins: `build.rs`
turns it into the `app_opener` and `mock_keyring` cfgs, so the check variant
has the real `AppOpener` (the OpenURI portal inside a Flatpak) and the real
keyring (inside a Flatpak, the Secret portal's file keyring) in place of
`E2eOpener` and the in-memory store.
[The Flatpak check](flatpak.md#the-flatpak-check) says what it talks to.

## How the harness runs the app

The harness, not a driver, owns the app's process (`e2e/support/app.ts`):
`wdio.conf.ts` launches the app before each spec file's session, with the
sandbox's environment and `TAURI_WEBDRIVER_PORT`, waits for its server to
answer, and kills it afterwards. Each worker has a port of its own: 4445 for
the run's first worker, and one up for each later one (worker `0-5` uses
4450). A spec that relaunches the app calls
`relaunchApp()`, which kills it (SIGKILL, so no exit handler runs, like a
crash), starts it again on the same sandbox, display and port, and opens a
new session.
Ending a WebDriver session alone doesn't close the app. The server runs
scripts and dispatches keys in the page, so key presses are synthetic
(untrusted) `KeyboardEvent`s; `e2e/support/ui.ts` already clicks and fills
through page scripts. For genuine input, see [Real keyboard and mouse
input](real-input.md).

On Linux each worker starts an isolated Xvfb virtual display of its own
(`e2e/support/display.ts`, with `-noreset`, so that the display can't
reset under the app's GTK init when the sandbox probe leaves it) and
points `DISPLAY` at it, so the app never touches your real desktop and
parallel workers never share a screen, a pointer or a keyboard. WebdriverIO 10's testrunner would otherwise start one
display shared by the whole run, so `wdio.conf.ts` turns it off
(`displayServerEnabled: false`), and `run-e2e.mjs` drops `WAYLAND_DISPLAY`,
which would make the testrunner point GTK at your compositor even then. On Windows and macOS
workers would share a desktop, so another worker's window could take the foreground at any moment,
and Radix closes an open menu or select list when its window loses focus. So the suite runs one
worker at a time there by default; setting `HOPLODEX_E2E_WORKERS` higher brings the race back.
The harness self-heals after an interrupted prior run,
killing anything left over on a worker's port before launching the app.

## Quit cleanup

The E2E suite kills the app rather than quitting it, so a script checks
that decrypted document copies are removed when the app quits, against the
built E2E binary. It also closes the window as soon as it shows, before the
page listens for the quit request, and there checks only that the app exits:

- **Linux:** `xvfb-run -a python3 e2e/scripts/quit-cleanup.py` (needs Xvfb,
  no other packages). Window closed, SIGTERM, SIGHUP and SIGINT. It also
  checks that no process of the app can leave a core dump, and that on a quit
  by signal the app ends its web processes itself (#166), so it needs a hard
  core limit above 0 (`ulimit -H -c`).
- **macOS:** `python3 e2e/scripts/quit-cleanup.py`, in a logged-in desktop
  session (in a tart VM, `scripts/tart-vm.sh gui python3
  e2e/scripts/quit-cleanup.py`). Window closed, Cmd+Q, SIGTERM, SIGHUP and
  SIGINT. It presses the close button through the Accessibility API and
  posts Cmd+Q to the app, so whatever runs it needs Accessibility.
- **Windows:** `powershell -ExecutionPolicy Bypass -File
  e2e/scripts/quit-cleanup.ps1`, in a desktop session (over SSH, through
  `scripts\windows\gui.ps1`). Window closed,
  `taskkill` without /F, and a log-off (`WM_QUERYENDSESSION` and
  `WM_ENDSESSION` to every window, then the process ended at once, as
  Windows may).

None needs the real directories: each launch gets a scratch sandbox. An
E2E build never hands an opened document to the OS either, so no real
viewer starts: `open_document` writes the copy's path to
`HOPLODEX_E2E_OPENED_LOG` instead, which `photos-documents` checks.

## E2E-only seams

Feature 007 adds two more E2E-only seams, both read only by a build with the
`e2e` feature. `HOPLODEX_E2E_CONSENT` answers the native confirmation before a
document goes to another app, which no WebDriver can click: `open` answers
"Open in another app", anything else (or nothing) "Cancel". Each request's
title is appended to the file `HOPLODEX_E2E_CONSENT_LOG` names, which the
harness points into the sandbox. `wdio.conf.ts` sets the answer to `open` for
every spec, and a spec that tests the dialog sets its own and calls
`relaunchApp()`, since the app reads it at start-up.
`HOPLODEX_E2E_PDF_PREVIEW=off` makes the start-up check say the computer's PDF
viewer can't be used, so PDFs aren't previewable while TIFF and text still
are (a spec sets it and calls `relaunchApp()`, as `document-preview.e2e.ts`
does). Both
variable names are among the markers `scripts/check-no-webdriver.mjs` looks
for, so a shipped binary that carries either one fails `build-appimage.sh`'s
check, just as one with the WebDriver server does.

WebDriver drives only the main web view. What goes on inside the PDF surface,
a child web view, is the PDF surface check's.

## E2E files

An E2E file is named for the area of the app whose setup its tests share
(`locking.e2e.ts`, `accessories-mounting.e2e.ts`), with no spec or story
number in it: one file can hold journeys from several stories, and from
several specs. Each story's tests are one top-level `describe()` titled
`"<spec> US<n> - <the story's heading in spec.md>"`, such as
`describe("006 US2 - Mount an Item on a Firearm", …)`, and a regression test
that belongs to no story is `describe("Issue #<n> - …")`. The citation check
([Spec citations](citations-and-attack-surface.md#spec-citations)) fails when a title names a story its spec doesn't have, or doesn't quote
the story's heading exactly. To find a story's tests,
`grep -rl '"006 US2 -' e2e/specs`. A title that ends in `(was us9-locking)`
gives the file's name before #121, when a `usN` number, unrelated to the
specs' story numbers, began every name.

`flatpak.e2e.ts` is the exception: it has its own setup (the container's Flatpak
installation, private buses and stand-in services, `support/flatpak.ts`) and
runs only under `npm run test:flatpak`; `wdio.conf.ts` excludes it from every
other run and runs nothing else under `test:flatpak`.

A new journey goes into a new `describe()` in the existing file whose setup
it shares (the same seed, the same databases, the same relaunch or idle-lock
setting in `wdio.conf.ts`), whatever spec it belongs to. Each file costs
about 2.2 s to start and 2.5 s to set up its database or seed (#77), and at
1 worker, on the macOS and Windows VMs, a split costs more than it saves. Start
a new file only when a journey shares setup with no existing file, or when
the per-spec times (issue #77's "Measuring") show that one file has grown long
enough to hold the run up. Then split it along its `describe()` blocks, name
each part for what it shares, move shared helpers into `e2e/support/` (as
`accessories.ts` and `documentPreview.ts` hold the `accessories-*` and
`document-*` files' helpers), and update `LONGEST_FIRST`.
