# Phase 0 Research: Document Preview and Consent Before Opening Externally

The spec leaves four things to planning:
- how to render PDF and TIFF;
- where the rendering runs;
- how the consent step is made trustworthy;
- how document names become searchable.

Issue #13's two security findings (spec.md, "Security findings posted on
the issue") changed the second and third answers. On 2026-10-03 the owner
decided that:
- the confirmation is the operating system's dialog;
- documents take an allowlist of document types, checked by content;
- the only image type documents still take is TIFF.

Those decisions are folded into spec.md (FR-008, FR-009, FR-012, FR-016,
FR-017, SC-008) and shape §1, §2, §12 and §14 below.

Every NEEDS CLARIFICATION in the Technical Context is resolved here.

## 1. Threat model: where the trust boundary is

- **Decision**: Two boundaries, each enforced by the Rust side, never by the
  web view alone.
  1. **Document content never reaches the privileged web view as
     something it interprets.** The web view holds the IPC bridge, and so
     every command and the whole open collection. It receives:
     - PNG bitmaps that HoploDex itself encoded from rendered pages;
     - page text as plain strings;
     - decoded text as a plain string.

     It never receives a PDF's or TIFF's bytes, so no parser of untrusted
     structure runs in it. Every parser of untrusted structure (PDFium,
     the TIFF and fax decoders) runs in a separate **render helper
     process** (§4) that has no database connection, no key, no IPC
     bridge, no network and, where the OS allows, no file access (§5).
  2. **Handing a document to another program is a native decision.** The
     Rust side does all of it:
     - it checks the document is a document type whose content matches
       (§2);
     - it shows the operating system's confirmation naming that exact
       document (§12);
     - it writes the copy and calls the opener only on a "yes" in that
       dialog.

     The web view can ask for an external open but cannot confirm one.
- **Rationale**: Finding 1 (High) is that `open_document` hands arbitrary
  bytes to `ShellExecuteExW`, and that a React-only prompt is bypassable by
  a compromised web view. Finding 2 (Medium) is that HTML and SVG load
  remote content in the external viewer, and that HTML named `.pdf` was
  detected as HTML by the system opener. FR-004 adds that a document must
  not reach "the application's data or commands". The web view is exactly
  where those live, so nothing that parses a document may run there.
- **Residual risks, accepted and recorded for the release security review
  (#21)**:
  - The web view still decodes the PNGs HoploDex makes. These are
    well-formed, produced by the `png` crate from a bitmap.
  - With "Open in another app" set, a compromised web view could open the
    session's other documents after the user's first confirmation (spec
    edge case). They are all document types with matching content, and
    each copy is marked as untrusted (§14).
  - Rendering bugs in PDFium are contained to the helper.

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
  - Parse fully at attach (for example, open the PDF): needs the helper at
    attach time and adds nothing to the security boundary, since preview
    checks again.

## 3. The PDF engine: PDFium through `pdfium-render`

- **Decision**: Render PDFs with PDFium (Google's PDF engine, as in
  Chrome), driven by the `pdfium-render` crate (MIT OR Apache-2.0,
  0.9.x). It's bound at runtime to a PDFium shared library that HoploDex
  ships (§6). Only the helper process loads it (§4).

  It gives us:
  - page count and page sizes without rendering;
  - rendering a page at a pixel size;
  - each page's text in reading order (`PdfPageText`), for screen readers;
  - a distinct error for a password-protected file (FR-006);
  - "damaged" errors for files it can't parse.

  The build has **no V8 and no XFA**: pdfium-binaries publishes those as
  separate variants, and we take the plain one. So a PDF's JavaScript and
  XFA forms cannot run at all. Links, form fields, embedded files and
  launch actions are just not acted on: HoploDex calls only load, size,
  render and text.
- **Rationale**:
  - It is mature, fast (SC-001's first page in 1 s at 10 MB), heavily
    fuzzed, and the same on every OS (FR-002).
  - It reports exactly the conditions FR-006 names.
  - It is a C++ engine, so it is never run in-process (§4).
- **Alternatives considered**:
  - **pdf.js in the web view**: the most common choice for web apps.
    - It runs inside the privileged web view, so a pdf.js flaw is a flaw
      with the IPC bridge in reach. CVE-2024-4367 was exactly that:
      arbitrary JavaScript through a crafted font.
    - pdf.js 5 also needs `'wasm-unsafe-eval'` in `script-src` for its
      JPEG 2000 and color decoders. That loosens `csp_test.rs`'s policy;
      without it, those decoders fall back to JavaScript about 3× slower.
    - Isolating it in a sandboxed `<iframe>` is unreliable in Tauri: wry
      can inject `__TAURI_INTERNALS__` into subframes, and frames inherit
      the window's ACL (GHSA-57fm-592m-34r7). The CSP's `frame-src 'none'`
      would also have to be loosened.

    Rejected on FR-004.
  - **hayro** (pure Rust, Apache-2.0/MIT, 0.7.1 June 2026): memory-safe
    and attractive, but self-described as experimental with no performance
    work done. It doesn't support encrypted PDFs, so it can't even report
    FR-006's "protected by its own password". Text extraction is not
    offered. Revisit when it matures. It would slot into the same helper
    (§4), so a later switch changes one module.
  - **MuPDF**: AGPL-3.0, which brings network-use terms into the combined
    work and a licensing question we don't need. Rejected.
  - **The OS's own PDF view** (WebView2's PDF viewer, macOS PDFKit): it
    differs per OS (FR-002), Linux's WebKitGTK has none, and on Windows it
    needs the PDF loaded by URL into the privileged web view. Rejected.

## 4. Isolation: the render helper process

- **Decision**: The HoploDex executable started again as a helper.

  **Starting it**:
  - It is started as `hoplodex --render-helper <pdfium library path>`.
    `main()` checks for that argument first, before any Tauri, keyring,
    session or logging setup, and runs `services::preview::helper::run()`,
    which never returns to the app path.
  - It starts with a cleared environment, no inherited handles but its
    three pipes, and `CREATE_NO_WINDOW` on Windows.

  **Protocol**: length-prefixed binary frames over stdin and stdout.
  1. The parent sends one `Load { kind: pdf|tiff, bytes }`. The helper
     answers `Loaded { pages: [{ width_pt, height_pt }] }` or
     `Failed { reason }`.
  2. Then any number of `Render { page, width_px }` →
     `Page { png }` | `PageFailed`, and `Text { page }` → `Text { string }`.
  3. On EOF on stdin, the helper exits. Its stderr is discarded, never
     logged, since it could hold content.

  **Lifetime**:
  - one helper per open preview;
  - started by `open_preview`, killed by `close_preview`, by moving to
    another document, and when the `OpenDatabase` is dropped (lock, close,
    switch, sleep, shutdown, quit; §16).

  **Crashes and hangs**:
  - If the helper crashes or exceeds a time limit (§5) on a page, that
    page reports `PREVIEW_PAGE_FAILED`. The helper is restarted, at most
    twice per preview, for the other pages.
  - If it crashes during `Load`, the preview reports `PREVIEW_FAILED`.

  **Ownership**:
  - The `Preview` is owned by `OpenDatabase`.
  - The page-render path takes its pipe handle out from under the session
    lock first. A slow page never blocks other commands' use of the
    connection.
- **Rationale**:
  - FR-004 and the spec's crafted-document edge case: "the worst outcome
    allowed is that the preview fails", and it must not "bring the
    application down". The release profile is `panic = "abort"`, so a
    panic in an in-process decoder would end the app. A memory-corruption
    bug in a C++ engine in-process would reach the open database's key.
    Neither is possible across a process boundary.
  - FR-003 holds: the bytes travel by pipe (memory), never by file.
  - FR-003's "released when the preview closes" holds absolutely, because
    the helper's memory goes away with it.
  - Using the same executable (rather than a Tauri `externalBin` sidecar)
    needs no second binary per target triple, signing or bundling entry.
- **Alternatives considered**:
  - In-process on a worker thread: fails both rationale points.
  - A long-lived shared helper: it would keep one document's memory into
    the next, and gains little, since spawning costs tens of milliseconds
    (§18).
  - A WebAssembly sandbox (PDFium compiled to wasm, run by `wasmtime`): a
    strong boundary, but it adds a large runtime and needs a wasm PDFium
    build. Recorded as the upgrade path if OS confinement (§5) proves weak
    on some platform.

## 5. Confining the helper

- **Decision**: Defence in depth on top of the process boundary. Every
  item is set before the helper reads the document.

  | Measure | Linux | macOS | Windows |
  |---|---|---|---|
  | No new privileges | `prctl(PR_SET_NO_NEW_PRIVS)` | n/a | n/a |
  | No file or network access after PDFium is loaded | Landlock: all filesystem rights denied, plus TCP bind/connect on ABI ≥ 4. Best effort: an older kernel without Landlock still gets everything else, logged once | `sandbox_init` with the pure-computation profile | Process mitigation policies (no child processes, no dynamic code, no remote or low-label image loads); a restricted token is noted for #21 |
  | Dies with HoploDex | `PR_SET_PDEATHSIG(SIGKILL)` | stdin EOF | job object with `KILL_ON_JOB_CLOSE` |
  | Memory cap | `RLIMIT_DATA` 2 GiB | none (time limit only) | job memory limit 2 GiB |
  | No child processes | seccomp is not added; Landlock blocks `execve` of any file | in the profile | job `ACTIVE_PROCESS = 1` |
  | No core dump of document content | `RLIMIT_CORE = 0` and `PR_SET_DUMPABLE 0` | `RLIMIT_CORE = 0` | `SetErrorMode(SEM_NOGPFAULTERRORBOX)`, WER excluded for the helper |
  | Time limit | 20 s for `Load`, 10 s per `Render`/`Text`, enforced by the parent killing the helper | same | same |

  **No system fonts**: PDFium gets an empty system-font provider, so a
  non-embedded font is drawn with PDFium's built-in substitutes. The
  helper never reads font files, so Landlock and the macOS profile can
  deny all file access, and pages render identically on every OS (FR-002).
- **Rationale**:
  - FR-004 says nothing may "fetch anything from the network or the
    computer's files". PDFium doesn't load remote content by design;
    confinement makes that true even under an exploit.
  - Core dumps matter specifically here. A crafted file is exactly what
    makes a decoder crash, and systemd-coredump stores cores on disk,
    which would be decrypted content on disk (FR-003).
  - `RLIMIT_DATA`, not `RLIMIT_AS`, because PDFium's allocator reserves
    large `PROT_NONE` regions that `RLIMIT_AS` counts and `RLIMIT_DATA`
    doesn't.
- **Dependencies**:
  - `landlock` (MIT OR Apache-2.0), Linux only;
  - `sandbox_init` through `libc`/FFI, macOS;
  - existing `windows-sys` features plus `Win32_System_JobObjects` and
    `Win32_System_Threading`.
- **Alternatives considered**:
  - seccomp-bpf filters on Linux: more brittle across glibc and PDFium
    versions, for little gain over Landlock plus the process boundary.
  - Bubblewrap or Flatpak portals: external programs, which FR-002
    forbids.

## 6. Shipping PDFium

- **Decision**: Take the pinned plain (non-V8) build of
  [pdfium-binaries](https://github.com/bblanchon/pdfium-binaries) for each
  target: linux-x64, linux-arm64, mac-univ, win-x64, win-arm64.
  - **Fetching**: `scripts/fetch-pdfium.sh` downloads the tarballs by
    release tag. It checks each against a SHA-256 recorded in
    `scripts/pdfium.lock` and unpacks the library to
    `src-tauri/pdfium/<target>/`, which is git-ignored.
  - **Bundling**: Tauri's `bundle.resources` carries the library.
    `build-appimage.sh` and the dev container's `Dockerfile` run the fetch.
    The main process resolves it with `app.path().resource_dir()` and
    passes the path to the helper. The main process never loads it.
  - **Tests**: cargo tests find it through `HOPLODEX_PDFIUM_DIR`, set by
    the dev container. Without it, the PDF tests fail with a message
    pointing at the fetch script rather than skipping, so a missing engine
    can't pass silently.
  - **Updates**: an update is a change to `pdfium.lock`, reviewed like a
    dependency bump. The dependency audit can't see this library, so its
    advisories are checked by hand at each update and at the release
    review (#21), noted in DEVELOPMENT.md.
- **Licensing** (constitution, Licensing): PDFium is BSD-3-Clause. The
  build compiles in FreeType (FTL, which the FSF lists as GPLv3-compatible),
  libjpeg-turbo (IJG/BSD-3), OpenJPEG (BSD-2), Little CMS (MIT), zlib,
  libpng, and abseil (Apache-2.0). pdfium-binaries' own scripts are MIT.
  - These are recorded in `docs/third-party/pdfium.md`, with the source
    and version, before the library is added.
  - The license file pdfium-binaries ships is copied into the bundle
    beside the library.
  - Verifying that list against the pinned build's `LICENSE` file is a
    task, and the release's manual license check covers it.
- **Alternatives considered**:
  - Building PDFium ourselves: hours of Chromium toolchain per target for
    no gain over a reproducible, checksummed download.
  - Static linking: pdfium-binaries doesn't publish static builds, and a
    static link would put PDFium in the main process.

## 7. TIFF decoding

- **Decision**: In the helper, decode TIFF with the `tiff` crate (MIT,
  pure Rust), plus the `fax` crate (MIT) for CCITT Group 3 and 4, the usual
  compression of black-and-white scans.
  - `Load` walks the IFDs for the page count and sizes, using the DPI
    tags, or 200 DPI when missing, to get physical page sizes.
  - `Render` decodes one page, downsamples it to the requested width, and
    encodes PNG.
  - **Failures**: a page with an unsupported compression, photometric or
    layout fails alone with `PREVIEW_PAGE_FAILED`, and the others still
    show (spec edge case). A file whose first IFD can't be read fails as
    `PREVIEW_DAMAGED`.
- **Rationale**: The decoders are pure Rust, so they are memory-safe, but
  they can panic or allocate hugely on hostile input. Running them in the
  helper turns both into a failed page (§4). WebKitGTK and WebView2 can't
  display TIFF at all, so FR-002 needs our own decoder anyway.
- **Alternatives considered**:
  - `image`'s `tiff` feature: single-page only, and it would decode the
    whole first page in the main process.
  - libtiff through FFI: C, and a second native library.

## 8. Text and CSV

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
  - **Shown**: the frontend shows it as a React text child of a `<pre>`
    with `white-space: pre-wrap`. It is never set as HTML, never
    link-detected, and CSV is never laid out.
- **Rationale**: This decoding cannot fail unsafely (`encoding_rs` is
  total and allocation-bounded by input size), so no helper is needed.
  React escapes text children, so the content is inert.
- **Size**: a 10 MB text file is about 10 M characters in one text node.
  WebKitGTK lays that out in well under a second (§18). A text document
  over 10 MB is shown in 1 MB chunks appended as the user scrolls, so the
  first screen is within budget.
- **Alternatives considered**: decoding in the web view with
  `TextDecoder`. It would need the raw bytes in the web view, which §1
  rules out.

## 9. What the web view receives, and the CSP

- **Decision**: The CSP is unchanged. `img-src` already allows `blob:`,
  which page PNGs use. There are no frames, workers, wasm or fonts.
  - **Pages**: `<img>` elements with `blob:` URLs from the PNG bytes
    (`tauri::ipc::Response`, binary IPC, as `get_photo_original` does).
    Each URL is revoked when its page leaves the render cache (§10) and
    when the preview closes.
  - **Links**: no element derived from a document is ever a link, form or
    frame. Links in a PDF are pixels.
  - **Tests**: `csp_test.rs` gains a test that `frame-src`, `worker-src`
    (through `default-src`) and `script-src` still forbid what pdf.js would
    have needed, so a later change of engine can't loosen them silently.
- **Rationale**: FR-004's "nothing fetched", and finding 2's "disable
  active content and external resource loading", hold by construction.
  Issue #73 (the web view's own navigation) is separate and unaffected:
  the preview adds no navigable element.

## 10. Pages, zoom and memory

- **Decision**:
  - **Layout**: the viewer lays out every page from the sizes `Loaded`
    returned (FR-005: "every page"), as one vertical scroll.
  - **Rendering**: it renders only pages within one screen of the view,
    at `css width × devicePixelRatio`. That is capped at 4096 px wide and
    24 megapixels; beyond the cap the bitmap is scaled up in CSS, so a
    400% zoom stays bounded.
  - **Zoom steps**: fit width (the default for PDF and multi-page TIFF),
    fit page, and 50–400% in 25% steps. A single-page TIFF has fit and
    actual size.
  - **Zooming**: the current bitmaps are scaled in CSS at once, then
    re-rendered after 150 ms without further zoom input.
  - **Memory**: at most 8 page bitmaps are kept in the frontend,
    least-recently-shown first out.
  - **Progress**: until page 1's bitmap arrives, its slot shows progress
    ("Preparing page 1…"). Other pages show a placeholder of the right
    size.
  - **Current page**: the page indicator follows the page with the most
    visible area. Next, previous, first and last scroll to that page's
    top.
- **Rationale**: The spec's large-document case (300 pages, 150 MB) must
  stay responsive and closable. Rendering only visible pages bounds both
  the helper's work and the web view's memory.

## 11. Screen-reader text that can't be selected

- **Decision**: For each rendered PDF page, the frontend asks for
  `get_preview_page_text`. Then:
  - **The page**: the page is a `<figure>` whose `<img>` has `alt=""`, and
    whose text sits in a visually hidden `<div>` with `user-select: none`,
    labelled "Page {n} of {count}".
  - **A page with no text** (a scan, or any TIFF page): the `<img>` gets
    `alt="{document name}, page {n} of {count}"`. A single-page TIFF gets
    `alt="{document name}"`.
  - **Copying**: the viewer's root handles `copy` and `contextmenu` by
    preventing the default.
- **Rationale**: FR-005 says a PDF page's text must be available to
  screen readers without being selectable or copyable. Hidden text with
  `user-select: none` is read by AT-SPI/UIA/AX but can't be selected by
  pointer or select-all. The handlers close the remaining paths.
- **Alternatives considered**:
  - pdf.js-style positioned text spans over the image: selectable by
    design, and positioning needs the glyph boxes.
  - `aria-label` with the page text: unwieldy for long pages, and some
    screen readers truncate it.

## 12. The native confirmation

- **Decision**: `services::consent` defines a `Consent` trait:
  `ask(&self, request: ConsentRequest) -> ConsentAnswer`.
  - **The app's implementation** uses `tauri-plugin-dialog`'s Rust API:
    `app.dialog().message(..).kind(Warning).title(..).buttons(OkCancelCustom("Open in another app", "Cancel"))`,
    parented to the main window. It is awaited through a oneshot channel,
    off the session lock.
  - **The dialog's text**:
    - for a document, the title names the document;
    - for the setting change, the title states the setting;
    - the body is the four FR-009 consequences
      (contracts/ui-document-preview.md §4).
  - **The flow**:
    1. Check the type (§2).
    2. Pause the idle clock: the window gets no input while a native
       dialog is up, as with the system file chooser (003 research §15).
    3. Ask.
    4. Resume the idle clock.
    5. Re-check that the same database session is still open (by its
       generation), and only then write the copy (§14).

    A lock or sleep during the dialog therefore writes nothing
    (`DATABASE_CLOSED`).
  - **Rust tests** pass a fake `Consent` that records the requests and
    answers as told.
  - **The E2E build** (`e2e` feature only) reads
    `HOPLODEX_E2E_CONSENT=open|cancel` and appends each request's title to
    `HOPLODEX_E2E_CONSENT_LOG`. `check-no-webdriver.sh` gains this seam's
    variable name as a second marker that must be absent from a release
    binary, checked both ways round like the first.
- **Rationale**: The owner chose a native dialog because it can't be
  answered by the web view (finding 1: "require a trusted native decision
  for that exact attachment"). The text is built in Rust from the stored
  filename, so a compromised web view can't put a different name in front
  of the user. Constitution III's single confirmation pattern is deviated
  from deliberately, and recorded in spec.md's Assumptions and in Complexity
  Tracking.
- **Alternatives considered**:
  - In-app dialog plus `confirmed: true` to the backend: bypassable.
    Rejected by the owner.
  - Both an in-app explanation and a native yes/no: two dialogs for one
    decision.

## 13. The setting and the session's confirmation

- **Decision**:
  - **The setting**: `machine.json` gains
    `documentOpening: "preview" | "external"`, with
    `#[serde(default)]` = `"preview"`, so existing files read as the
    default and `VERSION` stays 1. `MachineSettings` gains
    `document_opening()` and `set_document_opening()`.
  - **The session's confirmation**: `OpenDatabase` gains
    `external_open_confirmed: bool`. It is set only by a "yes" in the
    native dialog for a document, and dies with the `OpenDatabase`, so a
    lock, close or switch forgets it (FR-012). It is never written
    anywhere.
  - **The rule** lives in one function, `needs_consent(setting,
    confirmed_this_session)`: `true` unless `setting == External &&
    confirmed_this_session`. Every external open goes through it.
- **Rationale**: The setting is per computer, for all databases (FR-011,
  003 FR-013), which is exactly `machine.json`'s role. The flag must not
  outlive the session, which is exactly `OpenDatabase`'s lifetime. Any
  confirmed open sets it: the user has accepted the consequences in this
  session. With "Preview in HoploDex" set, every open still asks
  regardless.
- **Alternatives considered**: the flag in the frontend's session store.
  It would be settable by a compromised web view.

## 14. The external copy

- **Decision**: `open_document` keeps the existing path
  (`<cache>/opened-documents/<id>/`) and clean-up (001 FR-035, 003 FR-022),
  hardened:
  - **The name**: `safe_file_name(stem)` plus **the canonical extension of
    the type found** (§2), never the stored name's extension (FR-017).
  - **Permissions**: on Unix, the folders are created `0700` and the file
    `0600`.
  - **Marked as untrusted**:
    - on Windows, a `:Zone.Identifier` stream with `ZoneId=3`, so Office
      opens it in Protected View with macros blocked and SmartScreen
      applies;
    - on macOS, a `com.apple.quarantine` attribute.
  - **Reuse**: when the copy already exists with the same length, it is
    reused rather than rewritten (spec edge case: "the one copy is reused
    or replaced"). Documents never change after attaching, and a file
    another program holds open on Windows can't be rewritten.
  - **No program for the type**: detected from the opener's error. That is
    `ShellExecuteExW`'s `SE_ERR_NOASSOC`, `open`'s non-zero exit on macOS,
    and `xdg-open`'s exit status 3 or 4 on Linux, which the `open` crate
    surfaces. The copy is securely deleted at once and
    `NO_APP_FOR_DOCUMENT` returned. Per-OS behavior is a manual check
    (quickstart).
- **Rationale**: Finding 1's "do not send executable/script types to an
  unrestricted OS opener" is met by §2. The canonical extension also
  closes the gap where the OS picks a handler by a misleading name.
  Zone marking is the platform's own "this came from elsewhere" signal for
  legacy `.doc`/`.xls` macros, which §2 can't fully exclude. Unix
  permissions make FR-009 (a)'s "anyone using this computer account" true
  rather than "anyone on this computer".

## 15. Searching document names

- **Decision**: A third trigram FTS5 index, `document_names_fts`, in
  `0002_fts5.sql`:
  - **The index**: an external-content table over
    `document_attachments(original_filename)`, with an insert trigger, a
    delete trigger and an update trigger, kept for safety even though
    filenames don't change.
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
  - A separate index keeps the firearm and accessory indexes untouched.
    Their external-content triggers need exact old values, and refreshing
    them from document triggers would mean rebuilding a firearm's whole
    row from a document change.
  - Trigram matching makes "appraisal" find "2024 appraisal.pdf" and "pdf"
    find every PDF (spec edge case).
  - The index is small: one short column, at most a few documents per
    record.
- **Alternatives considered**:
  - A `document_names` column on `firearms_fts` and `accessories_fts`: the
    trigger coupling above.
  - `LIKE` only: a scan of every filename per search, which misses the
    500 ms budget at 10,000 + 10,000 records with documents.

## 16. Closing the preview with the session

- **Decision**:
  - **Backend**: the `Preview` (its helper and its decoded text) lives in
    `OpenDatabase`, and its `Drop` kills the helper and zeroizes held
    bytes. Every path that removes the `OpenDatabase` therefore ends it:
    lock (any cause), close, switch, sleep step 1, shutdown, quit.
  - **Frontend**: the viewer is rendered inside the collection providers.
    `SessionProvider` unmounts them on any of those (keyed by the open), so
    the preview leaves the screen before the chooser is drawn (SC-006).
    Its cleanup revokes every `blob:` URL and drops the text.
  - **No hold-up**: nothing waits on the helper. Killing is immediate
    (FR-014: "does not hold up any of these").
  - **Stale requests**: a page request that races a close gets
    `DATABASE_CLOSED`, or `PREVIEW_CLOSED` if the preview was replaced, and
    is ignored.
- **Rationale**: This reuses the one mechanism 003 already tests for every
  lock cause, so SC-006 is covered by extending those tests rather than
  adding a parallel path.

## 17. Error codes

New stable codes, all in the existing `CommandError` shape
(contracts/tauri-commands.md):

| Code | When |
|---|---|
| `DOCUMENT_TYPE_NOT_ALLOWED` | attach or external open of a type outside FR-016's list (message names the document types; for JPEG/PNG says to add it under Photos) |
| `DOCUMENT_CONTENT_MISMATCH` | content doesn't match the extension, a macro-enabled Office file, or markup in a text/CSV file |
| `PREVIEW_UNSUPPORTED` | `open_preview` on a document type that isn't previewed (RTF, Word, spreadsheet) |
| `PREVIEW_DAMAGED` | PDFium or the TIFF reader can't parse it |
| `PREVIEW_PASSWORD_PROTECTED` | a PDF with its own password |
| `PREVIEW_FAILED` | the helper crashed, timed out or ran out of memory during load, or restarted too often |
| `PREVIEW_PAGE_FAILED` | one page couldn't be rendered; the others are unaffected |
| `PREVIEW_CLOSED` | a page or text request for a preview that has been closed or replaced |
| `NO_APP_FOR_DOCUMENT` | the OS has no program for the type; the copy was deleted |

`open_document` and `set_document_opening` answer a cancelled native
confirmation with a result, `{ opened: false }` and `{ changed: false }`,
not an error: cancelling is a normal outcome.

## 18. Performance

| Operation | Budget | Expected | Test |
|---|---|---|---|
| `open_preview` → page 1 PNG, 10 MB PDF | 1 s (SC-001) | read blob ~50 ms; spawn and load PDFium ~40 ms; pipe 10 MB ~10 ms; parse and render page 1 at 1600 px ~150–300 ms; PNG (fast compression) ~40 ms | `performance_test.rs` |
| `open_preview` → page 1, 10 MB TIFF | 1 s | IFD walk ~5 ms; decode page 1 (LZW or G4, 300 DPI letter) ~150 ms; downsample and PNG ~60 ms | `performance_test.rs` |
| Progress visible, 150 MB PDF | 1 s (SC-001) | the viewer shows progress at once; `Loaded` arrives after the 150 MB pipe and PDFium's lazy parse | `DocumentPreview.test.tsx` (progress shown before the first page) |
| `list_firearms` / `list_accessories` search with document names | 500 ms at 10,000 + 10,000 (SC-007) | one more indexed subquery; 30,000 seeded documents | `performance_test.rs` |
| 10 MB text shown | 1 s | decode ~30 ms; one text node | `performance_test.rs` (decode); E2E timing note |

UI-thread work is limited to setting `<img>` sources. Decoding and
rendering happen in the helper, and blob creation is asynchronous.

## 19. Testing approach

- **Hostile corpus (SC-003, SC-008)**: generated in-repo by a test-support
  module, never downloaded malware. It includes:
  - PDFs with `/OpenAction` JavaScript, `/Launch`, `/URI` links,
    `/EmbeddedFile`, `/AcroForm` with a submit action, a remote image
    XObject reference, and an external font reference;
  - truncated and bit-flipped PDFs and TIFFs, a TIFF claiming 2^31 pages
    or a 100,000 × 100,000 page, and a "PDF" that is HTML;
  - a Windows PE (just the `MZ` header and stub), a shell script, a
    `.lnk`, an SVG with an external image, an HTML page, a `.docm`
    renamed `.docx`, and a JPEG and a PNG.

  Each is asserted to be refused or shown inertly, with the app process
  alive.
- **No network, no file reads (FR-004)**: the helper tests run it under a
  loopback listener and a canary file. They assert no connection and no
  read, the latter through Landlock on Linux in the dev container.
- **No disk (SC-002)**:
  - `preview_no_disk_test.rs` previews one document of each previewable
    type, each with a unique 64-byte marker. During and after, it scans
    the test's isolated `XDG_CACHE_HOME`, `XDG_CONFIG_HOME`,
    `XDG_DATA_HOME`, `TMPDIR` and the system temp folder for the marker.
  - The E2E spec repeats the scan around a real preview.
- **The native dialog**: Rust tests through the fake `Consent`; E2E
  through the `HOPLODEX_E2E_CONSENT` seam (§12). That the real dialog
  appears and blocks is a manual check (quickstart).
- **Isolation**: every test uses throwaway databases and directories, as
  today (DEVELOPMENT.md, "Test isolation"). The helper reads nothing but
  stdin.

## 20. Dependencies added

| Crate / artifact | License | Why | Notes |
|---|---|---|---|
| `pdfium-render` 0.9 | MIT OR Apache-2.0 | PDFium bindings (§3) | no default features beyond dynamic binding; `thread_safe` not needed (one thread in the helper) |
| PDFium (pdfium-binaries, plain build) | BSD-3-Clause, with bundled third-party code (§6) | the PDF engine | not seen by `cargo deny`; recorded and checked by hand |
| `tiff` | MIT | TIFF pages (§7) | |
| `fax` | MIT | CCITT G3/G4 (§7) | |
| `landlock` (Linux) | MIT OR Apache-2.0 | helper confinement (§5) | |
| `zip`, `cfb`, `encoding_rs`, `png` | MIT / MIT / (Apache-2.0 OR MIT) AND BSD-3-Clause / MIT OR Apache-2.0 | content checks (§2), text (§8), page PNGs | promoted from transitive, at the locked versions |

None collects data or makes network requests (constitution, Security &
Data Handling). No npm package is added.
