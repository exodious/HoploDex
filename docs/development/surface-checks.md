# PDF surface, window controls and WebKit's sandbox

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
menu offered (the filter's own log, `apply(.., log_items: true)`), presses every
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
