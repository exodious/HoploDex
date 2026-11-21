# PDF surface, window controls, drops and WebKit's sandbox

_Part of the [development docs](../../DEVELOPMENT.md)._

## PDF surface check

The PDF surface (specs/007-document-preview/research.md §4 and §23) is a web
view that shows a document the app doesn't trust, and each OS builds it
differently, so one script per OS builds `src-tauri/examples/pdf_surface_check.rs`
(which uses the app's own `services::preview::surface`, not a copy) and drives
it with real input. It shows hostile, truncated, bit-flipped and 10 MB PDFs,
probes what a frame can reach (network, IPC, commands), clicks every toolbar and
context-menu item and presses the viewer's shortcuts, and fails if anything
reached the network, a command or the disk. It prints the 10 MB PDF's time to
first paint. Run it on every OS before a pull request that touches
`services/preview/surface/` or the viewer's page area, and whenever the web view
engine is updated (Tauri, wry, WebKitGTK, WebView2):

- **Linux:** `scripts/dev-container.sh scripts/pdf-surface-check.sh`, under Xvfb
  with XTest input and throwaway `HOME` and `XDG_*` folders.
- **macOS:** `scripts/macos/pdf-surface-check.sh`, from a Mac with the tart VM
  set up (it starts the VM, copies the checkout in, runs the check in the VM's
  desktop session and copies the log back). The `--hud-on` variant leaves
  WebKit's PDF toolbar on and passes only if the watch on the Preview copy
  deletes it, closes the surface and sets the PDF preview hold; run it too.
- **Windows:** `powershell -ExecutionPolicy Bypass -File
  scripts\windows\pdf-surface-check.ps1`, in the signed-in desktop session
  (over SSH, through `scripts\windows\gui.ps1`), at 100% scaling.

Each takes `shows` to run the 3-page PDF and take a screenshot instead. Logs and
screenshots go to `e2e/screenshots-out/pdf-surface/`. Exit codes: 0 passed, 1 no
window, 2 no load, 3 usage, 4 a check failed.

## Window controls check

The main window's browser controls (specs/008-hardening-batch/research.md §12
to §15) are two layers: the page's handlers (`src/features/app/browserControls.ts`)
and each OS's own filter on the web view's menu and keys
(`src-tauri/src/window_controls/`). Both are installed only in release and E2E
builds (`not(debug_assertions)`; `npm run tauri dev` keeps Inspect Element and
reload). The native layer differs on each OS, so one script per OS builds
`src-tauri/examples/window_controls_check.rs` with `--profile e2e` and drives
it with real input (`src-tauri/examples/real_input.{py,swift,ps1}`, which the
PDF surface check uses too). The example opens a main window with the app's own
`window_controls::apply` and a test page that has none of the page's handlers,
so it judges the native layer alone. It right-clicks a blank area, a row, a
button, an image, a text field and a selection and logs every item each native
menu offered (the filter's own log, `apply(.., log_items: true)`), waiting up to
10 s for each menu, since the first one on a text field waits on the OS's text
services (1.9 to 3.5 s on a freshly started GitHub macOS runner), presses every
key 008 FR-012 names and the mouse's back and forward buttons with text typed in a
field, and on macOS presses ⌃⌘D on a selection. It fails (exit 4) if a menu
offered an item outside 008 FR-011's list or appeared where none belongs, a Look Up
panel opened, or the page reloaded or navigated or lost the typed text. Run it
on every OS before a pull request that touches `window_controls/` or the
browser controls, and whenever the web view engine is updated:

- **Linux:** `scripts/dev-container.sh scripts/window-controls-check.sh`, under
  Xvfb with XTest input and throwaway `HOME` and `XDG_*` folders.
- **macOS:** `scripts/macos/window-controls-check.sh`, from a Mac with the tart
  VM set up, as the PDF surface check runs. AppKit's AutoFill is not in the log
  (it is added outside the menu's items) and is allowed (spec.md
  Clarifications 2026-10-11).
- **Windows:** `powershell -ExecutionPolicy Bypass -File
  scripts\windows\window-controls-check.ps1`, in the signed-in desktop session
  (over SSH, through `scripts\windows\gui.ps1`), at 100% scaling.

Logs and screenshots go to `e2e/screenshots-out/window-controls/`. Exit codes:
0 passed, 1 no window, 3 usage (or a build with debug assertions), 4 a check
failed. Two unit tests in `window_controls` say which builds install the filters: the
E2E build does (`cargo test --profile e2e --features e2e --manifest-path
src-tauri/Cargo.toml window_controls`) and a plain debug build does not (the
default `cargo test`).

## Drop check

A file drop reaches `add_photo_from_path` and `add_document_from_path` only if
the main web view's native drop recorded it, and a drag that starts in the app
is refused (`src-tauri/src/drops/`, [File drops](../architecture/frontend.md)).
E2E sends its drops itself, so only this check shows a real one. One script per
OS builds `src-tauri/examples/drop_check.rs` without the `e2e` feature and
drives it with real input (`real_input.{py,swift,ps1}`'s `drag`): a file
dragged from another process's window must be attachable, and links dragged
out of the page, out of a second web view and back in from outside the window
must not be, nor reach the recorder as a drop at all (the example's header
lists every case). Run it on every OS before
a pull request that touches `drops/` or how drops are recorded, and whenever
the web view engine is updated:

- **Linux:** `scripts/dev-container.sh scripts/drop-check.sh`, under Xvfb with
  XTest input and throwaway `HOME` and `XDG_*` folders.
- **macOS:** `scripts/macos/drop-check.sh`, from a Mac with the tart VM set up,
  as the PDF surface check runs.
- **Windows:** `powershell -ExecutionPolicy Bypass -File
  scripts\windows\drop-check.ps1 [-App EXE]`, in the signed-in desktop session
  (over SSH, through `scripts\windows\gui.ps1`), at 100% scaling.

On Windows and macOS the script then runs a second part,
`src-tauri/examples/app_drop_check.rs` (#207): the app itself, as a release
builds it (`npm run build`, then `cargo build --release --features
custom-protocol`), or the one `-App` (Windows) or `DROP_CHECK_APP` (macOS)
names, such as an installed one. It opens a database of one firearm, presses
its way to that firearm's page through the accessibility tree (UI Automation,
or AppKit's), drags a `.jpg` and a `.pdf` there, closes the app and reads the
database back: both must be attached. Only this part drops onto the app's own
page. On Windows the files come from File Explorer
(`examples/explorer_source.ps1`), whose drag data isn't the stand-in source's
(it answers `S_FALSE` for a format it lacks); on macOS from
`drag_source.swift`, since the refusal there asks for the drag's source, not
its data. On Windows, run it as a user who doesn't use HoploDex: a build
without `e2e` keeps its settings in the user's real `%APPDATA%`
([test isolation](test-isolation.md) says how the script guards them). Linux
has no second part: the dev container's Xvfb session has no
accessibility bus to find the page's controls with.

Logs and screenshots go to `e2e/screenshots-out/drop-check/`. Exit codes: 0
passed, 1 no window (or, in the app part, no database or no way to the
firearm's page), 3 usage, 4 a check failed, 5 (Windows) the settings folder
is someone's.

## WebKit's sandbox on a Linux host

On Linux the app turns on WebKit's web-process sandbox (bubblewrap) for the
whole app, but only where a probe shows it works: at start-up it runs itself
once as `hoplodex --webkit-sandbox-probe` and sets `WEBKIT_FORCE_SANDBOX=1`
only if that child loads a page. A host needs `bubblewrap` and `xdg-dbus-proxy`
(and no AppArmor rule that restricts user namespaces for it); without them the app runs unsandboxed and logs why. The
dev container never gets the sandbox: podman blocks the `/proc` mount bubblewrap
needs, and WebKit skips its sandbox where `/run/.containerenv` exists, so the
probe doesn't run there either. So the container's E2E and surface-check runs
are unsandboxed. Before merging a change to the preview, run the E2E suite once
on a Linux host (`npm run build && npm run test:e2e`) and the surface check
there (`scripts/pdf-surface-check.sh`, without the container) and note the
result in the pull request. A Flatpak runs the same probe (WebKit sandboxes
there through `flatpak-spawn --sandbox`); only a container skips it.

A checkout you also use with the container has an empty `node_modules` on the
host, since the container keeps its own in a volume mounted there, and the
host can't write to it. Run the host's E2E from a second copy of the checkout
(`git ls-files` plus `dist/`, then `npm ci` there; point its `src-tauri/target`
at a host build folder), not from this one. To see that a run is sandboxed,
look at the main window's `WebKitWebProcess`: its `/proc/<pid>/ns/pid` and
`ns/mnt` differ from the shell's. The app's log line isn't in the E2E output.
The probe's child takes about 165 ms (the first run after boot about 315 ms),
against 158 ms for the same child with the sandbox off.
