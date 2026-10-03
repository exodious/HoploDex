# Spike: the web view's own PDF viewer (2026-10-03)

**Question**: can HoploDex preview a PDF with the PDF viewer the web view
already has, instead of bundling or linking a PDF engine (research.md §3 and
§6 chose PDFium from pdfium-binaries; the owner doesn't want a binary someone
else built)? The answer must keep issue #13's findings closed and write
nothing to disk unless the user asks for it.

**Answer on Linux: yes**, with four measures (below). macOS and Windows are
still to be tested, each in its own session.

## What each web view has

| OS | Web view | Built-in PDF viewer | Tested |
|---|---|---|---|
| Linux | WebKitGTK | PDF.js, bundled in WebKitGTK since 2.40 (`webkit-pdfjs-viewer://pdfjs/web/viewer.html`). In Debian's 2.54 (which the AppImage bundles) and Arch's 2.52 | Yes, Debian 2.54 in the dev container |
| macOS | WKWebView | PDFKit, the OS's own | No |
| Windows | WebView2 | Edge's viewer (PDFium in Chromium's sandbox), updated with Edge | No |

So nothing new is bundled: on Linux PDF.js already ships inside the bundled
WebKitGTK, and on macOS and Windows the viewer is the OS's. research.md §3's
"Linux's WebKitGTK has none" was wrong.

## The spike

`src-tauri/examples/pdf_spike.rs`, run by `src-tauri/examples/pdf_spike.sh`
in the dev container under Xvfb with throwaway `HOME` and `XDG_*` dirs. It
opens one window, `preview`, that no capability names, on a PDF that a
custom protocol (`hdpreview`) serves from memory with `Cache-Control:
no-store`. The PDF (3.6 MB) holds a marker string, a link to example.com and
a JavaScript open action. Probes in the top frame and in the viewer's frame
report what each can reach; real X input (XTest) clicks the link and the
viewer's Save button. Afterwards every file written during the run is listed
and searched for the marker.

## Results (Linux, WebKitGTK 2.54)

| Check | Result |
|---|---|
| Renders | Yes: both pages, page count, zoom, and toolbar buttons for find, print and Save. Text layer present, so a screen reader can read it |
| Decrypted content on disk | **None.** No file written during the run holds the marker. Files written: WebKit salts, `hsts-storage.sqlite`, the Mesa shader cache, the fontconfig cache, the dconf user db |
| Where the viewer runs | A child frame with origin `webkit-pdfjs-viewer://pdfjs`. The top frame is our `hdpreview://localhost/doc.pdf` |
| Viewer frame → Tauri IPC | **Blocked.** No `__TAURI_INTERNALS__` (Tauri injects its scripts and the invoke key into the main frame only, `tauri/src/manager/webview.rs:162`). `parent` is cross-origin (`SecurityError`). A raw `ipc://` fetch fails. A `postMessage` to the IPC handler without the key does nothing (the key is 128 random bits, `tauri/src/lib.rs:1250`) |
| Top frame → app commands | **Allowed without an app manifest.** Tauri counts any registered custom protocol as local (`webview/mod.rs:1698`), and with no app ACL manifest a local page may call every app command (`webview/mod.rs:1823`). Plugin commands were refused |
| Top frame → app commands, with an app manifest | **Refused** (`spike_secret not allowed`) |
| Network from either frame | **Reachable** by default (`fetch` to example.com went out). **Blocked** with the web view's proxy set to a closed port (`proxy_url("http://127.0.0.1:9")`) |
| Clicking the PDF's link | A top-level navigation to `http://example.com/link-target`, refused by `on_navigation` |
| The PDF's JavaScript | PDF.js has `enableScripting` on and ships `pdf.sandbox.mjs`. The test PDF's `app.alert` did not reach `window.alert` in any run, but that isn't proof it can't run. Setting `enableScripting` and `enableXfa` to false from a frame script at `webviewerloaded` (PDF.js's hook for embedders) works |
| Save | A navigation to a `blob:webkit-pdfjs-viewer://pdfjs/…` URL. If allowed, Tauri's `on_download` fires; its default destination was the working directory (`/workspace/src-tauri/document.pdf`), so it must be handled, not left to the default. With a destination set, the file was written there |
| Print | Not tried (the GTK print dialog; a user's own choice) |

## What a design on this needs

1. **An app ACL manifest** (`tauri_build::AppManifest` in `build.rs`) so
   every HoploDex command needs a permission, granted only to the main
   window. Without it the preview window's top frame can call any command.
   This is hardening for the whole app, not just the preview.
2. **The preview web view**: `incognito`, `proxy_url` to a closed port,
   `on_navigation` allowing only the preview's own URL and the viewer's,
   `on_new_window` denying, and `on_download` asking with the OS's save
   dialog (so Save writes only where the user picks) or denying.
3. **A frame script** (`initialization_script_for_all_frames`) that turns off
   PDF.js scripting and XFA at `webviewerloaded`.
4. **Serve from memory** through a custom protocol with
   `Cache-Control: no-store`; a one-time path per preview so nothing else can
   fetch it.

TIFF (which no web view but WebKit on macOS shows) and text keep the plan's
own decoders.

## What the macOS and Windows sessions need to check

Run `cargo run --example pdf_spike` (no Xvfb; click the link and Save by
hand) with `SPIKE_PROXY=1 SPIKE_ALLOW_BLOB=1`, and for each:
- Does the viewer show a PDF from a custom protocol? (Windows serves it as
  `http://hdpreview.localhost/`.) What URL and origin does the viewer frame
  report, and are any extra navigations needed (allow and log them)?
- Can the viewer's frame reach IPC (`internals`, `postMessage`, `rawIpc`)?
  On Windows, check whether WebView2's PDF viewer is a script context the
  frame script reaches at all.
- Does `proxy_url` block the network? (macOS needs Tauri's `macos-proxy`
  feature and macOS 14; WebView2 takes it as a browser argument.)
- Does anything land on disk? Look for the marker under the app's data and
  cache dirs (WebView2's user data folder especially) and the temp dir.
- Does the PDF's JavaScript run (an alert reading "HD SPIKE PDF JAVASCRIPT
  RAN"), and if so, can it be turned off?
- Where does Save go, and does `on_download` fire?
- The app manifest result is the same on every OS (Tauri's own code), so it
  needs no retest.
