# Document preview

_Part of the [architecture docs](../architecture.md)._

`services/preview/` (feature 007) shows a document without writing it to disk.

`Preview` lives in `OpenDatabase`, so closing or locking the database closes the
preview.

The `hdpreview` protocol (`protocol_handler.rs`) serves one document's bytes by
token to the web views, which have no file access.

A PDF is shown by the OS web view's own viewer in a child web view over the page
area (`surface/`, one file per OS plus `frame_script.js`; no network, no IPC
reach, no toolbar actions).

`availability.rs` holds `PdfAvailability` and the hold (`pdfPreviewHold` in
`machine.json`: set on macOS when the watch catches a copy of a document, it
keeps PDF preview off until the app is updated).

A TIFF is decoded in a confined child, `hoplodex --render-helper` (`helper.rs`;
the confinement and framing are `services/helper/`'s), and text in `text.rs`.
The child answers a page with raw pixels: `helper_handle.rs` checks them against
the request and encodes the PNG, so what reaches the web view is the app's own.

On Linux `sandbox_probe.rs` runs the app once with `--webkit-sandbox-probe` and
turns WebKit's sandbox on only where it works; only a container skips it, and a
Flatpak runs it (WebKit sandboxes there through `flatpak-spawn --sandbox`),
where `sandbox_probe::name_default_application` gives the process a default
`GApplication` with the app id whenever WebKit's sandbox is on, because the
Flatpak portal refuses the a11y bus name WebKit otherwise picks (tao's
`enableGTKAppId` is not used: a second HoploDex would hand over to the first and
exit).

`commands/preview.rs` holds `open_preview`, `set_preview_bounds`,
`focus_preview`, `render_preview_page` and `close_preview`.

Read `specs/007-document-preview/research.md` before changing any of it.
