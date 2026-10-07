# Implementation Plan: Document Preview and Consent Before Opening Externally

**Branch**: `007-document-preview` | **Date**: 2026-10-06 (first planned 2026-10-03) | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/007-document-preview/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

Attached documents are **previewed inside HoploDex** by default:
- **PDF** in the computer's own PDF viewer, inside the app's window;
- **TIFF** with HoploDex's own page and zoom controls;
- **text and CSV** as plain text.

**Opening one in another program** takes the operating system's own
confirmation, which states the consequences. A per-computer setting can
make "Open in another app" the default, asking once per database session.
Each page's search also matches its own records' **document names**.

**How the design got here**:
- **2026-10-03, issue #13's security findings**: the confirmation is the
  OS's dialog, shown by Rust; documents take an **allowlist of document
  types confirmed by content** (PDF, TIFF, TXT, CSV, RTF, DOC/DOCX,
  XLS/XLSX, ODT/ODS); photos go under Photos only.
- **2026-10-03, the PDF engine**: the owner rejected bundling PDFium (a
  binary someone else built), this plan's first answer. The web view's own
  PDF viewer was spiked on Linux, macOS and Windows and passed on all
  three, with measures (`spike-webview-pdf.md`).
- **2026-10-06, spec clarifications**: a PDF gets its viewer's own
  controls, minus Save, Print and anything that reaches the network;
  WebKit's sandbox is used on Linux where it works; if a copy is caught
  on disk, PDF preview stops on that computer until HoploDex is updated
  (FR-003a).

**Technical approach** (research.md):
- **Trust boundaries** (§1): nothing that parses a PDF or a TIFF runs in
  the main web view, which holds the IPC bridge.
- **One type check** (§2): `services::document_types::classify` at
  attach, at preview and at external open; the recorded type is the one
  it finds.
- **PDF** (§3–§10): a **child web view** inside the main window (Tauri's
  multiwebview), placed over the viewer's page area, showing the PDF with
  the web view's own viewer. The document is served from memory by a
  custom protocol at a one-time URL. The surface is held to the spike's
  measures:
  - an **app ACL manifest**, so it can call no command;
  - a **content filter**, a **proxy to a tripwire port** HoploDex holds,
    WebRTC off and a **navigation allowlist**, so it reaches no network;
  - **Save, Print, downloads and Open in Preview off**, a startup check
    of the switches, and on macOS a **watch** that deletes a copy that
    gets out anyway;
  - PDF.js's **scripting off** on Linux;
  - **WebKit's sandbox** on Linux, after a probe shows it works;
  - Escape, F6 and idle activity seen in the surface by Rust.
- **TIFF** (§11, §14): the pure-Rust `tiff` and `fax` crates in a
  **render helper**, the HoploDex executable restarted with a hidden
  argument, confined per OS; pages reach the main web view as PNGs.
- **Text** (§12): decoded in Rust and shown as a React text node.
- **Native consent** (§16): a `Consent` trait implemented with
  `tauri-plugin-dialog`'s Rust API; the session's "already confirmed"
  flag lives in `OpenDatabase`.
- **The setting and the hold** (§17): in `machine.json`.
- **External copies** (§18): written under the session lock (settles
  #65), private from creation (settles #71), the canonical extension, and
  a Zone or quarantine mark.
- **Search** (§19): a third trigram index, `document_names_fts`.

## Technical Context

**Language/Version**: Rust 1.97+ (edition 2024, `src-tauri`), TypeScript 5.x /
React 18 (`src`), unchanged

**Primary Dependencies**:
- **New crates**: `tiff`, `fax` (TIFF); `landlock` (Linux, the helper's
  confinement).
- **New features of existing crates**: `tauri`'s `unstable` (child web
  views) and `macos-proxy`; more `windows-sys` features (job objects,
  threading, security, `GetLastInputInfo`).
- **Promoted from dev-dependencies** (already in the tree through wry, at
  its versions): `webkit2gtk` (Linux), `objc2-web-kit` and `block2`
  (macOS), `webview2-com` (Windows).
- **Promoted from transitive**, at locked versions: `zip`, `cfb`,
  `encoding_rs`, `png`.
- **Reused**: `tauri-plugin-dialog` (its Rust message API),
  `tauri-plugin-opener`, `zeroize`, `libc`, `objc2-app-kit`.
- **Nothing bundled or fetched**: the PDF viewer is the one inside the web
  view HoploDex already uses.
- **Frontend**: no new npm package. It reuses `Dialog` (gaining an `xl`
  size), `Button`, `Menu`, `SegmentedControl` and `useToast`.

**Storage**: The existing encrypted SQLCipher database. `0002_fts5.sql` is
edited in place to add `document_names_fts` and three triggers. No column
changes, and an existing development database must be recreated.
`machine.json` gains `documentOpening` and `pdfPreviewHold` (both
defaulted, so no version bump).

**Testing**:
- **cargo**: integration tests against real temporary SQLCipher databases
  and the real helper binary: document types, consent with a fake
  `Consent` and opener, the `hdpreview` protocol and the preview's
  lifecycle with a recording `PreviewSurface`, the TIFF helper's hostile
  corpus and confinement, the ACL lists, the sandbox probe's decisions.
- **The PDF surface check** (new, research.md §23): an example built on
  the app's own surface code that runs the spike's reach probe, real input
  on every viewer control, and a disk scan. In the dev container on Linux,
  and by script in the macOS 26 VM and on the Windows test machine.
- **Vitest + React Testing Library**: the viewer, the surface placeholder,
  the list, the settings fieldset and the session unmount.
- **WebdriverIO**: one spec with real keyboard input and the E2E-only
  consent and PDF-availability seams, plus the screenshots walk.

**Target Platform**: Desktop: Windows 10+, macOS 26+ (raised from 12+ on
2026-10-06: the owner tests only macOS 26), Linux. The PDF viewer needs
WebKitGTK 2.40+ (the AppImage bundles 2.54) and a WebView2 runtime that
has the interfaces in research.md §7; where either is missing, PDFs open
in another app and TIFF and text still preview.

**Project Type**: Desktop application (Tauri: React frontend + Rust backend in
one repo)

**Performance Goals**:
- a PDF shown, or TIFF page 1, within 1 s at 10 MB, and progress within
  1 s for larger files (SC-001);
- search with document names within 500 ms at 10,000 firearms + 10,000
  accessories (SC-007);
- no decoding on the UI thread.

The table is in research.md §22.

**Constraints**:
- Fully offline: the surface can reach no network and no local service.
- No document bytes to disk while previewing (FR-003), including the
  helper's core dumps and the PDF viewer's own Save, Print and Open in
  Preview.
- Nothing in a document runs or fetches (FR-004).
- The main window's CSP is unchanged (no frames, workers, wasm or eval).
- Cipher settings are untouched.
- No external open without a "yes" in the native dialog, except as FR-012
  allows within a session.

**Scale/Scope**: Three user stories (P1–P3), 18 functional requirements
and 8 success criteria.
- **Interface**: 8 new commands and 7 amended; 4 events; 1 custom
  protocol; 9 new error codes.
- **Schema**: 1 new FTS table.
- **Process and web views**: 1 child web view (the PDF surface), 1 helper
  process mode, 1 startup probe mode (Linux).
- **Security model**: an app ACL manifest covering every command.
- **Settings**: 2 new machine fields.
- **UI**: 1 new viewer component and 1 new settings fieldset.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Requirement | How this plan satisfies it |
|---|---|---|
| I. Code Quality | Lint/static analysis, review, small single-purpose modules, no speculative abstraction | **One rule, one place**: `document_types::classify` for every type decision (§2), `needs_consent` for the session rule (§17), `services::consent` for the dialog text, `preview::surface` for every per-OS surface measure, `COMMANDS` for the command list. **Per-OS code** sits behind one module per OS (`surface/linux.rs`, `macos.rs`, `windows.rs`), as `platform/` does. **Refactor, not workaround**: `add_document` loses its `mimeType` argument and every caller is updated; the main window moves from `tauri.conf.json` to `setup()` rather than adding an E2E-only path. **The spike's code is retired**: `examples/pdf_spike*` are replaced by the surface check, which uses the app's own surface code. `clippy`, `rustfmt`, `eslint` and `prettier` run locally. CI stays disabled by the owner's choice |
| II. Testing (NON-NEGOTIABLE) | Tests first, real persistence, one test per acceptance scenario, test isolation | Every acceptance scenario and success criterion maps to a named test (quickstart.md). Tests run the real `ops` and the real helper against temporary databases and isolated folders. What only a real web view can show (the surface's network, disk and command reach, real input on each viewer's controls) is the surface check, automated on all three OS. The native dialog has a fake in Rust tests and an E2E-only seam, checked absent from release builds. Six manual checks are numbered procedures, not merge gates |
| III. UX Consistency | One component set and pattern, WCAG 2.1 AA | The viewer reuses `PhotoViewer`'s layout, footer, ← / → and Escape, and the delete `ConfirmDialog`. TIFF and text get HoploDex's own controls, the same on every OS. The list's actions are the existing ghost buttons; the setting is a `SegmentedControl` in Database settings. **Two deliberate deviations** (Complexity Tracking): the external-open confirmation is the OS dialog, and a PDF's controls are its viewer's, which differ by OS (spec FR-002, clarified 2026-10-06). Keyboard (with F6 into and out of a PDF) and screen-reader access are in contracts/ui-document-preview.md §7–§8 |
| IV. Performance | 100 ms feedback / 1 s completion, 500 ms search, no UI-thread blocking | "Preparing…" is drawn before `open_preview` returns. A PDF is served from memory; the surface is reused within a viewer so the next PDF is a navigation. TIFF pages render only when visible, capped in size, at most 8 kept, off the session lock and off the UI thread. Search adds one indexed subquery. `performance_test.rs`, E2E timing and the surface check's first-paint time hold each budget (research.md §22). The Linux sandbox probe adds a child process at startup, measured |
| V. User Privacy | Local only, real deletion, no hidden copies | **No disk**: previews write nothing; the viewer's Save, Print and Open in Preview are off; the macOS watch deletes a copy that gets out before Preview opens it, and turns PDF preview off rather than risk another; the helper's core dumps are off. **Real deletion**: deleting a document merges the search indexes, so its name leaves no FTS segment. **The external copy**: still under 001 FR-035's clean-up, now owner-only from creation and never written after a lock; the confirmation states it plainly, including the network consequence. **Nothing leaves the device**: the surface and the helper can't reach the network |
| Security & Data Handling | Encryption at rest, vetted dependencies, no cipher change | The trust boundaries (research.md §1) answer both #13 findings: native consent, an allowlist by content, and no PDF or TIFF parser in the main web view. The app ACL manifest closes "a local page may call every command" for the whole app. No key or cipher change. **Vetted**: every new crate is seen by `cargo deny`; nothing is bundled outside it. The WebKit SPI on macOS is checked at each start. Persistence PR notes come from research.md §1, §2, §18 and §19 |
| Licensing | GPLv3-compatible dependencies and bundled assets with recorded source | `tiff`, `fax` (MIT), `landlock` (MIT OR Apache-2.0); the promoted crates are MIT or MIT/Apache-2.0. No asset is bundled. The PDF viewers are part of the OS or of the WebKitGTK already bundled in the AppImage (PDF.js is Apache-2.0, inside WebKitGTK's own notices) |

**Result**: PASS, with the justified entries in Complexity Tracking.

## Project Structure

### Documentation (this feature)

```text
specs/007-document-preview/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── spike-webview-pdf.md # The web view PDF spike (Linux, macOS, Windows)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   ├── tauri-commands.md        # document types; attach and open amended; preview commands, events
│   │                            #  and protocol; setting commands; search; error codes
│   └── ui-document-preview.md   # list actions, viewer, states, native dialog text, setting,
│                                #  attach refusals, keyboard, accessibility, lock, screens walk
├── checklists/
│   └── requirements.md
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

The contracts and data model are **deltas** against 001, as amended by 002
to 006. As in earlier features, the final task adds a one-line "amended by
007" pointer at each amended anchor:
- **001**:
  - FR-010 (documents of a document type, previewed by default, external
    open after confirmation), FR-013 (search), User Story 4 scenario 4;
  - the DocumentAttachment entity;
  - `add_document`, `open_document`, `list_documents` and `list_firearms`
    in the IPC contract.
- **003**: FR-013's list of per-computer settings; `machine.json` in the
  data model.
- **006**:
  - FR-007a and User Story 1 scenario 12 (an accessory's documents);
  - FR-018 (Accessories search);
  - `list_accessories` and the document commands' `owner` in its IPC
    contract.

### Source Code (repository root)

This is the existing Tauri layout. Only files that change or are added are
listed.

```text
scripts/
├── check-no-webdriver.mjs           # + HOPLODEX_E2E_CONSENT and HOPLODEX_E2E_PDF_PREVIEW markers, both ways round
├── pdf-surface-check.sh             # NEW (from examples/pdf_spike.sh): Linux, Xvfb + XTest, in the dev container
├── macos/pdf-surface-check.sh       # NEW (from examples/pdf_spike_mac.sh): drives the macOS 26 VM; --hud-on
└── windows/pdf-surface-check.ps1    # NEW (from examples/pdf_spike_win.ps1): the Windows test machine
DEVELOPMENT.md                       # the surface check on each OS; the seams; the sandbox on a Linux host

src-tauri/
├── Cargo.toml                       # tiff, fax, landlock (linux); tauri unstable + macos-proxy; webkit2gtk,
│                                    #  objc2-web-kit, block2, webview2-com promoted; zip, cfb, encoding_rs,
│                                    #  png promoted; windows-sys features
├── build.rs                         # app ACL manifest from COMMANDS
├── capabilities/default.json        # allow-<command> for every command, window "main" only
├── tauri.conf.json                  # app.windows emptied: the main window is built in setup() (research.md §6)
├── src/
│   ├── main.rs                      # `--render-helper` and `--webkit-sandbox-probe` checked first; the probe
│   │                                #  sets WEBKIT_FORCE_SANDBOX (Linux); main window built with its data
│   │                                #  directory; hdpreview protocol; 8 commands; Consent, opener, tripwire and
│   │                                #  PdfAvailability state; leftovers swept at start and on signals
│   ├── commands_list.rs             # NEW: COMMANDS, shared by build.rs (include!) and the ACL test
│   ├── app_dirs.rs                  # + the web views' data directories (E2E: under HOPLODEX_E2E_CACHE_HOME)
│   ├── db/
│   │   ├── migrations/0002_fts5.sql # document_names_fts + 3 triggers
│   │   └── mod.rs                   # reclaim_deleted_record merges document_names_fts too
│   ├── models/
│   │   └── document_attachment.rs   # DocumentSummary +previewKind, +previewAvailable, +openable
│   ├── services/
│   │   ├── document_types.rs        # NEW: the allowlist, classify(), signatures, canonical MIME/extension
│   │   ├── consent.rs               # NEW: Consent trait, ConsentRequest text (FR-009), dialog impl,
│   │   │                            #  E2E seam (cfg feature = "e2e"), needs_consent()
│   │   ├── preview/                 # NEW
│   │   │   ├── mod.rs               # Preview, PreviewContent, PreviewSurface trait
│   │   │   ├── protocol_handler.rs  # hdpreview: token lookup, headers, ready and hook reports
│   │   │   ├── availability.rs      # PdfAvailability: startup check per OS, the hold, the E2E seam
│   │   │   ├── tripwire.rs          # the held loopback port
│   │   │   ├── sandbox_probe.rs     # Linux: the probe child and its decision (research.md §9)
│   │   │   ├── surface/
│   │   │   │   ├── mod.rs           # builds the child web view: label, incognito, protocol URL, navigation,
│   │   │   │   │                    #  new window, download, frame script, bounds, focus, close
│   │   │   │   ├── frame_script.js  # PDF.js hook, hidden buttons, cancelled shortcuts and contextmenu
│   │   │   │   ├── linux.rs         # content filter, proxy, print and context-menu signals, input signals
│   │   │   │   ├── macos.rs         # rule list, proxy, HUD and WebRTC SPI, NSEvent monitor, the kqueue watch
│   │   │   │   └── windows.rs       # WebResourceRequested, browser args, PDF toolbar, context menu,
│   │   │   │                        #  SaveAsUIShowing, accelerator keys, GetLastInputInfo
│   │   │   ├── helper.rs            # TIFF helper entry: confine, serve Load/Render
│   │   │   ├── helper_protocol.rs   # frame encoding shared by parent and helper
│   │   │   ├── confine.rs           # per-OS confinement (Landlock, sandbox_init, job object, rlimits)
│   │   │   ├── tiff.rs              # IFD walk, page decode (tiff + fax), downsample → PNG
│   │   │   └── text.rs              # BOM/UTF-8/UTF-16/Windows-1252 decode, control characters
│   │   ├── attachments.rs           # mime_type_for removed in favor of document_types (photos keep theirs)
│   │   ├── machine_settings.rs      # documentOpening, pdfPreviewHold
│   │   └── mod.rs
│   ├── commands/
│   │   ├── documents.rs             # classify at attach; open_document with consent, write under the lock,
│   │   │                            #  private copy, NO_APP_FOR_DOCUMENT; list_document_types;
│   │   │                            #  delete reclaims indexes
│   │   ├── preview.rs               # NEW: open_preview, set_preview_bounds, focus_preview,
│   │   │                            #  render_preview_page, close_preview + ops
│   │   ├── databases.rs             # get_document_opening, set_document_opening
│   │   ├── firearms.rs              # list_firearms search joins document names
│   │   ├── accessories.rs           # list_accessories search joins document names
│   │   └── mod.rs
│   └── session/
│       ├── mod.rs                   # OpenDatabase +preview, +external_open_confirmed, +generation;
│       │                            #  write-under-lock helper for open_document
│       └── idle.rs                  # note_activity callable from the surface's input hooks
├── examples/
│   ├── human_seed.rs                # PDF (3 pages, with text), a password-protected PDF, TIFF (multi-page, G4),
│   │                                #  TXT, CSV, DOCX, ODS, an accessory's PDF, a raw pre-007 JPEG row
│   ├── pdf_surface_check.rs         # NEW, replacing pdf_spike.rs: the app's surface + reach probe + disk scan
│   ├── pdf_spike_input.swift, pdf_spike_input.ps1, pdf_spike_dns.c   # kept, renamed for the surface check
│   └── pdf_spike.rs, pdf_spike*.sh, pdf_spike_win.ps1                  # removed once the check replaces them
└── tests/
    ├── document_types_test.rs       # NEW: FR-016 table, every refusal, canonical types
    ├── pdf_preview_test.rs          # NEW: protocol, tokens, lifecycle with a recording surface, availability,
    │                                #  hold and sweep, US2-4
    ├── tiff_preview_test.rs         # NEW: US1-2/3, pages, failures, hostile TIFFs, no disk
    ├── tiff_helper_test.rs          # NEW: confinement, parent death, limits, no core dump
    ├── preview_text_test.rs         # NEW: research.md §12 decoding
    ├── open_document_test.rs        # NEW: US2, US3, FR-008–FR-013, FR-017, #65 race, with fake Consent and opener
    ├── acl_manifest_test.rs         # NEW: COMMANDS = generate_handler! = capability
    ├── webkit_sandbox_test.rs       # NEW (Linux): the probe's decisions
    ├── support/hostile_documents.rs # NEW: corpus generators
    ├── fixtures/documents/          # NEW: small PDF/TIFF/DOCX/ODS fixtures with their source note
    ├── document_test.rs             # attach paths refuse; mimeType from content
    ├── fts_search_test.rs, list_accessories_test.rs   # + document names, not through mounts
    ├── deletion_wipe_test.rs        # + filename gone after delete_document and record delete
    ├── lock_test.rs                 # + preview open at each lock cause
    ├── machine_settings_test.rs     # + documentOpening, pdfPreviewHold
    ├── csp_test.rs                  # + the main web view still forbids frames, workers, wasm, eval
    ├── performance_test.rs          # + research.md §22
    └── human_seed_coverage_test.rs  # document_names_fts is an index

src/
├── components/Dialog.tsx            # + size "xl"
├── features/
│   ├── media/
│   │   ├── DocumentList.tsx         # name follows setting; Preview; Open in another app…; meta; refusals;
│   │   │                            #  accept from list_document_types; hosts DocumentPreview
│   │   ├── DocumentPreview.tsx      # NEW: viewer, TIFF toolbar, pages, text, states, keyboard, a11y, footer status
│   │   ├── PreviewSurface.tsx       # NEW: the PDF placeholder: bounds, hidden while covered, ready, F6, events
│   │   ├── useTiffPages.ts          # NEW: visible-page rendering, zoom debounce, 8-bitmap cache, blob URLs
│   │   ├── mediaService.ts          # addDocument without mimeType; openDocument → {opened}; preview,
│   │   │                            #  document-type and setting wrappers; event listeners
│   │   ├── filePaths.ts             # isDocumentPath from the document types
│   │   ├── types.ts                 # previewKind, previewAvailable, openable, PreviewInfo, DocumentType,
│   │   │                            #  DocumentOpening, SurfaceBounds, PdfEndReason
│   │   ├── media.css                # viewer and page styles
│   │   └── *.test.tsx
│   ├── databases/
│   │   └── DatabaseSettingsDialog.tsx  # "Documents" fieldset
│   └── session/SessionProvider.test.tsx  # viewer gone before the chooser on lock

e2e/
├── specs/us13-document-preview.e2e.ts   # NEW
├── wdio.conf.ts                         # consent and PDF-availability seams; WEBVIEW2_USER_DATA_FOLDER no longer set
└── screenshots/screens.e2e.ts           # + contracts/ui-document-preview.md §10 (PDF by window screenshot)
```

**Structure Decision**: No new project or layer.
- **Rust** owns:
  - every security decision: types, consent, the session flag, copies,
    which web view may call what;
  - the PDF surface and every per-OS measure on it;
  - the TIFF helper, text decoding and search.
- **React** owns the viewer's layout, HoploDex's controls and
  accessibility, and tells Rust where the surface goes.
- **The preview** lives in `features/media/` beside `PhotoViewer`, which
  it mirrors.
- **The E2E spec** is numbered `us13`, continuing the series.

**First tasks**, because later ones depend on them holding:
1. The child web view (research.md §4): prove `Window::add_child` with
   the surface's settings (incognito, proxy, on Windows its own data
   folder and browser arguments) inside the main window on Linux, macOS
   and Windows. If one can't, switch to the owned-window fallback before
   building the rest.
2. The app ACL manifest (§5) with every existing command, and the full
   existing E2E suite passing with it.
3. The main window built in `setup()` with its data directory, and the
   Windows E2E run passing without `WEBVIEW2_USER_DATA_FOLDER`.

### Close-out (at pull request time)

The final phase of tasks.md updates the related issues as part of opening
the pull request:

- **#13** (this feature): link the pull request (it closes the issue on
  merge). Reply to each security finding with how it is addressed:
  - native consent (FR-008, research.md §16);
  - the allowlist by content at attach and at open (FR-016, FR-017, §2);
  - canonical extensions, private copies and Zone/quarantine marks
    (§18);
  - no PDF or TIFF parser in the main web view; the PDF surface's ACL,
    network, disk and scripting measures (§1, §5–§9);
  - the network consequence in the dialog (FR-009 (c)).

  Note the residual risks: the once-per-session choice, Linux without
  WebKit's sandbox, and macOS's PDF code in a frame with Tauri's script.
- **#65** (document opening can recreate plaintext after lock cleanup):
  the pull request closes it. Comment that the copy is now written under
  the session lock and only into the session that asked (research.md §18),
  with the regression test's name.
- **#71** (decrypted document copies lack private permissions): the pull
  request closes it. Comment with the Unix modes from creation, the
  symlink and owner check, and the Windows DACL (§18), with the tests.

The branch also finished the cross-platform test environments, which 007's
spike and per-OS measures needed. The pull request closes these four too.
Before it does, open one follow-up issue for the checks they leave (below),
and link it from each:

- **#26** (verify the app on Windows and macOS, FR-022): do its last box
  in this branch: record in 001's artifacts that FR-022 is verified on
  Linux, Windows and macOS (build, Rust, Vitest, lint and audits, all 14
  E2E spec files and the screenshot walk on each), and bring 001's
  `plan.md` up to date where it is stale (the embedded driver, not
  `tauri-driver`; macOS 26, Apple silicon only). Run the release
  performance tests on the Windows and macOS machines and report them
  against the budgets. Comment with the results and tick #27, #28 and
  #29. CI (#25) is separate work: take it off #26's list and say it
  stays open on its own. The manual sleep, lock and shutdown checks go to
  the follow-up issue.

- **#27** (Windows development and test environment): comment that the
  harness is isolated (7810a4e) and ported (3a90476, 47a3bab), the
  power and session messages are tested against the hidden window
  (47a3bab), and `scripts/windows/` with DEVELOPMENT.md's Windows section
  replaces the dropped VM scripts. Move to the follow-up: Windows human
  testing (the `sandbox-dirs` split, WebView2's profile, the seed's
  safety check, `human-testing.ps1`, the keyring question), and sleep,
  lock and shutdown checked by hand on Windows.
- **#28** (macOS development and test environment): comment that the
  harness is isolated and ported, `scripts/tart-vm.sh` and
  DEVELOPMENT.md's macOS section are done, the screenshot walk,
  `quit-cleanup.py` and `human-testing.sh` run on macOS (b438ebf,
  a1402d0), and the sleep, wake, lock and power-off handlers are tested
  (37a6ba5). Move to the follow-up: sleep, wake, screen lock and log-out
  or shut-down by hand on a real Mac (a guest can't sleep), and saved
  passphrases in the Keychain under `human-testing.sh`'s `HOME`.
- **#29** (one embedded WebDriver): bring the status at the top up to
  date: the embedded driver now runs the E2E suite and the screenshot
  walk on Linux, macOS and Windows, and both outstanding boxes are
  ticked (all 14 spec files pass on each, the real-input tests pending
  off Linux). Comment that `npm run audit:webdriver` runs on Windows
  through Node (3ae66d5), and pass the release `hoplodex.exe` to it,
  which the first Windows run didn't, and report the result.
- **#21** (release security review): comment that the review must cover:
  - the app ACL manifest and the three-list test;
  - the PDF surface's measures on each OS, the WebKit SPI and its startup
    check, the macOS watch and hold, and the tripwire;
  - the Linux sandbox probe;
  - the TIFF helper and its confinement on each OS;
  - the consent and PDF-availability seams' absence from release builds;
  - `document_types::classify` and the external-copy hardening.

  List the residual risks from research.md §1.
- **#73** (web view navigation): comment that the app ACL manifest now
  refuses every command to any page other than the main window's, which
  narrows #73's impact, and that the PDF surface has its own navigation
  allowlist; the main window's own navigation is unchanged, so #73 stays
  open.
- **#40** (context menu and shortcuts): comment that 007 suppresses the
  context menu and browser shortcuts in the PDF surface only; the main
  window is unchanged.
- **#42** (Flatpak): comment that WebKit's sandbox doesn't run inside
  Flatpak, so a Flatpak build previews PDFs without it, under Flatpak's
  own sandbox (research.md §9).

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|---|---|---|
| A second web view in the main window, behind Tauri's `unstable` feature, with per-OS native code for its filter, proxy, switches, input and (macOS) a watch | FR-001's PDF preview without a bundled engine (the owner's choice, 2026-10-03); FR-003, FR-003a and FR-004 each need a measure the spike found per OS; FR-007 needs it inside the viewer | A bundled PDFium was rejected by the owner. pdf.js in the main web view would run where the IPC is (FR-004). A separate window can't hold the viewer's chrome. Each per-OS measure closes a gap the spike found in the others (research.md §6, §7) |
| A second process mode (TIFF helper) with per-OS confinement | The release build aborts on panic, and a hostile TIFF can make a decoder panic or allocate without bound; FR-004 asks for a sandbox where the OS offers one; FR-003 rules out core dumps | In-process decoding ends the app on a panic. `panic = "unwind"` changes every crash path for one decoder and doesn't bound memory (research.md §11) |
| An app ACL manifest, so every command needs a permission | Without it, any page in any HoploDex web view may call every command; the PDF surface's top frame did in the spike on every OS | Keeping the surface's protocol off the "local" list isn't something Tauri offers; the manifest is Tauri's own mechanism (research.md §5) |
| The external-open confirmation is the OS dialog, not the app's `ConfirmDialog` (constitution III) | Issue #13 finding 1: a confirmation the web view draws can be answered by a compromised web view. The owner chose a native dialog on 2026-10-03 | An in-app dialog plus a backend `confirmed` flag is bypassable; showing both asks twice (research.md §16) |
| A PDF's controls differ by OS (constitution III) | The web view's viewer can't take HoploDex's controls; the owner chose the viewer's own (spec, 2026-10-06) | HoploDex's own controls would need its own engine, which is the rejected PDFium |

## Post-Design Constitution Check

*Re-evaluated after Phase 1 (research.md, data-model.md, contracts/,
quickstart.md).*

- **Code Quality**: each rule lives in one place: document types
  (`classify`), the consent rule (`needs_consent`), the consent text
  (`services::consent`), the command list (`COMMANDS`), every surface
  measure (`preview::surface`, one file per OS), and the TIFF helper's
  frames (`helper_protocol`, shared by both ends). The frontend reads the
  types from `list_document_types` and holds no copy. The spike's
  duplicate harness is replaced by a check that uses the app's own surface
  code. Still PASS.
- **Testing**: every acceptance scenario and success criterion has a
  named test (quickstart.md). What only a real web view can show is
  automated by the surface check on every OS; the rest is cargo, Vitest
  and E2E. SC-002, SC-003 and SC-008 have generated, reproducible inputs.
  The native dialog is covered by a fake and a release-checked seam. Six
  manual checks are numbered procedures. Still PASS.
- **UX Consistency**: the viewer mirrors `PhotoViewer`; the one new
  shared change is `Dialog`'s `xl` size, which the photo viewer may adopt.
  The two recorded deviations (the OS dialog, a PDF's own controls) are
  the spec's. Toasts move into the footer while a PDF is shown, so nothing
  hides under the surface. Still PASS.
- **Performance**: the budgets have explicit tests or measurements; TIFF
  rendering is bounded by visibility and size caps; the surface is reused
  between PDFs; search adds one indexed subquery. Still PASS.
- **User Privacy**: no new storage outside the encrypted database and
  `machine.json`, which holds a choice and, at most, a version and the
  paths of caught copies (never a name or content). Deleted document names
  are merged out of the index. External copies are private and marked,
  and the dialog discloses the network consequence. Still PASS.

**Result**: PASS. The design adds no violations beyond those justified
above.

## Findings confirmed with the user

Confirmed by the owner on 2026-10-03, after reviewing issue #13's security
findings:

- **The confirmation is the operating system's dialog** (research.md
  §16), for both "Open in another app…" and changing the setting to "Open
  in another app". It is a recorded exception to constitution III.
- **Documents are document types only, confirmed by content** (research.md
  §2): PDF, TIFF, TXT, CSV, RTF, DOC/DOCX, XLS/XLSX, ODT/ODS; photos
  (JPEG, PNG) belong under Photos; other images, programs, scripts,
  installers, shortcuts, web pages, SVG and macro-enabled Office files are
  refused.
- **TIFF is the one image type documents keep**, as the format of scanned
  paperwork. The preview covers PDF, TIFF, text and CSV.
- **No PDFium** (2026-10-03): no binary built by someone else; the web
  view's own viewer instead, after the spike.

Confirmed in spec.md's clarifications of 2026-10-06:
- a PDF has its viewer's own controls, minus Save, Print and the network;
- WebKit's sandbox on Linux where it works, never forced;
- a caught copy turns PDF preview off on that computer until HoploDex is
  updated.

Design choices made at planning that the spec left open (worth a look):

- **The surface is a child web view in the main window** (research.md
  §4), behind Tauri's `unstable` feature, with an owned-window fallback
  if the first task finds a system where it doesn't work.
- **WebKit's sandbox is turned on for the whole app on Linux**, the main
  window included, after a probe that starts a real sandboxed web process
  (§9). One full E2E run on a Linux host with the sandbox on is part of
  the pull request.
- **TIFF keeps a confined helper process** (§11), because the release
  build aborts on panic.
- **Escape, F6 and idle activity in a PDF are seen natively** (§10), and
  F6 is the key into and out of the PDF.
- **The main window is built in `setup()`** rather than from
  `tauri.conf.json`, so its WebView2 folder can be set without
  `WEBVIEW2_USER_DATA_FOLDER`, which would break the surface (§6).
- **The setting's place**: a "Documents" fieldset in Database settings,
  saved at once, with a hint that it applies to every database on this
  computer (ui contract §5).
- **The session flag**: any confirmed open sets it, whatever the setting
  at the time (§17).
- **The copy's name**: its stored name's stem with the type's canonical
  extension (§18).
- **Pre-007 rows** of other types: listed, deletable, never previewed or
  opened, with no migration (data-model.md).
- **#65 and #71 are settled here**, since `open_document` is being
  rewritten anyway (§18).
