# Phase 0 Research: Document Preview and Consent Before Opening Externally

The spec leaves these to planning:
- how to show PDF and TIFF;
- where the code that reads a document runs, and what it can reach;
- how the consent step is made trustworthy;
- how document names become searchable.

Three rounds of decisions shape the answers:
- **2026-10-03, issue #13's security findings**: the confirmation is the
  operating system's dialog; documents take an allowlist of document types,
  checked by content; the only image type documents still take is TIFF.
- **2026-10-03, the PDF engine**: the owner rejected bundling PDFium, a
  binary someone else built (the first version of this plan). The
  alternative, the web view's own PDF viewer, was spiked on Linux, macOS
  and Windows (`spike-webview-pdf.md`) and passed on all three.
- **2026-10-06, spec clarifications after the spike**: a PDF gets its
  computer's viewer with that viewer's own controls; the viewer's Save and
  Print are off; WebKit's sandbox is used on Linux where it works and never
  forced; if a copy is caught on disk, PDF preview stops on that computer
  until HoploDex is updated (FR-003a).

All of these are in spec.md. Every NEEDS CLARIFICATION in the Technical
Context is resolved below. Section numbers are cited by the other
artifacts.

## 1. Threat model: where the trust boundaries are

- **Decision**: Four places handle a document, and each is held to what it
  needs. Every boundary is enforced by the Rust side, never by the web view
  alone.
  1. **The main web view** holds the IPC bridge, so every command and the
     whole open collection. It never receives a PDF's or a TIFF's bytes. It
     gets only:
     - PNG bitmaps of TIFF pages that HoploDex encoded itself (§11);
     - a text document's decoded text, as a plain string (§12).
  2. **The PDF surface** is a second web view inside the main window
     (§4) that shows a PDF with the web view's own viewer (§3). It holds
     the one document it shows and nothing else:
     - **no commands**: an app ACL manifest makes every command need a
       permission that only the main web view has (§5);
     - **no network**: a content filter, a proxy to a port HoploDex holds,
       and a navigation allowlist (§6);
     - **no disk**: the document is served from memory, and the viewer's
       Save, Print and downloads are off (§7);
     - **nothing runs**: the viewer's scripting is off where it has a
       switch, and links, new windows and context menus are refused (§8);
     - **WebKit's sandbox** on Linux where it works (§9).
  3. **The TIFF helper** is the HoploDex executable restarted to decode
     TIFF pages (§11). It has no database connection, no key, no IPC, and
     where the OS allows, no files and no network.
  4. **Handing a document to another program** is a native decision
     (§16). Rust checks the document is a document type whose content
     matches (§2), shows the operating system's confirmation naming that
     exact document, and only on a "yes" there writes the copy and calls
     the opener. The web view can ask for an external open but can't
     confirm one.
- **Rationale**:
  - Finding 1 (High): `open_document` hands arbitrary bytes to
    `ShellExecuteExW`, and a prompt the web view draws can be answered by
    a compromised web view.
  - Finding 2 (Medium): HTML and SVG load remote content in the external
    viewer, and HTML named `.pdf` was taken for a PDF.
  - FR-004: a document must not reach "the application's data or
    commands". Those live in the main web view, so nothing that parses a
    PDF or a TIFF runs there.
- **Residual risks, accepted and recorded for the release security review
  (#21)**:
  - **macOS**: WebKit's PDF code runs in the PDF surface's own web
    process, where Tauri injects its IPC script. The ACL manifest is what
    refuses its commands (§5). An exploit of WebKit itself that escaped
    the web process is outside what HoploDex can contain.
  - **Linux without WebKit's sandbox** (in a container, inside Flatpak,
    or where the system forbids it): a PDF that exploited PDF.js or
    WebKit would run with the user's access to their files, as the main
    window's own web process does (spec edge case).
  - **Windows**: Edge's viewer runs in Chromium's own sandbox, which
    WebView2 always uses.
  - **"Open in another app" set**: a compromised main web view could open
    the session's other documents after the user's first confirmation
    (spec edge case). They are all document types with matching content,
    and each copy is marked as untrusted (§18).
  - The main web view decodes the PNGs HoploDex makes. These are
    well-formed, produced by the `png` crate from a bitmap.

## 2. Document types: one allowlist, checked by content, at attach and at open

- **Decision**: A new pure service, `services::document_types`, holds the
  one list, and `classify(filename, bytes) -> Result<DocumentType,
  Refusal>` is the one check. The list:

  | Type | Extensions | Content signature checked | Previewed as |
  |---|---|---|---|
  | PDF | `.pdf` | `%PDF-` within the first 1024 bytes | `pdf` |
  | TIFF | `.tif`, `.tiff` | `II*\0`, `MM\0*` (or BigTIFF `II+\0`, `MM\0+`) | `tiff` |
  | Plain text | `.txt` | not markup (below) | `text` |
  | CSV | `.csv` | not markup | `text` |
  | RTF | `.rtf` | `{\rtf` | not previewed |
  | Word | `.doc` | OLE compound file with a `WordDocument` stream, no `Macros` / `_VBA_PROJECT_CUR` storage | not previewed |
  | Word | `.docx` | ZIP with `[Content_Types].xml` whose main part is `…wordprocessingml.document.main+xml`, and no `vbaProject.bin` | not previewed |
  | Spreadsheet | `.xls` | OLE compound file with a `Workbook` or `Book` stream, no `_VBA_PROJECT_CUR` | not previewed |
  | Spreadsheet | `.xlsx` | ZIP, main part `…spreadsheetml.sheet.main+xml`, no `vbaProject.bin` | not previewed |
  | OpenDocument | `.odt`, `.ods` | ZIP whose first entry `mimetype` is `application/vnd.oasis.opendocument.text` / `.spreadsheet`, and no `Basic/` scripts | not previewed |

  **What "not markup" means**: after an optional BOM and whitespace, the
  first 512 bytes don't start with `<`, followed by `!DOCTYPE`, `html`,
  `svg`, `?xml`, `script` or `head`, compared case-insensitively. That is
  enough to stop the OS from sniffing the file as a web page.

  **Results**:
  - The recorded `mime_type` is the type's canonical one, whatever the file
    chooser supplied, so `add_document` loses its `mimeType` argument.
  - The refusals are `DOCUMENT_TYPE_NOT_ALLOWED` (with a photo-specific
    message for `.jpg`/`.jpeg`/`.png`) and `DOCUMENT_CONTENT_MISMATCH`.

  **Where it runs**:
  - in `add_document` and `add_document_from_path` (FR-016);
  - again at `open_preview` (FR-001) and `open_document` (FR-017), since a
    row stored before this feature, or an edited file, may not pass.
- **Rationale**: The owner's answer is that documents are for paperwork
  only, not arbitrary scripts or programs, and photos go under Photos.
  - An allowlist is the only shape that answers finding 1. A denylist of
    executable extensions is never complete (`.scr`, `.cpl`, `.appref-ms`,
    `.settingcontent-ms`, …).
  - Checking content answers finding 2's "HTML named `.pdf`" case. The
    open-time re-check means a compromised web view that writes a row
    some other way still can't reach the opener with it.
  - Macro-enabled Office content is refused by content, not just by
    extension, because renaming `.docm` to `.docx` keeps the macros.
  - The checks are signature-level. They don't parse the file, so they run
    safely in the main process.
- **Crates**: `zip` and `cfb` are already in the tree through `calamine`
  (Cargo.lock: `zip` 7.2/8.6, `cfb` 0.7.3). They are promoted to direct
  dependencies at the locked versions, as `zeroize` and `libc` were. Both
  only read directory structures here, never decompress content, so a ZIP
  bomb costs nothing.
- **Alternatives considered**:
  - Decide by the stored MIME type: the file chooser's `File.type` is
    whatever the OS says, and a compromised web view says anything.
  - Keep accepting anything and gate only the opener: the owner rejected
    arbitrary attachments outright.
  - Parse fully at attach (for example, open the PDF): adds nothing to the
    security boundary, since preview checks again.

## 3. The PDF engine: the web view's own viewer

- **Decision**: Show a PDF with the PDF viewer the web view already has
  (`spike-webview-pdf.md`, "What each web view has"):

  | OS | Viewer | Where it runs |
  |---|---|---|
  | Linux | PDF.js, bundled in WebKitGTK since 2.40 | a cross-origin frame (`webkit-pdfjs-viewer://pdfjs`) without Tauri's IPC |
  | macOS | WebKit's PDF plugin, on PDFKit | the top frame of the PDF surface's web view |
  | Windows | Edge's viewer (PDFium in Chromium's sandbox) | extension frames in processes of their own, without Tauri's IPC |

  Nothing is bundled: PDF.js ships inside the WebKitGTK HoploDex already
  needs (the AppImage bundles Debian's 2.54), and on macOS and Windows the
  viewer is the OS's, updated with it.

  **What the viewer brings**: every page, page count, zoom and fit,
  find, selecting and copying text, the screen-reader tree (seen in AT-SPI,
  the AX tree and UI Automation in the spike), and a protected PDF's
  password prompt. These differ by OS (FR-002, FR-005). HoploDex never
  sees a document's password, because the prompt is the viewer's own.

  **What HoploDex takes away** (§6–§8): Save, Print, downloads, "Open in
  Preview", links, new windows, the network, and on Linux PDF.js's
  scripting and XFA.
- **Rationale**:
  - The owner rejected a third-party binary (PDFium from pdfium-binaries),
    and the spike showed every web view HoploDex runs on has a viewer that
    can be held to FR-003 and FR-004.
  - The viewers are the most used and most fuzzed PDF code on each OS,
    and they are patched with the OS or the runtime, not by a HoploDex
    release.
  - FR-002 was amended (2026-10-06) to let the PDF's own controls differ
    by OS, which is the cost of this choice.
- **Alternatives considered**:
  - **PDFium from pdfium-binaries in a confined helper** (this plan's
    first version): rejected by the owner, a binary someone else built.
  - **PDFium built from source**: hours of Chromium toolchain per target,
    and still a native library HoploDex must patch itself.
  - **pdf.js bundled into the main web view**: it would run where the IPC
    bridge is. CVE-2024-4367 was arbitrary JavaScript through a crafted
    font. Rejected on FR-004.
  - **hayro** (pure Rust): experimental, can't open encrypted PDFs, no
    text for screen readers. Revisit when it matures.
  - **MuPDF**: AGPL-3.0. Rejected.

## 4. The PDF surface: a child web view in the main window

- **Decision**: A PDF is shown in a **second web view placed inside the
  main window**, over the viewer's page area, through Tauri's multiwebview
  API (`Window::add_child`, behind Tauri's `unstable` feature). Everything
  around it (title, details, previous and next, "Open in another app…",
  delete, close) is React in the main web view, as for TIFF and text, so
  FR-002's "what surrounds a document" is the same everywhere.

  **Its label** is `preview`. No capability names it (§5).

  **Its content** is served from memory by a custom protocol,
  `hdpreview`:
  - The URL is `hdpreview://localhost/<token>/document.pdf` (on Windows
    `http://hdpreview.localhost/<token>/document.pdf`, wry's form). The
    token is 128 random bits, new for each document shown, and refused
    once that document is no longer shown.
  - The last segment is always `document.pdf`. On macOS a copy that gets
    out is named after it (`spike-webview-pdf.md`, "If Open in Preview
    gets through"), so the document's name never reaches another app's
    records.
  - The response has `Content-Type: application/pdf`, `Cache-Control:
    no-store` and no `Accept-Ranges`, and holds the whole document. Bytes
    are held `Zeroizing` in the `Preview` (data-model.md) until it closes.
  - Any other path, or a request for a token no longer shown, gets 404.

  **Its geometry**: the viewer's page area is a placeholder element.
  React measures it (`ResizeObserver`, window resize) and sends its
  rectangle in logical pixels with `set_preview_bounds`. Rust sets the
  surface's position and size. The surface is hidden (`hide()`) while
  anything is drawn above the viewer: the delete confirmation, a menu, a
  toast that would overlap it (the `PreviewSurface` component tracks
  this), and while the main window is minimized.

  **Its lifetime**:
  - **Created** when the viewer shows a PDF and has no surface,
    **navigated to the next document's URL** when the viewer moves from
    one PDF straight to another, and **closed** when the viewer moves to a
    TIFF or text document, closes, or the session ends (§20).
  - **Shown only once its document has been served**: the protocol
    handler emits `preview:pdf-ready` after the response body is handed
    over. Until then React shows "Preparing {name}…" in the page area.
  - **No built-in viewer**: a web view without a PDF viewer turns a PDF
    into a download. `on_download` for the surface's own URL is the sign:
    the download is refused, PDF preview is marked unavailable for this
    run (§7), and React shows the "can't be previewed on this computer"
    state.
- **Rationale**:
  - FR-007 makes the preview "a viewer over the record", laid out like
    the photo viewer. A separate window (as in the spike) can't hold the
    React chrome, because the surface must have no IPC.
  - One surface per open viewer, not per document, keeps moving between
    documents within SC-001's second on Windows, where each new web view
    with its own user data folder starts a browser process (§6). The
    previous document's page is unloaded by the navigation, and its bytes
    are zeroized when its token is revoked.
- **Risk and fallback**: `unstable` means the API can change in a Tauri
  minor release; the lock file pins it, and a Tauri update is reviewed
  like any dependency bump. The first task proves a child web view with
  §6's settings (its own user data folder on Windows, a proxy, incognito)
  inside the main window on all three OS. If one of them can't, the
  fallback is an owned, undecorated window kept over the viewer's page
  area (moved and resized with the main window), with the same settings.
  The spike ran in such a window, so every measure below is known to work
  there.
- **Alternatives considered**:
  - **A separate top-level window**: the chrome would have to live in
    another window, which isn't "a viewer over the record".
  - **An `<iframe>` or `<embed>` in the main web view**: on macOS the
    frame is same-origin and reached the parent's `__TAURI_INTERNALS__`
    (spike); on every OS it would put the viewer where the IPC is.

- **Result of the first task, Linux (T006, 2026-10-06)**:
  `examples/pdf_surface_check.rs` in "shows" mode, run by
  `examples/pdf_surface_check.sh` in the dev container under Xvfb (WebKitGTK
  2.54): the child web view `preview` (`Window::add_child`, incognito,
  `proxy_url` to a loopback listener) loaded `hdpreview://localhost/<token>/document.pdf`,
  PDF.js drew the 3-page fixture inside the main window over the requested
  rectangle (reported as x=40, y=100, 920x660, and the main page stayed
  alive around it), and a request the surface made to a name outside the
  app reached the proxy (tripwire hit). So the child web view stands on
  Linux, with two things Tauri doesn't do for us, both handled in
  `surface/linux.rs`:
  - **`add_child` can't place a child web view on Linux.** wry packs it into
    the window's `GtkBox` beside the main web view and ignores its bounds
    (the PDF filled the lower half of the window). The surface moves the main
    web view into a `GtkOverlay`, puts itself over it, and positions itself
    with margins and a size request. A moved web view shows nothing until its
    page next changes, so the other web views are made to repaint. GTK is
    called through its C API, since `gtk` isn't a dependency of this crate
    (adding `gtk = "0.18"`, already in the tree, would make it safe Rust).
  - **An incognito web view with no `data_directory` of its own gets no custom
    protocols.** tauri-runtime-wry registers a protocol once for each context
    key, on the first web view with that key, and an incognito web view builds
    a context of its own that the registration never reaches ("The URL can't
    be shown"). The surface's `data_directory` (a folder it never uses on
    Linux) is the key that fixes it.
- **Result of the first task, macOS (T007, 2026-10-06)**: the same example
  in "shows" mode, run through `scripts/tart-vm.sh gui` in the macOS 26
  (Tahoe) VM as the standard test user (WebKit of that release): exit 0, with
  no change to `surface/macos.rs` or `surface/mod.rs`. The child web view
  `preview` (`Window::add_child`, incognito, `proxy_url` to the loopback
  listener) loaded `hdpreview://localhost/<token>/document.pdf` (page load
  Started and Finished, 2.3 s after the surface was built, mostly the 2 s
  the example waits for the window), and `surface.bounds()` reported x=40,
  y=100, 920x660, the rectangle asked for. The screenshot
  (`e2e/screenshots-out/pdf-surface/shows-macos.png`, the whole 1920x4200
  screen) shows WebKit's own PDF viewer (white page, "Page 1 of 3", its
  scroll bar) drawn inside the main window's red outline, the main page's
  banner above it, and the main page still running (4 of its 1 s ticks). A
  system dialog (the VM's screen-recording prompt for `sshd`, raised by
  `screencapture`) and the VM's leftover Terminal windows cover part of it;
  they aren't the app's. So the child web view stands on macOS, with nothing
  needed beyond `add_child`'s own placement (no overlay, unlike Linux), and
  the protocol worked without a `data_directory` (macOS ignores it).
  - **The proxy probe did run.** A PDF document on WebKit is still a
    document into which `eval` runs script (WKWebView's `evaluateJavaScript`
    goes to the main frame, which for a PDF is a generated page holding the
    viewer), so the example's `new Image().src = 'http://…invalid/…'`
    reached the proxy: `tripwire hits=1`. A control run with `--no-probe`
    (the example's new flag, which skips that request) gave `tripwire
    hits=0`, so the one hit is the probe's and the surface makes no other
    request of its own. That shows `proxy_url` is applied to the surface's
    own network on macOS for a non-loopback name (as §6 says, a loopback one
    bypasses the proxy there).

- **Result of the first task, Windows (T008, 2026-10-06)**: the same example,
  `cargo run --example pdf_surface_check -- shows`, on Windows Server 2025
  (WebView2 runtime 154.0.4258.62, no code change to `surface/windows.rs` or
  the example): exit 0. The child web view loaded
  `http://hdpreview.localhost/<token>/document.pdf` (WebView2 serves the
  scheme as `hdpreview://localhost/…` to the handler, and page-load Started and
  Finished report the `http://` form, which is what `on_navigation` compares),
  served in 2.4 s from the surface's build, and Edge's PDF viewer (its toolbar,
  "1 of 3", page 1's text) drew inside the main window exactly within the
  page's red outline (`e2e/screenshots-out/pdf-surface/shows-windows.png`;
  `bounds()` reported x=40, y=100, 920x660 logical, and the main page kept
  ticking, 6 pings). Two browser processes, listed both by the example and
  independently by polling `Win32_Process` while it ran, both children of
  the example's process:
  - main window (pid 8012): `--user-data-dir=<scratch>\cache\main-webview\EBWebView`
    with wry's default arguments only;
  - surface (pid 6020): `--user-data-dir=<scratch>\cache\preview-webview2\EBWebView
    --disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection
    --proxy-bypass-list=<-loopback> --proxy-server=http://127.0.0.1:<port>
    --webrtc-ip-handling-policy=disable_non_proxied_udp`.
  Neither used the default folder under `%LOCALAPPDATA%`. The tripwire was hit
  5 times (two while the surface's browser started, before the document was
  requested, not attributed to a request; the others after the document
  loaded, one of them the probe image to a name outside the app): everything
  the surface's browser process sent out went to the proxy and no further. The
  viewer's `favicon.ico` request reached the app's own protocol handler (404).
  So the child web view stands on Windows.
- **Decision after the first task (T009, 2026-10-06)**: the child web view
  passed on Linux, macOS and Windows, so it stands and the owned-window
  fallback is not needed.

## 5. Keeping the surface unprivileged: the app ACL manifest

- **Decision**: `build.rs` gives `tauri_build` an app manifest
  (`Attributes::app_manifest(AppManifest::new().commands(&COMMANDS))`)
  listing every HoploDex command. Tauri then refuses any command a
  window's capabilities don't allow. `capabilities/default.json` (web view
  `main`, by `webviews`, not `windows`) allows each one; no capability names
  `preview`. A capability's `windows` reaches every web view in the window,
  and the surface is a child web view of `main`: the macOS surface check
  found the surface's page answered with `windows: ["main"]`.
  - **One list**: `COMMANDS` is the single list of command names, used by
    `build.rs`. A unit test parses `main.rs`'s `generate_handler!` and
    `capabilities/default.json` and fails unless all three name the same
    commands, so a new command can't be registered without a permission,
    or allowed without being registered.
  - **Plugins**: `core:default` and `dialog:default` stay on `main` only.
    The surface gets no plugin permission either, so it can't listen to
    or emit events.
- **Rationale**: Tauri counts every registered custom protocol as local,
  and with no app manifest a local page may call every app command. The
  spike's `spike_secret` command answered the surface on all three OS
  until a manifest was added, and was refused with one. On macOS, where
  WebKit's PDF code runs in the frame that has Tauri's IPC script, this is
  the layer that stops a PDF exploit from reaching the collection.
- **Also**: this hardens the whole app, not just the preview. Any other
  page that ever loads in a HoploDex web view is refused too (relevant to
  #73).

## 6. Keeping the surface off the network

- **Decision**: Three layers, each tested on its own in the spike, plus
  WebRTC off. The surface is also `incognito` (WebKit's ephemeral session,
  WebView2's InPrivate), so nothing it fetches is cached.

  | Layer | Linux (WebKitGTK) | macOS (WKWebView) | Windows (WebView2) |
  |---|---|---|---|
  | Content filter: block every URL but `hdpreview:` (and the viewer's own scheme) | `WebKitUserContentFilter` compiled into a store under the app's cache folder (rules only) | `WKContentRuleList`, the same rules, store under the cache folder | `WebResourceRequested` on `*`, refusing every request but the surface's own URL. It sees only the top frame |
  | Proxy to the **tripwire**, a port HoploDex holds | `proxy_url`; loopback goes through it (wry's empty bypass list) | `proxy_url` (Tauri's `macos-proxy` feature); loopback **bypasses** it, so the filter is the layer that matters | browser arguments `--proxy-server=…` and `--proxy-bypass-list=<-loopback>`: **the layer that covers Edge's viewer frames**, which the filter can't see |
  | WebRTC | off by default and absent; set off explicitly | off through WebKit SPI (`-[WKPreferences _setPeerConnectionEnabled:]`), checked at startup (§7) | `--webrtc-ip-handling-policy=disable_non_proxied_udp` |
  | Navigation | `on_navigation` allows only the surface's current URL, `about:blank`, and on Linux `webkit-pdfjs-viewer:`; `on_new_window` denies | same | same |

  **The tripwire** (`services::preview::tripwire`): a `TcpListener` on
  `127.0.0.1:0`, bound when the first PDF is previewed and held for the
  rest of the run. It accepts, reads nothing, and closes each connection.
  It keeps a count for tests and logs once per run that a request was
  refused, without the address, since the address could come from the
  document. On Windows, WebView2's own browser process sends requests
  there too (`config.edge.skype.com`, `edge.microsoft.com`), which is
  expected.

  **Windows' user data folder**: browser arguments belong to the browser
  process, which every web view sharing a user data folder shares, so the
  surface gets a folder of its own (`data_directory`), under the app's
  cache folder (`<cache>/preview-webview2/`). It holds no document content
  (spike) and is deleted at startup and after the surface's browser
  process has exited. wry adds its default browser arguments only when an
  app gives none, so HoploDex writes them in full alongside these.

  **E2E on Windows**: `WEBVIEW2_USER_DATA_FOLDER` overrides every web
  view's folder, which would put the surface back in the main window's
  browser process, where it never loads (spike). So `wdio.conf.ts` stops
  setting it. Instead, the main window is built in `setup()` rather than
  from `tauri.conf.json`, with `data_directory` from `app_dirs` (in an E2E
  build, under `HOPLODEX_E2E_CACHE_HOME`), and the surface's folder comes
  from the same place.
- **Rationale**: FR-004 says nothing may fetch anything from the network.
  The spike showed that each OS leaves a different gap in each layer:
  macOS's proxy lets loopback through, Windows' filter can't see the
  viewer, and WebRTC escapes the filter on macOS. With all layers, nothing
  reached a local test service or the outside on any OS.
- **Alternatives considered**:
  - A proxy to a "closed" port such as 127.0.0.1:9: anything could listen
    there. The tripwire's port can't be taken over on `127.0.0.1` on any
    OS (spike).
  - A `Content-Security-Policy` header on the PDF response: on Linux it
    would apply to the wrapper document holding PDF.js's frame and could
    block the viewer itself, and it doesn't reach the viewer's frames on
    Windows. The filter does the job. Not added.

## 7. Keeping the document off disk

- **Decision**:
  - **Served from memory** (§4), `no-store`, incognito. The spike found no
    file holding the document on any OS with these.
  - **The viewer's own ways out**, per OS:

    | | Linux (PDF.js) | macOS (WebKit) | Windows (Edge) |
    |---|---|---|---|
    | Save / download | the frame script hides PDF.js's download buttons; `on_download` refuses | the HUD's Download does nothing (wry has no handler); the HUD is off anyway | `HiddenPdfToolbarItems` hides Save and Save As; `SaveAsUIShowing` is cancelled; `on_download` refuses |
    | Print | the frame script hides the print buttons and replaces `window.print`; WebKitWebView's `print` signal is answered "handled" from Rust | no print path without the app's menu; HoploDex's menu has no Print | `HiddenPdfToolbarItems` hides Print; `AreBrowserAcceleratorKeysEnabled(false)` stops Ctrl+P |
    | Open in another program | none | **Open in Preview** (the HUD and the context menu): HUD off through SPI (`PDFPluginHUDEnabled`), `contextmenu` cancelled by the frame script | none |
    | Context menu | WebKitWebView's `context-menu` signal keeps only Copy and Select All | cancelled | `ContextMenuRequested` keeps only Copy |
    | Keyboard | the frame script cancels Ctrl+S, Ctrl+P and Ctrl+O | none found | browser accelerator keys off (Ctrl+S, Ctrl+P, F12…) |

  - **The availability check** (`services::preview::pdf_availability`),
    run once at startup, before anything is shown:
    - **macOS**: `+[WKPreferences _features]` lists
      `PDFPluginHUDEnabled`, `-[WKPreferences
      _setEnabled:forFeature:]` and `_setPeerConnectionEnabled:` respond,
      and a `WKPreferences` reads both back as off after they are set.
    - **Windows**: the installed WebView2 runtime
      (`GetAvailableCoreWebView2BrowserVersionString`) is at least the
      first version that has every interface used (`HiddenPdfToolbarItems`,
      `SaveAsUIShowing`, `ContextMenuRequested`, browser accelerator keys).
      When the surface is built, each interface is queried again and its
      absence closes the surface.
    - **Linux**: WebKitGTK is at least 2.40 (PDF.js).

    If the check fails, PDF preview is off for this run
    (`PDF_PREVIEW_UNAVAILABLE`, with "on this computer" wording, FR-003a).
    TIFF and text are unaffected. An E2E build (`e2e` feature only) reads
    `HOPLODEX_E2E_PDF_PREVIEW=off` to fail the check, so the frontend's
    handling can be tested and photographed; `check-no-webdriver.mjs`
    keeps the name out of release binaries, as for the consent seam
    (§16).
  - **The watch (macOS, FR-003a)**: while a PDF is shown, a thread holds
    a `kqueue` on `$TMPDIR` (`EVFILT_VNODE`, `NOTE_WRITE`, through
    `libc`). On each event it lists the folder for `WebKitPDFs-*` entries
    that weren't there when the surface was created, and deletes each at
    once, contents first. The spike's polling delete beat Preview at 4 ms
    and at 50 ms, and lost at 500 ms; a `kqueue` event arrives within
    milliseconds. When it catches one, HoploDex:
    1. deletes it;
    2. records its path in `machine.json`'s `pdfPreviewHold.leftovers`
       until a later sweep finds it gone;
    3. sets the hold to this HoploDex version (§17);
    4. closes the surface and emits `preview:pdf-ended` with reason
       `copyCaught`, so React says what happened (ui contract §3).
  - **Sweeps**: at the surface's close, at startup and on a shutdown
    signal, every path in `leftovers` is deleted and removed from the
    list once gone. Only paths HoploDex recorded are touched, never other
    apps' `WebKitPDFs-*` folders.
  - **Linux and Windows** have no switch that can silently stop working:
    every way out passes through a handler HoploDex answers (`on_download`,
    `SaveAsUIShowing`, the `print` signal). FR-003a's watch is macOS only.
- **Rationale**: FR-003 and FR-003a. The spike found the copy macOS's
  Open in Preview writes is named after the URL's last segment, stays
  after HoploDex quits, and lands in Preview's Recent Documents and other
  system records unless deleted before Preview opens it. The SPI switches
  can vanish in an OS update, hence the check at each start and the watch
  as a backstop.
- **Alternatives considered**:
  - FSEvents for the watch: its latency hasn't been measured against the
    50 ms margin; `kqueue` on one folder is immediate and needs no new
    crate.
  - Sweeping every `WebKitPDFs-*` in `$TMPDIR`: other apps' WKWebViews
    write there too.

## 8. Keeping document content inert

- **Decision**:
  - **Linux, PDF.js**: an `initialization_script_for_all_frames` script
    runs in PDF.js's frame and, at `webviewerloaded` (PDF.js's hook for
    embedders), sets `enableScripting` and `enableXfa` to false and
    `annotationEditorMode` to disabled. It then loads an image from
    `hdpreview://localhost/hooked/<surface secret>` (the secret is 128
    random bits written into the script when the surface is built; the
    filter allows the scheme). On Linux the surface is shown only after
    the protocol handler has seen that request for the current document;
    if it doesn't come within 5 s of the document being served, the
    surface is closed with `PREVIEW_FAILED` rather than shown with
    scripting on.
  - **macOS and Windows**: their viewers have no switch for a PDF's
    JavaScript. The spike saw no script dialog or alert from the test
    PDF's `app.alert` on either. The containment is §5 and §6.
  - **Links** are top-level navigations, refused by `on_navigation`
    (spike, all OS). On Windows, Edge's automatic HTTPS still sends a
    request for the link, which the filter refuses.
  - **Developer tools** are off in a release build (WebView2's
    `AreDevToolsEnabled`, WebKit's `enable-developer-extras` left off).
- **Rationale**: FR-004 and SC-003. Nothing in a document runs, follows a
  link, submits a form or opens an embedded file. The viewers already
  don't open embedded files or launch actions; the network layers make
  form submission and remote content impossible regardless.

## 9. WebKit's sandbox on Linux

- **Decision**: Turn WebKit's web-process sandbox on for the whole app
  where a probe shows it works, and never otherwise.
  - **How it's turned on**: `WEBKIT_FORCE_SANDBOX=1` in the environment
    before the first web context exists. wry creates the surface's context
    and loads its URL inside `build()`, so a Tauri web view can't turn it
    on for itself; the variable covers every web view, the main window's
    included.
  - **The probe**: `main()` checks for `--webkit-sandbox-probe` first,
    like the TIFF helper's argument (§11). Before Tauri starts, the app
    runs itself with that argument and `WEBKIT_FORCE_SANDBOX=1`, with a
    5 s limit. The probe initializes GTK, creates a web view, loads
    `about:blank` and exits 0 on load-finished. Only if it does, `main()`
    sets the variable for itself. A crash (WebKit's `g_error` when bwrap
    can't run), a non-zero exit or a time-out means no sandbox.
  - **Skipped without probing** where WebKit itself won't sandbox: in a
    container (`/run/.containerenv`, `/.dockerenv`) and inside Flatpak
    (`/.flatpak-info`, which has its own sandbox).
  - **Logged once** at startup: whether the sandbox is on, and why not.
- **Rationale**: FR-004: "where the computer offers a sandbox … and
  HoploDex finds it works there, the preview MUST run in it; where it
  doesn't work, HoploDex MUST NOT force it." The spike showed the sandbox
  gives the web process its own user, pid, network and mount namespaces
  and hides `HOME`, and that forcing it where bwrap can't run ends the app
  at its first web view. A real web process in a child is the only probe
  that catches every reason it can fail (bwrap missing, AppArmor's
  user-namespace restriction, the AppImage's library paths, a missing
  `xdg-dbus-proxy`) without risking the app itself.
- **The main window under the sandbox**: its web process reads nothing
  from disk (every file goes through IPC or the custom protocols), and
  file drops and dialogs are handled in the UI process, so nothing it
  does needs `HOME`. The dev container can't run the sandbox, so E2E there
  runs without it. The full E2E suite is run once on a Linux host with the
  sandbox on before merge, and the result recorded in the pull request.
- **Cost**: one child process at startup, measured in §22. A cached result
  isn't used: a "works" answer that went stale (a newly installed AppArmor
  rule) would end the app at start.
- **Alternatives considered**:
  - **A preview web view built outside Tauri** on its own
    `WebKitWebContext` with the sandbox on: confines the surface alone,
    but means reimplementing the custom protocol, navigation and download
    handling in GTK and embedding a foreign widget in wry's window. More
    code for less coverage than sandboxing the main window too.
  - **A plain bwrap probe** (run bwrap with WebKit's flags): fast, but
    misses the AppImage's library paths and `xdg-dbus-proxy`.

## 10. Input in the PDF surface

- **Decision**: The surface takes the keyboard and mouse when the user is
  in it, and no HoploDex script runs in its frames on macOS and Windows.
  Three things must still reach the app, and Rust watches for them on the
  surface's native view:

  | Need | Linux | macOS | Windows |
  |---|---|---|---|
  | **Escape** closes the viewer (FR-007) | `key-press-event` on the `WebKitWebView` widget | a local `NSEvent` monitor, keyDown with the surface's view as first responder | the controller's `AcceleratorKeyPressed` (Escape is an accelerator key) |
  | **F6** moves focus between the surface and the viewer's controls | the same | the same | the same |
  | **Activity** for the idle lock (003 FR-034) | `button-press-event`, `scroll-event`, `key-press-event`, `motion-notify-event` | the same monitor: key, mouse and scroll events | `AcceleratorKeyPressed` for keys, and `GetLastInputInfo` sampled each second while HoploDex is the foreground window |

  - **Escape** and **F6** are consumed and emitted to the main web view
    as `preview:escape` and `preview:focus-chrome`; React closes the
    viewer, or moves focus to the toolbar's first control.
  - **Activity** calls the idle clock's `note_activity` directly, at most
    once a second, as the main web view's listener does.
  - **Into the surface**: F6 or a click in the page area. F6 from React
    calls `focus_preview`, which focuses the surface's web view.
- **Rationale**: Without these, Escape would do nothing while reading a
  PDF, keyboard users couldn't leave it, and scrolling a long PDF would
  count as idle and lock the database (spec edge case: "scrolling, paging
  and zooming are input"). F6 is the usual key for moving between panes.
- **Alternatives considered**: a frame script posting to the protocol
  handler: it runs in PDF.js's frame on Linux, but on macOS the PDF
  plugin swallows the events and on Windows no script reaches Edge's
  frames.

## 11. TIFF: decoded in a helper process

- **Decision**: Decode TIFF with the `tiff` crate (MIT, pure Rust), plus
  the `fax` crate (MIT) for CCITT Group 3 and 4, the usual compression of
  black-and-white scans, in a **render helper**: the HoploDex executable
  started again with a hidden argument.

  **Starting it**:
  - `hoplodex --render-helper`. `main()` checks for that argument first,
    before any Tauri, keyring, session or logging setup, and runs
    `services::preview::helper::run()`, which never returns to the app
    path.
  - It starts with a cleared environment, no inherited handles but its
    three pipes, and `CREATE_NO_WINDOW` on Windows.

  **Protocol** (`preview::protocol`, shared by both ends):
  length-prefixed binary frames over stdin and stdout.
  1. The parent sends one `Load { bytes }`. The helper answers
     `Loaded { pages: [{ width_pt, height_pt }] }` or `Failed { reason }`.
     It walks the IFDs for the page count and sizes, using the DPI tags,
     or 200 DPI when missing.
  2. Then any number of `Render { page, width_px }` → `Page { png }` |
     `PageFailed`. It decodes one page, downsamples it to the requested
     width, and encodes PNG.
  3. On EOF on stdin, it exits. Its stderr is discarded, never logged,
     since it could hold content.

  **Lifetime**: one helper per TIFF shown; started by `open_preview`,
  killed by `close_preview`, by moving to another document, and when the
  `OpenDatabase` is dropped (§20).

  **Failures**:
  - A page with an unsupported compression, photometric or layout fails
    alone with `PREVIEW_PAGE_FAILED`, and the others still show (spec edge
    case).
  - A file whose first IFD can't be read fails as `PREVIEW_DAMAGED`.
  - A crash or time-out on a page fails that page; the helper is
    restarted, at most twice per preview, for the others. A crash during
    `Load` is `PREVIEW_FAILED`.

  **Confinement**, set before the helper reads the document:

  | Measure | Linux | macOS | Windows |
  |---|---|---|---|
  | No file or network access | Landlock: all filesystem rights denied, plus TCP bind/connect on ABI ≥ 4; best effort on older kernels, logged once | `sandbox_init` with the pure-computation profile | process mitigation policies (no child processes, no dynamic code, no remote or low-label image loads) |
  | No new privileges | `PR_SET_NO_NEW_PRIVS` | n/a | n/a |
  | Dies with HoploDex | `PR_SET_PDEATHSIG(SIGKILL)` | stdin EOF | job object with `KILL_ON_JOB_CLOSE` |
  | Memory cap | `RLIMIT_DATA` 2 GiB | time limit only | job memory limit 2 GiB |
  | No child processes | Landlock blocks `execve` of any file | in the profile | job `ACTIVE_PROCESS = 1` |
  | No core dump of document content | `RLIMIT_CORE = 0`, `PR_SET_DUMPABLE 0` | `RLIMIT_CORE = 0` | `SEM_NOGPFAULTERRORBOX`, WER excluded |
  | Time limit | 20 s for `Load`, 10 s per `Render`, enforced by the parent killing it | same | same |

- **Rationale**:
  - The release profile is `panic = "abort"`. The decoders are memory-safe
    but can panic or try huge allocations on hostile input (a TIFF
    claiming a 100,000 × 100,000 page). In-process, either ends the app,
    which the crafted-document edge case forbids. Across a process
    boundary either is one failed page.
  - FR-004 asks for a sandbox around "the process that reads the
    document" where the computer offers one; Landlock, `sandbox_init` and
    mitigation policies are what each OS offers a plain process.
  - FR-003: bytes travel by pipe, and core dumps are off, so a crash
    writes no content.
  - WebKitGTK and WebView2 can't show TIFF at all, so the web view's
    viewer isn't an option, and a decoder of our own makes TIFF look and
    work the same on every OS (FR-002).
  - Using the same executable rather than a Tauri sidecar needs no second
    binary, signing or bundling entry.
- **Alternatives considered**:
  - In-process on a worker thread: a panic aborts the app.
  - `panic = "unwind"` for the whole release: changes every other crash
    path in the app for one decoder, and still doesn't bound memory.
  - `image`'s `tiff` feature: single-page only.
  - libtiff through FFI: C, and a native library.
  - seccomp-bpf on Linux: more brittle across glibc versions for little
    gain over Landlock and the process boundary.
- **Amendment (2026-10-07), dies with HoploDex**: if HoploDex exits, every
  process it spawned goes with it at once, whatever the helper is doing. The
  table's macOS cell, "stdin EOF", is not enough (it waits for a page to
  finish), so on macOS the helper registers a `kqueue` `EVFILT_PROC` /
  `NOTE_EXIT` watch on its parent before `sandbox_init`, checks `getppid()`
  again after registering (to close the race with a parent that died first),
  and a thread blocked on the queue calls `_exit` when the parent exits. The
  wait needs nothing the profile denies, since the queue exists already.
  Linux (`PR_SET_PDEATHSIG`) and Windows (a thread waiting on the parent
  process) already did this; stdin EOF remains the normal way the helper ends.
- **Amendment (2026-10-07), the Windows job object is HoploDex's**: the
  arrangement above for Windows (the helper makes and joins its own job, and
  a thread waits on its parent) is replaced, because a job the helper owns
  closes only when the helper is gone, so the thread was the only thing
  tying the helper to HoploDex, and a thread can't act when HoploDex is
  terminated or crashes. Now HoploDex makes the job (`KILL_ON_JOB_CLOSE`,
  `ACTIVE_PROCESS = 1`, 2 GiB job memory), one per helper, since the
  one-process and memory limits are per job and a restarted helper starts
  while its predecessor is still ending. It holds the only handle (not
  inheritable) in the `HelperHandle`'s process record for the helper's life,
  and starts the helper with `CreateProcessW` and a
  `PROC_THREAD_ATTRIBUTE_JOB_LIST` attribute, so the helper is in the job
  before it runs any code. The same attribute list carries
  `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` (its three standard handles and
  nothing else), and the environment is empty, as before (`helper_job.rs`).
  Any end of HoploDex, including `TerminateProcess`, closes the handle and
  the kernel kills the helper at once (`tests/tiff_helper_test.rs` kills a
  parent and expects the helper gone within a second). The helper checks that
  it is in a job with those limits and refuses to serve otherwise (exit 3),
  and keeps its own mitigation policies, error mode and WER flags. The
  parent-watch thread is dropped: the job covers every case it did.

## 12. Text and CSV

- **Decision**: Decode in the main process (`services::preview::text`).
  - **Encoding**:
    - a UTF-8 BOM is stripped;
    - UTF-16 LE or BE is taken from its BOM;
    - otherwise UTF-8 if valid;
    - otherwise Windows-1252, the usual encoding of legacy spreadsheet
      exports, through `encoding_rs`, already in the tree through
      `calamine`, which replaces what it can't map.
  - **Characters**: C0 and C1 control characters other than tab, line feed
    and carriage return become U+FFFD, so nothing is "interpreted as
    formatting" (FR-005). CR LF and CR are normalized to LF.
  - **Returned**: the whole string, in `open_preview`'s answer.
  - **Shown**: as a React text child of a `<pre>` with `white-space:
    pre-wrap`. It is never set as HTML, never link-detected, and CSV is
    never laid out.
- **Rationale**: This decoding can't fail unsafely (`encoding_rs` is
  total and bounded by input size), so no helper is needed. React escapes
  text children, so the content is inert.
- **Size**: a text document over 1 MB is shown in 1 MB chunks appended as
  the user scrolls, so the first screen is within budget (§22).
- **Alternatives considered**: decoding in the web view with
  `TextDecoder`: it would need the raw bytes in the main web view, which
  §1 rules out.

## 13. What the main web view receives, and the CSP

- **Decision**: The main window's CSP is unchanged. `img-src` already
  allows `blob:`, which TIFF page PNGs use. The main web view gets no
  frames, workers, wasm or fonts from this feature.
  - **TIFF pages**: `<img>` elements with `blob:` URLs from the PNG bytes
    (`tauri::ipc::Response`, binary IPC, as `get_photo_original` does).
    Each URL is revoked when its page leaves the render cache (§14) and
    when the preview closes.
  - **No document-derived link, form or frame** is ever created in the
    main web view.
  - **Tests**: `csp_test.rs` gains a test that `frame-src`, `worker-src`
    (through `default-src`) and `script-src` still forbid frames, workers,
    wasm and eval, so a later change can't loosen them to bring a PDF
    engine into the main web view.
- **Rationale**: FR-004 and finding 2's "disable active content and
  external resource loading" hold by construction for TIFF and text. The
  PDF surface is a separate web view with its own rules (§5–§8).

## 14. TIFF pages, zoom and memory

- **Decision**:
  - **Layout**: a multi-page TIFF is one vertical scroll of every page,
    laid out from the sizes `Loaded` returned (FR-005: "every page").
  - **Rendering**: only pages within one screen of the view, at `css
    width × devicePixelRatio`, capped at 4096 px wide and 24 megapixels;
    beyond the cap the bitmap is scaled up in CSS, so 400% stays bounded.
  - **Zoom**: fit width (the default for a multi-page TIFF), fit page, and
    50–400% in 25% steps. A single-page TIFF has fit (its default), zoom
    in and out, and actual size. The current bitmaps are scaled in CSS at
    once, then re-rendered after 150 ms without further zoom input.
  - **Memory**: at most 8 page bitmaps in the main web view,
    least-recently-shown first out.
  - **Progress**: until page 1 arrives, its slot shows "Preparing page
    1…". Other pages show a placeholder of the right size.
  - **Current page**: the indicator follows the page with the most visible
    area. Next, previous, first and last scroll to that page's top.
- **Rationale**: The large-document edge case must stay responsive and
  closable; rendering only visible pages bounds both the helper's work and
  the web view's memory.

## 15. Accessibility

- **PDF**: the viewer's own accessibility tree. The spike found the page
  text and links in AT-SPI (Linux), the AX tree (macOS) and UI Automation
  (Windows), so screen readers read it, and selection and copying are the
  viewer's (FR-005 as amended on 2026-10-06). The page area placeholder
  in the main web view is a labelled region, "{document name}, PDF", with
  a visually hidden hint that F6 moves into the document; the screen
  reader then continues in the viewer's own tree.
- **TIFF**: each page is an `<img>` with `alt="{name}, page {n} of
  {count}"`; a single-page TIFF has `alt="{name}"`.
- **Text**: the `<pre>` is labelled "{name}" and is readable as text.
- **Every control HoploDex draws** is keyboard-usable (ui contract §7),
  and F6 reaches the PDF surface and leaves it (§10).

## 16. The native confirmation

- **Decision**: `services::consent` defines a `Consent` trait:
  `ask(&self, request: ConsentRequest) -> ConsentAnswer`.
  - **The app's implementation** uses `tauri-plugin-dialog`'s Rust API:
    `app.dialog().message(..).kind(Warning).title(..).buttons(OkCancelCustom("Open in another app", "Cancel"))`,
    parented to the main window, and awaited through a oneshot channel,
    off the session lock.
  - **The dialog's text**: for a document, the title names the document;
    for the setting change, the title states the setting; the body is the
    four FR-009 consequences (ui contract §4).
  - **The PDF surface** is hidden while the dialog is up, so it can't sit
    over the dialog's parent window in a way that hides what it is
    confirming.
  - **The flow**:
    1. Check the type (§2).
    2. Pause the idle clock: the window gets no input while a native
       dialog is up, as with the system file chooser (003 research §15).
    3. Ask.
    4. Resume the idle clock.
    5. Write the copy under the session lock, only if the same database
       session is still open (by its generation) (§18).

    A lock or sleep during the dialog therefore writes nothing
    (`DATABASE_CLOSED`).
  - **Rust tests** pass a fake `Consent` that records the requests and
    answers as told.
  - **The E2E build** (`e2e` feature only) reads
    `HOPLODEX_E2E_CONSENT=open|cancel` and appends each request's title to
    `HOPLODEX_E2E_CONSENT_LOG`. `check-no-webdriver.mjs` gains this seam's
    variable name as a second marker that must be absent from a release
    binary, checked both ways round like the first.
- **Rationale**: The owner chose a native dialog because the web view
  can't answer it (finding 1: "require a trusted native decision for that
  exact attachment"). The text is built in Rust from the stored filename,
  so a compromised web view can't put a different name in front of the
  user. Constitution III's single confirmation pattern is deviated from
  deliberately (spec Assumptions, plan Complexity Tracking).
- **Alternatives considered**:
  - An in-app dialog plus `confirmed: true` to the backend: bypassable.
    Rejected by the owner.
  - Both an in-app explanation and a native yes/no: two dialogs for one
    decision.

## 17. The setting, the session's confirmation, and the PDF preview hold

- **Decision**:
  - **The setting**: `machine.json` gains `documentOpening: "preview" |
    "external"`, with `#[serde(default)]` = `"preview"`, so existing files
    read as the default and `VERSION` stays 1. `MachineSettings` gains
    `document_opening()` and `set_document_opening()`.
  - **The session's confirmation**: `OpenDatabase` gains
    `external_open_confirmed: bool`. It is set only by a "yes" in the
    native dialog for a document, and dies with the `OpenDatabase`, so a
    lock, close or switch forgets it (FR-012). It is never written
    anywhere.
  - **The rule** lives in one function, `needs_consent(setting,
    confirmed_this_session)`: `true` unless `setting == External &&
    confirmed_this_session`. Every external open goes through it.
  - **The PDF preview hold** (FR-003a): `machine.json` gains
    `pdfPreviewHold: { version: string, leftovers: string[] } | null`,
    defaulted to `null`.
    - Set when the macOS watch catches a copy (§7), with this build's
      version (`CARGO_PKG_VERSION`).
    - At startup, a hold whose `version` differs from the running one has
      its `version` cleared, so a different version of HoploDex tries PDF
      preview again; `leftovers` stays until each path is gone.
    - While `version` equals the running one, PDF preview is off and
      PDFs are shown as not previewable on this computer.
- **Rationale**: The setting and the hold belong to the computer, for all
  databases (FR-011, FR-003a, 003 FR-013), which is `machine.json`'s role.
  The flag must not outlive the session, which is `OpenDatabase`'s
  lifetime. Any confirmed open sets it: the user has accepted the
  consequences in this session.
- **Alternatives considered**: the flag in the frontend's session store:
  settable by a compromised web view.

## 18. The external copy

- **Decision**: `open_document` keeps the existing folder
  (`<cache>/opened-documents/<id>/`) and clean-up (001 FR-035, 003 FR-022),
  hardened:
  - **Written under the session lock**: after the dialog, the copy is
    written inside the session's mutex, and only if the same database
    session (by generation) is still open. A close or lock takes the same
    mutex before clearing the folder, so either the copy is written first
    and then cleared, or nothing is written. This settles issue #65,
    where a lock between reading the document and writing its copy left
    the copy on disk. The opener is called after the mutex is released.
  - **The name**: `safe_file_name(stem)` plus **the canonical extension of
    the type found** (§2), never the stored name's extension (FR-017).
  - **Private from creation** (settles issue #71):
    - On Unix, each folder is created with mode `0700` and the file with
      `0600` (`DirBuilder`/`OpenOptions` with `mode`, `create_new`), so
      there is no window where they are readable by others. An existing
      folder is used only if it is a directory (not a symbolic link) owned
      by the user; otherwise the open fails with `INTERNAL_ERROR`.
    - On Windows, the per-document folder gets a protected DACL that
      grants only the current user (`SetSecurityInfo`, through
      `windows-sys`' security features), so nothing inherited from a
      relocated cache folder can widen it.
  - **Marked as untrusted**:
    - on Windows, a `:Zone.Identifier` stream with `ZoneId=3`, so Office
      opens it in Protected View with macros blocked and SmartScreen
      applies;
    - on macOS, a `com.apple.quarantine` attribute.
  - **Reuse**: when the copy already exists with the same length, it is
    reused rather than rewritten (spec edge case). Documents never change
    after attaching, and a file another program holds open on Windows
    can't be rewritten.
  - **No program for the type**: detected from the opener's error
    (`ShellExecuteExW`'s `SE_ERR_NOASSOC`; `open`'s non-zero exit on
    macOS; `xdg-open`'s exit status 3 or 4 on Linux). The copy is securely
    deleted at once and `NO_APP_FOR_DOCUMENT` returned. Per-OS behavior is
    a manual check (quickstart).
    - **Amended 2026-10-07 (Windows, T091 rework)**: `ShellExecuteExW`
      doesn't fail for a type with no program: on Windows Server 2025 it
      succeeds and shows "How do you want to open this file?" (with or
      without `SEE_MASK_FLAG_NO_UI`), which would leave the copy behind and
      break FR-010. So on Windows the check comes first. `Opener::has_app`
      (default `true`, so Linux and macOS still read the opener's exit
      status) is asked for the type's canonical extension before the
      consent dialog and before anything is written; on Windows it is
      `AssocQueryStringW(ASSOCF_INIT_IGNOREUNKNOWN, ASSOCSTR_COMMAND,
      ".<ext>", "open")`, and `0x80070483` (`ERROR_NO_ASSOCIATION`) means no
      app. Any other failure is not taken as "none". Then
      `NO_APP_FOR_DOCUMENT` comes back with no dialog and no copy: "This
      computer has no app that opens {kind} documents." The `31`/`1155`
      mapping of the launcher's error stays as a fallback, and its message
      adds "HoploDex deleted the copy it made.", since a copy had been made.
- **Rationale**: Finding 1's "do not send executable/script types to an
  unrestricted OS opener" is met by §2. The canonical extension closes the
  gap where the OS picks a handler by a misleading name. Zone marking is
  the platform's own "this came from elsewhere" signal for legacy
  `.doc`/`.xls` macros, which §2 can't fully exclude. Owner-only
  permissions make FR-009 (a)'s "anyone using this computer account" true
  rather than "anyone on this computer".

## 19. Searching document names

- **Decision**: A third trigram FTS5 index, `document_names_fts`, in
  `0002_fts5.sql`:
  - **The index**: an external-content table over
    `document_attachments(original_filename)`, with insert, delete and
    update triggers (the update kept for safety; filenames don't change).
  - **Collection search**: `list_firearms` gains
    `OR f.id IN (SELECT d.firearm_id FROM document_attachments d WHERE d.id IN (SELECT rowid FROM document_names_fts WHERE document_names_fts MATCH :query))`.
    The one- and two-character `LIKE` path gains
    `OR EXISTS (SELECT 1 FROM document_attachments d WHERE d.firearm_id = f.id AND d.original_filename LIKE :like ESCAPE '\')`.
  - **Accessories page search**: `list_accessories` gets the same over
    `accessory_id`.
  - **Mounted records**: only the record's own documents are joined, so
    no record is found through the documents of what is mounted on it
    (FR-015).
  - **Deletes**:
    - `delete_document` now calls `db::reclaim_deleted_record`, which
      merges all three indexes, instead of `reclaim_freed_space`, so no
      deleted filename stays in an FTS segment.
    - Deleting a firearm or accessory cascades to its documents. SQLite
      fires the child's delete triggers for foreign-key cascades, which a
      test confirms.
- **Rationale**:
  - A separate index leaves the firearm and accessory indexes alone. Their
    external-content triggers need exact old values, and refreshing them
    from document triggers would mean rebuilding a firearm's whole row
    from a document change.
  - Trigram matching makes "appraisal" find "2024 appraisal.pdf" and "pdf"
    find every PDF (spec edge case).
  - The index is small: one short column, a few documents per record.
- **Alternatives considered**:
  - A `document_names` column on `firearms_fts` and `accessories_fts`: the
    trigger coupling above.
  - `LIKE` only: a scan of every filename per search, which misses the
    500 ms budget at 10,000 + 10,000 records with documents.

## 20. Closing the preview with the session

- **Decision**:
  - **Backend**: the `Preview` lives in `OpenDatabase`. Its kinds hold:
    - PDF: the token, the `Zeroizing` bytes, and the surface's handle, an
      `Arc<dyn PreviewSurface>` that a following PDF in the same viewer
      takes over; when no PDF holds it any more, its `Drop` closes the
      child web view;
    - TIFF: the helper, whose `Drop` kills it;
    - text: nothing beyond the answer already sent.

    Every path that removes the `OpenDatabase` therefore ends it: lock
    (any cause), close, switch, sleep step 1, shutdown, quit. The
    protocol handler finds the bytes through the session, so a request
    after that gets 404. `PreviewSurface` is a trait so session tests run
    with a recorder instead of a web view, as `SessionEvents` does.
  - **The macOS watch** stops when the surface's handle is dropped, after
    a last sweep (§7).
  - **Frontend**: the viewer is inside the collection providers, which
    `SessionProvider` unmounts on any of those (keyed by the open), so the
    preview's chrome leaves the screen before the chooser is drawn
    (SC-006). Its cleanup revokes every `blob:` URL and drops the text.
  - **No hold-up**: closing a web view and killing a helper don't wait
    (FR-014).
  - **Stale requests**: a request that races a close gets
    `DATABASE_CLOSED`, or `PREVIEW_CLOSED` if the preview was replaced, and
    is ignored.
- **Rationale**: This reuses the one mechanism 003 already tests for every
  lock cause, so SC-006 is covered by extending those tests. The surface
  is a native child of the main window, so it is gone from the screen as
  soon as Rust closes it, before the frontend draws the chooser.

## 21. Error codes and events

New stable codes, in the existing `CommandError` shape:

| Code | When |
|---|---|
| `DOCUMENT_TYPE_NOT_ALLOWED` | attach or external open of a type outside FR-016's list (the message names the document types; for JPEG/PNG it says to add it under Photos) |
| `DOCUMENT_CONTENT_MISMATCH` | content doesn't match the extension, a macro-enabled Office file, or markup in a text/CSV file |
| `PREVIEW_UNSUPPORTED` | `open_preview` on a type that isn't previewed (RTF, Word, spreadsheet) |
| `PDF_PREVIEW_UNAVAILABLE` | a PDF while PDF preview is off: the startup check failed, no built-in viewer was found this run, or the hold is set (FR-003a); the message says which in plain words |
| `PREVIEW_DAMAGED` | a TIFF whose first IFD can't be read |
| `PREVIEW_FAILED` | the TIFF helper crashed, timed out or ran out of memory during load, or restarted too often; or the PDF surface couldn't be built or its PDF.js hook didn't run |
| `PREVIEW_PAGE_FAILED` | one TIFF page couldn't be rendered; the others are unaffected |
| `PREVIEW_CLOSED` | a request for a preview that has been closed or replaced |
| `NO_APP_FOR_DOCUMENT` | the OS has no program for the type; the copy was deleted |

A PDF that passes the content check but that the viewer can't show
(damaged, an unsupported feature) is reported by the viewer itself, in its
own words (FR-006). A password-protected PDF gets the viewer's own prompt.

`open_document` and `set_document_opening` answer a cancelled native
confirmation with a result, `{ opened: false }` and `{ changed: false }`,
not an error: cancelling is a normal outcome.

Events from Rust to the main web view (contracts/tauri-commands.md):
`preview:pdf-ready`, `preview:pdf-ended`, `preview:escape`,
`preview:focus-chrome`.

## 22. Performance

| Operation | Budget | Expected | Test |
|---|---|---|---|
| `open_preview` → PDF surface shown, 10 MB | 1 s (SC-001) | read blob ~50 ms; first surface: web view creation ~100–200 ms on Linux and macOS, ~300–500 ms on Windows (a browser process for its own folder); serving 10 MB from memory ~20 ms; first page drawn by the viewer ~200 ms | `performance_test.rs` (open and serve, recording surface, at 10,000 + 10,000); `us13` E2E (time to `preview:pdf-ready`); the surface check (time to first paint, by screenshot) on each OS |
| Next PDF in the same viewer | 1 s | navigation only; no new web view | the surface check |
| `open_preview` → page 1, 10 MB TIFF | 1 s | spawn helper ~30 ms; pipe 10 MB ~10 ms; IFD walk ~5 ms; decode page 1 (LZW or G4, 300 DPI letter) ~150 ms; downsample and PNG ~60 ms | `performance_test.rs` |
| Progress, 150 MB PDF or TIFF | 1 s (SC-001) | "Preparing {name}…" is drawn by React before `open_preview` returns; the viewer's own progress after | `DocumentPreview.test.tsx` |
| `list_firearms` / `list_accessories` search with document names | 500 ms at 10,000 + 10,000 (SC-007) | one more indexed subquery; 30,000 seeded documents | `performance_test.rs` |
| 10 MB text shown | 1 s | decode ~30 ms; first 1 MB chunk | `performance_test.rs` (decode) |
| Linux startup: the sandbox probe | not a budgeted action; under 500 ms on the host | one GTK web process started and stopped | measured on the host and noted in the pull request; over 500 ms is reconsidered |

UI-thread work is limited to setting `<img>` sources and the surface's
bounds. Decoding happens in the helper, PDF rendering in the viewer's own
processes.

## 23. Testing approach

- **What cargo can test without a web view** (real temporary databases,
  isolated folders, the real helper binary through
  `CARGO_BIN_EXE_hoplodex`):
  - `document_types`, the attach and open paths, consent with a fake
    `Consent` and a fake opener, the setting and the hold, search;
  - the TIFF helper, its hostile corpus and its confinement;
  - the `hdpreview` protocol handler (token, 404 after close, headers)
    and the `Preview` lifecycle with a recording `PreviewSurface`;
  - the macOS watch's sweep logic on a scratch folder (the `kqueue` part
    in the macOS surface check);
  - the ACL manifest list against `generate_handler!` and the capability.
- **The PDF surface check** (`src-tauri/examples/pdf_surface_check.rs`)
  replaces the spike's harness. It builds the surface through the app's
  own `services::preview::surface` code, not a copy, in a window under the
  same isolation as E2E, and:
  1. serves a PDF holding a unique marker, a link, a JavaScript open
     action, a remote image, a form with a submit action, an embedded file
     and a launch action;
  2. runs the reach probe from every frame a script reaches (and on
     Windows, from Edge's frames through the DevTools protocol): fetch,
     image, beacon, WebSocket and WebRTC to a local test service and to
     unresolvable names, a raw IPC request, and a command call;
  3. with real input, clicks every toolbar button and context-menu item,
     presses the viewer's shortcuts (Ctrl/⌘+S, Ctrl/⌘+P, Ctrl/⌘+O) and
     the link;
  4. fails if the test service, the outside or (except for WebView2's own
     hosts) the tripwire saw anything, if a command answered, or if any
     file written during the run holds the marker; on macOS also if a
     `WebKitPDFs-*` folder appeared;
  5. records the time to first paint.

  Run by `scripts/pdf-surface-check.sh` in the dev container (Linux, XTest
  input, as the spike), by `scripts/macos/pdf-surface-check.sh` in a macOS
  26 VM, and by `scripts/windows/pdf-surface-check.ps1` on the Windows
  test machine. It is automated on every OS; the Linux run is part of the
  pre-merge commands, and the macOS and Windows runs are done for this
  feature's pull request and whenever the surface code changes.
  - A **macOS variant** leaves the HUD on and checks the watch deletes
    the copy, closes the surface and sets the hold (FR-003a).
- **E2E** (`us13`, WebdriverIO in the dev container): the viewer's chrome,
  keyboard, previous and next, the consent seam, the setting, the lock
  with a PDF open, and a disk scan around a real PDF, TIFF and text
  preview. WebDriver drives the main web view; what happens inside the
  surface is the surface check's.
- **SC-002's disk scan**: a unique 64-byte marker per document; the
  test's isolated `XDG_*`, `TMPDIR`, the system temp folder and, on macOS
  and Windows, the user's temp and cache folders, searched during and
  after.
- **The native dialog**: a fake in Rust tests, the E2E seam, and a manual
  check that the real dialog appears and blocks (quickstart M1).
- **The Linux sandbox**: the probe's decision logic in a unit test (exit
  codes, time-out, the container and Flatpak skips); one full E2E run on a
  host with the sandbox on, before merge.

## 24. Dependencies

| Crate / feature | License | Why | Notes |
|---|---|---|---|
| `tauri` feature `unstable` | (Tauri) | `Window::add_child` for the PDF surface (§4) | no new crate |
| `tauri` feature `macos-proxy` | (Tauri) | `proxy_url` on macOS (§6) | needs macOS 14; the minimum is 26 |
| `tiff` | MIT | TIFF pages (§11) | new |
| `fax` | MIT | CCITT G3/G4 (§11) | new |
| `landlock` (Linux) | MIT OR Apache-2.0 | helper confinement (§11) | new |
| `webkit2gtk` (Linux) | MIT | the surface's filter, settings and signals (§6–§10) | promoted from dev-dependency, at wry's version and features |
| `objc2-web-kit`, `block2`, more `objc2-app-kit` and `objc2-foundation` features (macOS) | MIT | SPI switches, content rule list, `NSEvent` monitor (§6, §7, §10) | promoted from dev-dependency, at the locked versions |
| `webview2-com` (Windows) | MIT | the surface's settings and events (§6–§10) | promoted from dev-dependency, at wry's version |
| `windows-sys` features: `Win32_System_JobObjects`, `Win32_System_Threading`, `Win32_Security`, `Win32_Security_Authorization`, `Win32_System_SystemInformation` | MIT OR Apache-2.0 | helper job object and mitigations (§11), the copy's DACL (§18), `GetLastInputInfo` (§10) | features only |
| `zip`, `cfb`, `encoding_rs`, `png` | MIT / MIT / (Apache-2.0 OR MIT) AND BSD-3-Clause / MIT OR Apache-2.0 | content checks (§2), text (§12), page PNGs (§11) | promoted from transitive, at the locked versions |

No binary is bundled or fetched. Every crate is seen by `cargo deny`.
None collects data or makes network requests. No npm package is added.
