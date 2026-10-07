---

description: "Task list for feature 007: document preview and consent before opening externally"
---

# Tasks: Document Preview and Consent Before Opening Externally

**Input**: Design documents from `/specs/007-document-preview/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/ (tauri-commands.md, ui-document-preview.md), quickstart.md, spike-webview-pdf.md (all present)

**Tests**: The project constitution's Testing Standards principle is NON-NEGOTIABLE: every user story below includes test tasks written first, expected to fail, then made to pass by the implementation. Backend tests run the real `ops` against a temporary SQLCipher database (no mocks of the DB); TIFF tests run the real helper (`CARGO_BIN_EXE_hoplodex`); the native dialog is a fake `Consent` in Rust and the E2E-only seam in WebdriverIO. What only a real web view can show is the PDF surface check, automated on each OS.

**Organization**: Tasks are grouped by user story (spec.md's priorities P1–P3) so each story can be implemented and tested on its own. This feature is a **delta** against `specs/001-firearms-inventory/` as amended by 002 to 006: most tasks edit existing files (plan.md's Project Structure). The one schema change is made in place in `0002_fts5.sql` (CLAUDE.md); no new migration file.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: Which user story this task belongs to (US1–US3)
- Exact file paths are given per plan.md's Project Structure
- Work that must be built, tested or run on a specific platform gets one task per platform, starting with the platform's name

## Path Conventions

Existing Tauri desktop app: Rust backend in `src-tauri/`, React/TypeScript frontend in `src/`, WebdriverIO E2E suite in `e2e/`. Run tests, lint, the audit and screenshots through `scripts/dev-container.sh` (CLAUDE.md; commands in quickstart.md). macOS work runs in the macOS 26 VM and Windows work on the Windows test machine (DEVELOPMENT.md's macOS and Windows sections). Never open the real databases, `machine.json` or keyring entries (CLAUDE.md, "Never touch the real databases"); every new path (web view data folders, the surface check's scratch folders) is taken from `app_dirs` or a parameter.

## How the stories divide the shared pieces

- **Foundational** owns what every story stands on: the three "first tasks" of plan.md (the child web view proved on each OS, the app ACL manifest, the main window built in `setup()`), `services::document_types` and the type recorded at attach (`add_document` loses `mimeType`), `list_document_types`, the new `DocumentSummary` fields, the `documentOpening` field in `machine.json` (read by US2's consent rule and changed by US3), the document fixtures and the human seed's documents.
- **US1** owns everything that shows a document inside HoploDex: the `Preview` in the session, `open_preview` and the other preview commands, text decoding, the TIFF helper and its confinement, the PDF surface with every per-OS measure, `PdfAvailability` and the hold, the Linux sandbox probe, the surface check, the viewer and list's Preview action, and search by document names.
- **US2** owns handing a document to another program: `services::consent` (trait, text, dialog, E2E seam, `needs_consent`), the rewritten `open_document` (re-check, native consent, generation, write under the lock, private copy, canonical extension, Zone/quarantine, `NO_APP_FOR_DOCUMENT`), and the frontend's attach refusals, "Open in another app…" and their messages.
- **US3** owns the setting: `get_document_opening`/`set_document_opening` with the setting's confirmation, the once-per-session behavior across lock, close and switch, the "Documents" fieldset, and the list's name following the setting.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Dependencies, empty modules and fixtures, so later tasks in different files can proceed in parallel.

- [X] T001 Update `src-tauri/Cargo.toml` per research.md §24 and plan.md's Technical Context:
  - add `tiff` and `fax` (MIT) under `[dependencies]`, since the TIFF helper runs on every OS (T053, T055, T056), and `landlock` (MIT OR Apache-2.0) under `[target.'cfg(target_os = "linux")'.dependencies]` only;
  - add the `tauri` features `unstable` (child web views, research.md §4) and, for macOS only, `macos-proxy` (research.md §6);
  - promote from `[target.*.dev-dependencies]` to `[target.*.dependencies]`, at the same versions and features as wry's: `webkit2gtk` (Linux, `v2_40`); `objc2-web-kit`, `block2` and the `objc2-foundation` features (macOS), adding the `objc2-app-kit` and `objc2-web-kit` features the surface needs (`NSEvent`, `WKPreferences`, `WKContentRuleListStore`, …); `webview2-com` and `windows` (Windows). Remove the dev-dependency entries that become duplicates, and the comment that ties them to the spike;
  - promote `zip`, `cfb`, `encoding_rs` and `png` from transitive to direct, at the versions in `Cargo.lock` (research.md §2: `zip` at the locked 7.x/8.x `calamine` uses, `cfb` 0.7.3), with `default-features = false` where only directory reading is needed;
  - add the `windows-sys` features `Win32_System_JobObjects`, `Win32_System_Threading`, `Win32_Security`, `Win32_Security_Authorization` and `Win32_System_SystemInformation`.
  `cargo build --manifest-path src-tauri/Cargo.toml` and `npm run audit` (cargo deny, licenses) must pass in the dev container; record each new crate's license for the PR's Licensing note
- [X] T002 Create and declare the new backend modules, each with a module doc comment citing its source, empty but compiling:
  - `src-tauri/src/services/document_types.rs` (research.md §2) and `src-tauri/src/services/consent.rs` (§16), declared in `src-tauri/src/services/mod.rs`;
  - `src-tauri/src/services/preview/mod.rs` (§20) with `protocol_handler.rs` (§4, contracts/tauri-commands.md "The `hdpreview` protocol"), `availability.rs` (§7, §17), `tripwire.rs` (§6), `sandbox_probe.rs` (§9, `#[cfg(target_os = "linux")]`), `helper.rs`, `helper_protocol.rs`, `confine.rs`, `tiff.rs` (§11), `text.rs` (§12), and `surface/mod.rs` (§4–§10) with `surface/linux.rs`, `surface/macos.rs` and `surface/windows.rs`, each behind its `#[cfg(target_os = …)]` as `platform/` does, plus an empty `surface/frame_script.js` included with `include_str!`;
  - `src-tauri/src/commands/preview.rs` (contracts/tauri-commands.md "Preview commands") with an empty `pub mod ops`, declared in `src-tauri/src/commands/mod.rs`;
  - `src-tauri/src/commands_list.rs` (research.md §5), declared from `src-tauri/src/lib.rs`.
  `cargo build --manifest-path src-tauri/Cargo.toml` must pass
- [X] T003 [P] Create the new frontend modules as empty, compiling files with a header comment naming their contract section (contracts/ui-document-preview.md): `src/features/media/DocumentPreview.tsx` (§2–§3), `src/features/media/PreviewSurface.tsx` (§2 "Page area"), `src/features/media/useTiffPages.ts` (research.md §14). `npm run lint` must pass
- [X] T004 [P] Add `src-tauri/tests/fixtures/documents/` with small checked-in fixtures and a `SOURCE.md` saying how each was made and under what license: a 3-page PDF with text on each page, a password-protected PDF (its password in `SOURCE.md`), a single-page TIFF, a 4-page TIFF whose pages use LZW, Deflate, CCITT G4 and JPEG compression, a truncated and a bit-flipped copy of the 3-page PDF (both still starting `%PDF-`, so they pass the content check and reach the viewer), a DOCX, a DOC, an XLS, an XLSX, an ODT and an ODS. Add `src-tauri/tests/support/hostile_documents.rs` with generators for: a PE `.exe`, `.bat`, `.ps1`, `.sh`, `.lnk`, `.html`, `.svg`, `.gif`, `.heic` header; the installers `.msi` (an OLE file), `.dmg` (a UDIF trailer), `.pkg` (a XAR header) and `.deb` (an `ar` archive); a `.docm`, a DOCX with `vbaProject.bin`, a DOC with a `Macros` storage, an XLS with `_VBA_PROJECT_CUR`, an ODT with `Basic/`; HTML named `.pdf`; a `.txt` and a `.csv` starting with markup after a BOM and whitespace; truncated and bit-flipped TIFFs, a TIFF claiming 2^31 pages and one claiming a 100,000 × 100,000 page; a large uncompressed TIFF of a caller-given size (≥100 MB) for the large-document case; and a document holding a caller-given 64-byte marker for the disk scans (research.md §23). Declare it in `src-tauri/tests/support/mod.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: plan.md's three "first tasks", which later ones depend on holding, then the document type rule and the shapes every story uses.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

### First task 1: the child web view, proved on each OS (research.md §4)

- [X] T005 Implement the first part of `src-tauri/src/services/preview/surface/mod.rs` and its per-OS files: build a child web view labelled `preview` with `Window::add_child` inside the `main` window, `incognito`, `proxy_url` to a loopback port given by the caller, navigation and new-window handlers that deny everything but the given URL, and on Windows its own `data_directory` (taken as a parameter) with browser arguments written in full (wry's defaults plus `--proxy-server=…`, `--proxy-bypass-list=<-loopback>`, `--webrtc-ip-handling-policy=disable_non_proxied_udp`); `set_bounds(rect, visible)` and `close()`. Start `src-tauri/examples/pdf_surface_check.rs` in a "shows" mode: it opens a window as E2E isolates one (scratch config, cache and data folders), registers a minimal `hdpreview` handler serving the 3-page fixture PDF from memory, places the surface over a rectangle of the window, and exits 0 once the PDF has loaded, saving a window screenshot
- [X] T006 On Linux, run `pdf_surface_check` in "shows" mode in the dev container under Xvfb (as `examples/pdf_spike.sh` does) and confirm the PDF is drawn by PDF.js inside the main window at the given bounds, with the surface incognito and proxied; record the result in research.md §4
- [X] T007 On macOS, run `pdf_surface_check` in "shows" mode in the macOS 26 VM and confirm the PDF is drawn by WebKit's plugin inside the main window at the given bounds, with `proxy_url` set; record the result in research.md §4
- [X] T008 On Windows, run `pdf_surface_check` in "shows" mode on the Windows test machine and confirm the PDF is drawn by Edge's viewer inside the main window at the given bounds, with the surface in a browser process of its own (its own `data_directory` and arguments) while the main window keeps its own; record the result in research.md §4
- [ ] T009 If any of T006, T007 or T008 failed, switch `surface/` to research.md §4's fallback (an owned, undecorated window kept over the viewer's page area, moved and resized with the main window, with the same settings) and rerun the failing check before any later surface task; record the decision in research.md §4 and plan.md's "Design choices made at planning". If all passed, record that the child web view stands

### First task 2: the app ACL manifest (research.md §5)

- [X] T010 Write failing `src-tauri/tests/acl_manifest_test.rs`: it reads `COMMANDS` (from `src-tauri/src/commands_list.rs`), parses the command names out of `generate_handler!` in `src-tauri/src/main.rs` and the `allow-…` permissions out of `src-tauri/capabilities/default.json`, and fails unless all three name the same commands, listing each one missing from a list; and it checks that every capability's `windows` is exactly `["main"]` (no capability names `preview`) and that `core:default` and `dialog:default` appear only there
- [X] T011 Fill `src-tauri/src/commands_list.rs` with `pub const COMMANDS: &[&str]` naming every command now in `generate_handler!`; in `src-tauri/build.rs`, `include!` it and call `tauri_build::try_build(tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)))`, keeping the Windows `resource.lib` link for examples and rewording its comment from `pdf_spike` to `pdf_surface_check`; in `src-tauri/capabilities/default.json`, add the permission Tauri generates for each app command, keeping `core:default`, `core:window:allow-set-title` and `dialog:default`, window `main` only. Make T010 pass (depends on T002)
- [ ] T012 Run the full existing E2E suite in the dev container with the manifest in place, one spec file at a time as DEVELOPMENT.md describes (`npm run build` first; us1–us12, `us7-databases-no-keyring`, `ui-review`), and fix any command or plugin call the manifest now refuses

### First task 3: the main window built in `setup()` (research.md §6)

- [X] T013 In `src-tauri/src/app_dirs.rs`, add the web views' data directories: `main_webview_data_dir(app)` and `preview_webview_data_dir(app)` (`<cache>/preview-webview2/` on Windows), under `HOPLODEX_E2E_CACHE_HOME` in an E2E build as the other directories are; with a unit test that an E2E build's paths fall under the sandbox
- [X] T014 Build the `main` window in `setup()` in `src-tauri/src/main.rs` with `WebviewWindowBuilder` (title "HoploDex", 1200 × 800, minimum 800 × 600, as `tauri.conf.json` gives today) and `data_directory(app_dirs::main_webview_data_dir(app))`, and empty `app.windows` in `src-tauri/tauri.conf.json`, keeping `app.security.csp` and `devCsp` unchanged. In `e2e/wdio.conf.ts`, stop setting `WEBVIEW2_USER_DATA_FOLDER`. Run the whole Linux E2E suite in the dev container one spec at a time, and `cargo test` (depends on T013, T011)
- [X] T015 On Windows, run the full E2E suite on the Windows test machine without `WEBVIEW2_USER_DATA_FOLDER`, and confirm the main window's WebView2 folder is under the sandbox's cache and nothing is created in the default WebView2 user data folder; fix any failure
- [X] T016 On macOS, run the full E2E suite in the macOS 26 VM with the main window built in `setup()` and the ACL manifest; fix any failure

### Document types and the recorded type (FR-016, research.md §2)

- [X] T017 [P] Write failing `src-tauri/tests/document_types_test.rs` against `services::document_types::classify(filename, bytes)`:
  - every type of research.md §2's table is accepted from the fixtures, with its canonical MIME type (data-model.md's list, verbatim) and canonical extension, and the extension compared case-insensitively (`.PDF`, `.Tiff`);
  - `%PDF-` within the first 1024 bytes is accepted and beyond it refused; `II*\0`, `MM\0*`, `II+\0` and `MM\0+` are TIFF;
  - each hostile generator of T004 is refused with the right code: `DOCUMENT_TYPE_NOT_ALLOWED` for every extension outside the list, with the photo message for `.jpg`, `.jpeg` and `.png`; `DOCUMENT_CONTENT_MISMATCH` for a content/extension mismatch, every macro-enabled OOXML, OLE or ODF content (a `.docx` holding `vbaProject.bin` included), and a `.txt` or `.csv` whose first 512 bytes after an optional BOM and whitespace start with `<` followed by `!DOCTYPE`, `html`, `svg`, `?xml`, `script` or `head`, case-insensitively;
  - a ZIP whose entries would decompress to gigabytes classifies in under 50 ms (only the directory is read);
  - the type list's order and labels match contracts/tauri-commands.md's `DocumentType` ("PDF", "TIFF", "Plain text", "CSV", "RTF", "Word", "Spreadsheet", "OpenDocument text", "OpenDocument spreadsheet"), and `preview_kind` is `pdf` for PDF, `tiff` for TIFF, `text` for plain text and CSV and none for the rest
- [X] T018 Implement `src-tauri/src/services/document_types.rs`: the one list (label, extensions, canonical MIME type, canonical extension, preview kind), `classify(filename, bytes) -> Result<DocumentType, Refusal>` with research.md §2's signature checks through `zip` and `cfb` directory reading only, `from_recorded(mime_type) -> Option<DocumentType>` for stored rows, and `Refusal`'s conversion to `CommandError` with the codes `DOCUMENT_TYPE_NOT_ALLOWED` (a message naming the document types; for a photo, "{name} is a photo. Add it under Photos instead.") and `DOCUMENT_CONTENT_MISMATCH`. Make T017 pass
- [X] T019 [P] Extend `src-tauri/tests/document_test.rs`: both `ops::add_document` and `ops::add_document_from_path` refuse a JPEG, an `.exe`, HTML named `.pdf` and a `.docm` with the right code and store nothing (row count unchanged); an accepted file's recorded `mime_type` is the canonical one whatever its name's case; `list_documents` gives a PDF `previewKind: "pdf"` with `previewAvailable` true while PDF preview is available and false while it isn't, a DOCX `previewKind: null` with `openable: true`, and a raw-SQL row of type `image/jpeg` `previewKind: null`, `previewAvailable: false`, `openable: false`, still deletable
- [X] T020 In `src-tauri/src/commands/documents.rs`, make `ops::add_document(conn, owner, bytes, original_filename)` classify the bytes and record the canonical type, dropping its `mime_type` argument, and `add_document_from_path` classify likewise; remove `services::attachments::mime_type_for`'s document use (photos keep theirs) in `src-tauri/src/services/attachments.rs`. Update every caller, with no compatibility variant (refactor, not workaround): the `add_document` command, `src-tauri/examples/human_seed.rs`, every test under `src-tauri/tests/` (`grep -rn 'add_document(' src-tauri/tests src-tauri/examples`), and `addDocument` in `src/features/media/mediaService.ts` and its caller in `src/features/media/DocumentList.tsx`, which stop sending `mimeType` (depends on T018)
- [X] T021 In `src-tauri/src/models/document_attachment.rs`, add `preview_kind: Option<PreviewKind>`, `preview_available: bool` and `openable: bool` to `DocumentSummary` (camelCase), derived on read per data-model.md "Derived on read". In `src-tauri/src/services/preview/availability.rs`, define `PdfAvailability` (`Available` | `Unavailable { reason: UnavailableReason }` with `Held`, `CheckFailed` and `NoViewer`, each with its plain sentence for the message) as Tauri state, `Available` until US1's startup check exists; `list_documents` reads it for `previewAvailable`. Make T019 pass (depends on T020)
- [X] T022 Add `list_document_types` (contracts/tauri-commands.md) to `src-tauri/src/commands/documents.rs` with its `ops` function, returning the `DocumentType` list in table order; register it in `generate_handler!` in `src-tauri/src/main.rs`, in `COMMANDS` and in `src-tauri/capabilities/default.json` (T010 must still pass), with a case in `document_types_test.rs`
- [X] T023 [P] In `src/features/media/types.ts`, add `PreviewKind`, the three new `DocumentSummary` fields, `DocumentType`, `PageSize`, `PreviewInfo`, `SurfaceBounds`, `PdfEndReason` and `DocumentOpening` exactly as contracts/tauri-commands.md "Shapes" gives them; in `src/features/media/mediaService.ts`, add `listDocumentTypes()`. Update every test fixture building a `DocumentSummary` (`grep -rn 'originalFilename' src --include='*.test.*'`) so `npm test` and `npm run lint` pass
- [X] T024 [P] In `src-tauri/src/services/machine_settings.rs`, add `document_opening: DocumentOpening` (`"preview"` | `"external"`, `#[serde(default)]` = `Preview`) to `machine.json` with `document_opening()` and `set_document_opening()`, `VERSION` staying 1; extend `src-tauri/tests/machine_settings_test.rs`: a fresh file and an existing file without the field read `"preview"`, and a set value survives a reload (US3-1, FR-011)

### Seed (CLAUDE.md "Keep the human-testing seed in step")

- [X] T025 In `src-tauri/examples/human_seed.rs`, per quickstart.md's "Seed in step" row and its manual checks: give "Glock 19 Gen5" "Purchase receipt.pdf" (3 pages with text; extend `simple_pdf` for pages if needed), "Appraisal (protected).pdf" (the password-protected fixture), a multi-page G4 TIFF ("Appraisal scan.tif"), the existing text note, a CSV ("Round count.csv"), "Bill of sale.docx" and "Range log.ods"; keep the Leupold accessory's receipt and mount that accessory on the Glock so its documents test FR-007's "never the mounted record's"; and insert one raw-SQL pre-007 row of type `image/jpeg` ("Old scan.jpg") on the Glock, since `add_document` refuses it. Read the fixtures from `tests/fixtures/documents/` with `include_bytes!`. `cargo test --test human_seed_coverage_test --test seed_sandbox_test` must pass (depends on T020, T004)

**Checkpoint**: The surface works in the main window on every OS (or the fallback is chosen), every command is held to the `main` window, documents are recorded by their content, and every story can begin.

---

## Phase 3: User Story 1 - Preview a Document Inside the Application (Priority: P1) 🎯 MVP

**Goal**: A PDF, a TIFF (single or multi-page) or a text or CSV document attached to a firearm or an accessory is shown inside HoploDex, in a viewer over its record, with nothing written to disk, nothing in it run or fetched, and the preview gone with the session; each page's search also finds its own records' document names.

**Independent Test**: Attach a 3-page PDF, a single and a 4-page TIFF and a text file to a firearm, and a PDF to an accessory mounted on it; open each from its record and see it in the viewer with the right controls, move through only that record's documents, close with Escape, lock with a PDF shown; scan the disk for each document's marker during and after; search "appraisal" on the collection and "receipt" on the Accessories page.

### Tests for User Story 1 (mandatory per constitution)

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation**, except the two regression guards T029 and T037, which pass when written and must keep passing

- [X] T026 [P] [US1] Extend `src-tauri/tests/fts_search_test.rs`: a firearm with "2024 appraisal.pdf" is found by `list_firearms` searching "appraisal" and "pdf", and no longer after `delete_document`; a one- and a two-character term match through the `LIKE` path; deleting the firearm removes its entries (the cascade fires the trigger); a firearm is not found through a document of an accessory or firearm mounted on it; a photo's filename is not searched (FR-015, US1-9)
- [X] T027 [P] [US1] Extend `src-tauri/tests/list_accessories_test.rs`: an accessory with "Optic receipt.pdf", mounted on a firearm, is found by `list_accessories` searching "receipt", its host firearm is not found by `list_firearms` searching it, and an accessory is not found through the documents of a record mounted on it (US1-9)
- [X] T028 [P] [US1] Extend `src-tauri/tests/deletion_wipe_test.rs`: a unique document filename is absent from the database's raw bytes (read through `db/raw_file.rs`) after `delete_document` and, separately, after deleting the record that owned it (research.md §19)
- [X] T029 [P] [US1] (Regression guard: passes when written, and keeps passing once T044 adds the table) Extend `src-tauri/tests/human_seed_coverage_test.rs` so `is_user_table` treats `document_names_fts` and its shadow tables (`name.starts_with("document_names_fts")`) as an index, citing data-model.md; leave `NEVER_SEEDED` unchanged
- [X] T030 [P] [US1] Write failing `src-tauri/tests/preview_text_test.rs` for `services::preview::text::decode`: UTF-8, UTF-8 with BOM (stripped), UTF-16 LE and BE by BOM, invalid UTF-8 read as Windows-1252; C0 and C1 controls other than tab, LF and CR become U+FFFD; CR LF and CR become LF; `<b>bold</b>`, `=SUM(A1:A3)` and `https://example.com` come back unchanged as text (US1-4, research.md §12)
- [X] T031 [P] [US1] Write failing `src-tauri/tests/pdf_preview_test.rs` with a recording `PreviewSurface` and the protocol handler called directly:
  - `open_preview` on the 3-page PDF returns `{ kind: "pdf" }` and navigates the surface to `hdpreview://localhost/<token>/document.pdf` with a 128-bit token; the handler answers that URL `200` with exactly the document's bytes, `Content-Type: application/pdf`, `Cache-Control: no-store` and no `Accept-Ranges`, then emits `preview:pdf-ready` (on Linux only after `GET /hooked/<surface secret>` has also been seen); any other path, the previous token, and the token after `close_preview` get `404`;
  - on Linux, no hook within 5 s of the serve closes the surface with `preview:pdf-ended { failed }`; a download of the surface's own URL sets `PdfAvailability` to `NoViewer` and ends with `noViewer`;
  - PDF → PDF keeps the same surface, navigates it and revokes the old token (its bytes zeroized); PDF → TIFF and PDF → text close it; `close_preview` with a stale id is ignored; requests with a replaced `previewId` get `PREVIEW_CLOSED`;
  - `set_preview_bounds` refuses a negative, non-finite or sub-pixel size with `VALIDATION_ERROR`, and keeps the surface hidden before `preview:pdf-ready` whatever `visible` says;
  - `PdfAvailability`: `CheckFailed` and `Held` make PDFs `previewAvailable: false` and `open_preview` fail with `PDF_PREVIEW_UNAVAILABLE`, while TIFF and text still preview; the hold: `pdfPreviewHold.version` equal to the running version means `Held`, a different one is cleared at startup; the sweep deletes only the recorded `leftovers` on a scratch folder, drops the ones gone, and leaves another `WebKitPDFs-*` folder alone (FR-003a);
  - `open_preview` on HTML named `.pdf` fails with `DOCUMENT_CONTENT_MISMATCH` before any surface exists; on a DOCX or a pre-007 JPEG row with `PREVIEW_UNSUPPORTED` (US2-4);
  - `delete_document` of the document shown closes its preview first
- [X] T032 [P] [US1] Write failing `src-tauri/tests/tiff_preview_test.rs` against the real helper (`CARGO_BIN_EXE_hoplodex`):
  - the single-page fixture opens with one `PageSize` in points at its DPI (200 DPI when the tags are missing); the 4-page fixture with four, and each page renders to a PNG of the requested width (US1-2, US1-3);
  - a page with an unsupported compression fails alone with `PREVIEW_PAGE_FAILED` while the others render; an unreadable first IFD fails `open_preview` with `PREVIEW_DAMAGED`;
  - each hostile TIFF of T004 (truncated, bit-flipped, 2^31 pages, 100,000 × 100,000) fails cleanly and the test process and session are still usable (SC-003);
  - `close_preview` during the `Load` of a large generated TIFF (≥100 MB, T004) returns within 100 ms, the helper's PID is gone and the session is usable at once (spec Edge Cases, "A large document");
  - a helper killed during a `Render` fails that page, is restarted for the next, and after a third crash the preview fails with `PREVIEW_FAILED` and is closed;
  - `widthPx` is clamped to 4096 px and to 24 megapixels for the page's aspect ratio; `page` out of range, `widthPx` < 1 or a non-TIFF preview give `VALIDATION_ERROR`;
  - the helper's PID is gone after `close_preview`, after `open_preview` of another document, and after lock, close, switch, sleep step 1 and shutdown (US1-8);
  - with `TMPDIR` and the XDG folders pointed at a scratch folder, a preview of a TIFF holding a marker leaves no file containing it (FR-003)
- [X] T033 [P] [US1] Write failing `src-tauri/tests/tiff_helper_test.rs` against the real helper: once confined, it can't open a file or a TCP socket (on Linux, Landlock in the dev container; the test reports which ABI was enforced); it exits when its parent dies; its environment is empty; core dumps are off (`RLIMIT_CORE` 0, and on Linux not dumpable); a `Load` over 20 s or a `Render` over 10 s is killed by the parent (research.md §11's confinement table)
- [X] T034 [P] [US1] Write failing `src-tauri/tests/webkit_sandbox_test.rs` (`#![cfg(target_os = "linux")]`) for `sandbox_probe`'s decision function: probe exit 0 → on; a crash, a non-zero exit or a 5 s time-out → off with the reason; `/run/.containerenv`, `/.dockerenv` or `/.flatpak-info` present (paths given as parameters) → off without probing; never on otherwise (FR-004, research.md §9)
- [X] T035 [P] [US1] Extend `src-tauri/tests/machine_settings_test.rs`: `pdfPreviewHold` defaults to `null`, a file without it reads `null`, `{ version, leftovers }` survives a reload, and the field returns to `null` when both are empty
- [X] T036 [P] [US1] Extend `src-tauri/tests/lock_test.rs`: with a PDF preview open (recording surface) and with a TIFF preview open (real helper), every lock cause, close, switch, sleep step 1 and shutdown leaves `OpenDatabase` gone, the surface closed and the helper's PID gone, without holding up the close (FR-014, SC-006)
- [X] T037 [P] [US1] (Regression guard: the CSP doesn't change, so it passes when written) Extend `src-tauri/tests/csp_test.rs`: the main window's CSP still has `frame-src 'none'`, no `worker-src` loosening `default-src 'self'`, and no `'unsafe-eval'` or `'wasm-unsafe-eval'` in `script-src` (research.md §13)
- [X] T038 [P] [US1] Write failing `src/features/media/DocumentPreview.test.tsx` with mocked `mediaService` and events:
  - the dialog is size `xl`, titled with the name, described with `{kind} · added {date}` and `Document {i} of {count}`, and `{n} pages` for a multi-page TIFF; "Preparing {name}…" is shown before `openPreview` resolves (SC-001's progress), and Escape and the close control close the viewer and call `close_preview` while it is shown (spec Edge Cases, "A large document");
  - previous and next (buttons, ← and →) move only through the record's own documents in list order; Escape, the close control and `preview:escape` close it, call `close_preview`, and return focus to the opener (US1-6, US1-7);
  - F6 calls `focus_preview`; `preview:focus-chrome` moves focus to the first control;
  - a multi-page TIFF has first, previous, `Page {n} of {count}` (`aria-live="polite"`), next, last, zoom out, `{percent}%` menu (50–400% and the two fits), zoom in, fit width, fit page, disabled until page 1 arrives; a single-page TIFF has zoom out, percent, zoom in, fit and actual size; the page area keys of ui contract §7; each `<img>` has `alt="{name}, page {n} of {count}"` or `alt="{name}"`;
  - text is a text child of `<pre class="hd-preview__text">` with no `<a>` and no `<table>`, even for CSV, a URL and `<b>`;
  - every state of ui contract §3 shows its sentence in a `role="status"` region, with or without "Open in another app…" as the table says, and previous and next stay usable;
  - "Delete document" uses the list's `ConfirmDialog`, then moves to the next document, or the previous, or closes;
  - unmounting revokes every `blob:` URL and calls `close_preview`
- [X] T039 [P] [US1] Write failing `src/features/media/PreviewSurface.test.tsx`: it sends `set_preview_bounds` with its rectangle on mount, on resize and on window resize; `visible: false` while a `ConfirmDialog`, a `Menu` or a toast is over the viewer, and `true` once gone; it shows "Preparing {name}…" with a spinner (`aria-live="polite"`) until `preview:pdf-ready` for its `previewId`; it is `role="region"` named "{name}, PDF" with the visually hidden F6 hint of ui contract §2
- [X] T040 [P] [US1] Write failing `src/features/media/useTiffPages.test.ts`: only pages within one screen of the view are requested, at CSS width × `devicePixelRatio` capped at 4096 px; at most 8 bitmaps are kept, least recently shown first out, and each evicted `blob:` URL is revoked; zoom scales at once and re-renders after 150 ms without further input; the current page is the one with the most visible area (research.md §14)
- [X] T041 [P] [US1] Write failing `src/features/media/DocumentList.test.tsx` (US1 part): kind labels come from `listDocumentTypes` (a pre-007 row shows its extension in capitals); the meta line adds "· opens in another app", "· can't be previewed on this computer" and "· can't be opened: not a document type" as ui contract §1 says; "Preview" (ghost, `eye` icon) is shown whenever `previewKind` is set and disabled with the reason as `title` when `previewAvailable` is false; the name opens the viewer with `title` "Preview {name}"; `preview:pdf-ended` with `copyCaught` or `noViewer` reloads the list
- [X] T042 [P] [US1] Extend `src/features/session/SessionProvider.test.tsx`: with the viewer open, a lock unmounts it before the chooser is rendered (SC-006)
- [X] T043 [P] [US1] Write the failing `e2e/specs/us13-document-preview.e2e.ts` (US1 part), on the seeded Glock:
  - SC-001: attach a PDF of about 10 MB that the spec writes to its sandbox (`e2e/support/largePdf.ts`: a valid PDF whose pages carry a large incompressible image), open it and record the time to `preview:pdf-ready`, within 1 s;
  - keyboard only: open "Purchase receipt.pdf" from the record, wait for ready, F6 into it and back, → to the TIFF (the surface is closed), PageDown and + in the TIFF, → to the text, Escape;
  - moving through the Glock's documents never reaches the mounted Leupold's; the Leupold's own record previews its receipt;
  - SC-002: with a marker document of each previewable type attached, the sandbox's XDG folders, `TMPDIR` and the system temp folder hold no file containing a marker during and after each preview;
  - a lock with a PDF shown: once the chooser is up, a window screenshot from the X display (`import -window`) shows no PDF;
  - the idle lock fires with a PDF shown and no input (`HOPLODEX_E2E_IDLE_MINUTE_SECONDS`);
  - FR-006, SC-003: the truncated and the bit-flipped PDF of T004, attached to the Glock, each reach the surface (the viewer reports them in its own words), the footer's "Open in another app…" stays enabled beside them, and the app stays up;
  - with `HOPLODEX_E2E_PDF_PREVIEW=off`, PDFs say they can't be previewed on this computer and the TIFF and text still preview

### Implementation for User Story 1

#### Search by document names (FR-015)

- [ ] T044 [US1] In `src-tauri/src/db/migrations/0002_fts5.sql`, add `document_names_fts` verbatim from data-model.md (`fts5(original_filename, content = 'document_attachments', content_rowid = 'id', tokenize = 'trigram remove_diacritics 1')`) and its `AFTER INSERT`, `AFTER DELETE` and `AFTER UPDATE OF original_filename` triggers on `document_attachments`, in `firearms_fts`'s external-content pattern
- [ ] T045 [US1] In `src-tauri/src/db/mod.rs`, make `reclaim_deleted_record` also run `'optimize'` on `document_names_fts`; in `src-tauri/src/commands/documents.rs`, make `ops::delete_document` call it instead of `reclaim_freed_space` (research.md §19). Make T028 and T029 pass (depends on T044)
- [ ] T046 [US1] Add the document-name match to the search of `list_firearms` in `src-tauri/src/commands/firearms.rs` and `list_accessories` in `src-tauri/src/commands/accessories.rs`, joining only the record's own column (`firearm_id`, `accessory_id`): for three or more characters `OR f.id IN (SELECT d.firearm_id FROM document_attachments d WHERE d.id IN (SELECT rowid FROM document_names_fts WHERE document_names_fts MATCH :query))`, and for one or two `OR EXISTS (SELECT 1 FROM document_attachments d WHERE d.firearm_id = f.id AND d.original_filename LIKE :like ESCAPE '\')`, verbatim from research.md §19, and the same over `accessory_id`. Make T026 and T027 pass (depends on T044)

#### The preview in the session, and text

- [ ] T047 [P] [US1] Implement `src-tauri/src/services/preview/text.rs` (`decode(bytes) -> String`) per research.md §12 with `encoding_rs`. Make T030 pass
- [ ] T048 [US1] Implement `src-tauri/src/services/preview/mod.rs` per data-model.md "`Preview` (in memory, new)": `Preview { id, document_id, content }`, `PreviewContent::{Pdf { token, bytes: Zeroizing<Vec<u8>>, surface: Arc<dyn PreviewSurface>, served, hooked }, Tiff { helper, pages, bytes }, Text }`, and the `PreviewSurface` trait (`navigate(url)`, `set_bounds(rect, visible)`, `focus()`, `close()`, also on `Drop`), plus `PreviewInfo`, `PageSize` and `SurfaceBounds` (camelCase, contracts/tauri-commands.md). In `src-tauri/src/session/mod.rs`, add `preview: Option<Preview>` and a per-session preview id counter to `OpenDatabase`, so every path that drops it ends the preview (research.md §20)
- [ ] T049 [US1] Implement `open_preview` and `close_preview` in `src-tauri/src/commands/preview.rs` with their `ops`: `open_preview` replaces any open preview, classifies the document (`DOCUMENT_CONTENT_MISMATCH`; `PREVIEW_UNSUPPORTED` for RTF, Word, spreadsheets and pre-007 rows), and for text returns `{ kind: "text", text }` with no helper; `close_preview` is idempotent. Through `session.read`, so they work while pending changes wait. Make `ops::delete_document` close the preview of the document it deletes. Register both in `generate_handler!`, `COMMANDS` and the capability (depends on T048, T047)

#### TIFF in a confined helper (research.md §11, §14)

- [ ] T050 [P] [US1] Implement `src-tauri/src/services/preview/helper_protocol.rs`: length-prefixed binary frames `Load { bytes }` → `Loaded { pages: [{ width_pt, height_pt }] }` | `Failed { reason }`, and `Render { page, width_px }` → `Page { png }` | `PageFailed`, with a frame size limit, encoders and decoders used by both ends, and unit tests for a round trip and for truncated or oversized frames
- [ ] T051 [P] [US1] Implement `src-tauri/src/services/preview/tiff.rs`: an IFD walk for the page count and sizes (DPI tags, else 200 DPI), one page's decode with `tiff` plus `fax` for CCITT G3 and G4, downsampling to the requested width, and PNG encoding with `png`; an unsupported compression, photometric or layout is a page failure
- [ ] T052 [US1] On Linux, implement `src-tauri/src/services/preview/confine.rs`'s Linux branch: `PR_SET_NO_NEW_PRIVS`, `PR_SET_PDEATHSIG(SIGKILL)`, `RLIMIT_DATA` 2 GiB, `RLIMIT_CORE` 0, `PR_SET_DUMPABLE 0`, and Landlock denying every filesystem right (blocking `execve`) and, on ABI ≥ 4, TCP bind and connect, best effort on older kernels and logged once
- [ ] T053 [US1] Implement `src-tauri/src/services/preview/helper.rs`'s `run()` (confine, then serve frames over stdin and stdout with `tiff.rs`, exit on EOF, never return to the app path) and the parent side in `services/preview/mod.rs`: a `HelperHandle` that spawns `hoplodex --render-helper` with a cleared environment, only its three pipes, stderr discarded and `CREATE_NO_WINDOW` on Windows; the 20 s `Load` and 10 s `Render` limits enforced by killing it; restarts at most twice per preview; kill and wait on `Drop`. In `src-tauri/src/main.rs`, check for `--render-helper` first, before any Tauri, keyring, session or logging setup (depends on T050, T051, T052)
- [ ] T054 [US1] Add the TIFF path to `open_preview` (start the helper, `Load`, return `{ kind: "tiff", pages }`; `PREVIEW_DAMAGED`, `PREVIEW_FAILED`) and implement `render_preview_page` in `src-tauri/src/commands/preview.rs`, returning PNG bytes as a binary `tauri::ipc::Response` without holding the session lock while the helper renders, with `widthPx` clamped (4096 px, 24 MP) and the errors of contracts/tauri-commands.md; register it in `generate_handler!`, `COMMANDS` and the capability. Make T032 and T033 pass on Linux (depends on T053, T049)
- [ ] T055 [US1] On macOS, implement `confine.rs`'s macOS branch (`sandbox_init` with the pure-computation profile, `RLIMIT_CORE` 0; the time limit from the parent), and run `tiff_helper_test` and `tiff_preview_test` in the macOS 26 VM until they pass
- [ ] T056 [US1] On Windows, implement `confine.rs`'s Windows branch (a job object with `KILL_ON_JOB_CLOSE`, `ACTIVE_PROCESS = 1` and a 2 GiB memory limit; process mitigation policies for no child processes, no dynamic code and no remote or low-label image loads; `SEM_NOGPFAULTERRORBOX` and WER excluded), and run `tiff_helper_test` and `tiff_preview_test` on the Windows test machine until they pass

#### PDF: availability, the hold, the protocol and the tripwire

- [ ] T057 [P] [US1] In `src-tauri/src/services/machine_settings.rs`, add `pdfPreviewHold: { version: string | null, leftovers: string[] } | null` (default `null`) with methods to set the hold to a version, add and drop leftovers, clear a different version at startup, and sweep the recorded leftovers (delete each, drop the ones gone, touch nothing else), per data-model.md's `machine.json` table. Make T035 pass
- [ ] T058 [US1] Complete `src-tauri/src/services/preview/availability.rs`: the startup check per OS (Linux: WebKitGTK ≥ 2.40; macOS: `+[WKPreferences _features]` lists `PDFPluginHUDEnabled`, `_setEnabled:forFeature:` and `_setPeerConnectionEnabled:` respond, and a `WKPreferences` reads both back off; Windows: `GetAvailableCoreWebView2BrowserVersionString` at least the first version with `HiddenPdfToolbarItems`, `SaveAsUIShowing`, `ContextMenuRequested` and browser accelerator keys), the hold (`Held` while `pdfPreviewHold.version` is this `CARGO_PKG_VERSION`), `NoViewer` and `Held` set at run time, and the E2E seam: in an `e2e` build only (`#[cfg(feature = "e2e")]`), `HOPLODEX_E2E_PDF_PREVIEW=off` makes the check fail. Run it once in `setup()` before anything is shown (depends on T057, T021)
- [ ] T059 [P] [US1] Implement `src-tauri/src/services/preview/tripwire.rs`: a `TcpListener` on `127.0.0.1:0` bound at the first PDF preview and held for the run, accepting and closing each connection without reading, counting them for tests, and logging once per run that a request was refused, without its address (research.md §6); with a unit test
- [ ] T060 [US1] Implement `src-tauri/src/services/preview/protocol_handler.rs` and register `hdpreview` in `src-tauri/src/main.rs` with `register_asynchronous_uri_scheme_protocol`: answers per contracts/tauri-commands.md "The `hdpreview` protocol" (the current token's bytes with the three headers; `GET /hooked/<surface secret>` → `204` and `hooked`; everything else `404`), holding the session lock only to copy out the bytes, and emitting `preview:pdf-ready { previewId }` with `emit_to("main", …)` once served (and on Linux, hooked) (depends on T048)
- [ ] T061 [US1] Add the PDF path to `open_preview` in `src-tauri/src/commands/preview.rs`: `PDF_PREVIEW_UNAVAILABLE` (with its reason's sentence) unless `PdfAvailability` is `Available`; bind the tripwire; build the surface when the viewer has none (hidden, at the last bounds sent) or take over the previous PDF's; a new token; navigate; return at once. Start the 5 s hook timer on Linux (no hook → close, `preview:pdf-ended { failed }`), treat a download of the surface's own URL as `NoViewer` (refuse it, set availability, `preview:pdf-ended { noViewer }`). Implement `set_preview_bounds` and `focus_preview` with their validation and `PREVIEW_CLOSED`; register both in `generate_handler!`, `COMMANDS` and the capability. Make T031 pass with the recording surface (depends on T060, T059, T058, T049)

#### PDF: the surface and its per-OS measures (research.md §4–§10)

- [ ] T062 [US1] Complete `src-tauri/src/services/preview/surface/mod.rs` as the app's `PreviewSurface`: label `preview`, incognito, proxy to the tripwire, `on_navigation` allowing only the current URL, `about:blank` and on Linux `webkit-pdfjs-viewer:`; `on_new_window` denied; `on_download` refused; developer tools off in release; the frame script with a fresh 128-bit surface secret written in; bounds and visibility; focus; close (and on Windows deleting `app_dirs::preview_webview_data_dir` after its browser process exits, and at startup); hidden while minimized. Native Escape and F6 emit `preview:escape` and `preview:focus-chrome` to `main`; activity calls `note_activity` at most once a second; make `note_activity` in `src-tauri/src/session/idle.rs` callable from there (depends on T061, T009)
- [ ] T063 [P] [US1] Write `src-tauri/src/services/preview/surface/frame_script.js` (an `initialization_script_for_all_frames`): in PDF.js's frame at `webviewerloaded`, set `enableScripting` and `enableXfa` to false and `annotationEditorMode` to disabled, then load `hdpreview://localhost/hooked/<secret>`; hide PDF.js's download and print buttons and replace `window.print`; cancel Ctrl+S, Ctrl+P, Ctrl+O and `contextmenu` (research.md §7, §8)
- [ ] T064 [US1] On Linux, implement `surface/linux.rs`: a `WebKitUserContentFilter` blocking every URL but `hdpreview:` and `webkit-pdfjs-viewer:`, compiled into a store under the app's cache folder; WebRTC set off; the `print` signal answered handled; the `context-menu` signal keeping only Copy and Select All; `key-press-event` for Escape and F6 and `button-press-event`, `scroll-event`, `key-press-event`, `motion-notify-event` for activity. Run `pdf_preview_test` and the Linux build in the dev container (depends on T062, T063)
- [ ] T065 [US1] On macOS, implement `surface/macos.rs`: the same rules as a `WKContentRuleList` under the cache folder; `proxy_url`; the HUD off through `-[WKPreferences _setEnabled:forFeature:]` (`PDFPluginHUDEnabled`) and WebRTC off through `_setPeerConnectionEnabled:`; a local `NSEvent` monitor for Escape, F6 and activity while the surface's view is first responder; and the watch: a `kqueue` on `$TMPDIR` (`EVFILT_VNODE`, `NOTE_WRITE`, through `libc`) that deletes each `WebKitPDFs-*` entry not there when the surface was created, records it in `leftovers`, sets the hold to this version, closes the surface and emits `preview:pdf-ended { copyCaught }`, with a last sweep when the surface's handle drops. Build and run `pdf_preview_test` in the macOS 26 VM (depends on T062, T057)
- [ ] T066 [US1] On Windows, implement `surface/windows.rs`: `WebResourceRequested` on `*` refusing every request but the surface's own URL; `HiddenPdfToolbarItems` hiding Save, Save As and Print; `SaveAsUIShowing` cancelled; `ContextMenuRequested` keeping only Copy; `AreBrowserAcceleratorKeysEnabled(false)` and `AreDevToolsEnabled(false)`; `AcceleratorKeyPressed` for Escape, F6 and key activity, and `GetLastInputInfo` sampled each second while HoploDex is the foreground window; each interface queried when the surface is built, its absence closing the surface with `failed`. Build and run `pdf_preview_test` on the Windows test machine (depends on T062)
- [ ] T067 [US1] On Linux, implement `src-tauri/src/services/preview/sandbox_probe.rs` and its call in `src-tauri/src/main.rs`: `--webkit-sandbox-probe` checked first (initialize GTK, create a web view, load `about:blank`, exit 0 on load-finished); before Tauri starts, skip in a container or Flatpak, else run the app with that argument and `WEBKIT_FORCE_SANDBOX=1` with a 5 s limit, and set `WEBKIT_FORCE_SANDBOX=1` for itself only on exit 0; log once whether the sandbox is on and why not; keep the result as `WebKitSandbox` state. Make T034 pass
- [ ] T068 [US1] In `src-tauri/src/main.rs`, manage `PdfAvailability`, the tripwire and `WebKitSandbox` as state; sweep `pdfPreviewHold.leftovers` and clear a stale hold version at startup and on the shutdown signals (SIGTERM/SIGHUP/SIGINT, OS shutdown), beside the opened-documents clean-up; hide the surface and close it on the session's close paths. Make T036 pass (depends on T062, T054)

#### The PDF surface check (research.md §23)

- [ ] T069 [US1] Complete `src-tauri/examples/pdf_surface_check.rs` on the app's own `services::preview::surface` code (not a copy), in a window under E2E's isolation: serve the hostile PDF (a unique marker, a link, a JavaScript open action, a remote image, a remote font, a form with a submit action, an embedded file, a launch action), then the truncated and the bit-flipped PDF of T004 (the viewer may fail on them but the process stays up and nothing is reached); run the reach probe from every frame a script reaches (on Windows also Edge's frames through the DevTools protocol): fetch, image, beacon, WebSocket and WebRTC to a local test service and to unresolvable names, a raw IPC request and a command call; with real input, click every toolbar button and context-menu item, press Ctrl/⌘+S, Ctrl/⌘+P, Ctrl/⌘+O, Escape and F6, scroll, and click the link; fail if the test service, the outside or (but for WebView2's own hosts) the tripwire saw anything, a command answered, Escape or F6 didn't reach the main web view, scrolling didn't call `note_activity`, or any file written during the run holds the marker (on macOS also any new `WebKitPDFs-*`); then serve a PDF of about 10 MB it builds (pages carrying a large incompressible image) and print its time to first paint (SC-001). A `--hud-on` variant (macOS) leaves the HUD on and passes only if the watch deletes Open in Preview's copy, closes the surface and sets the hold. Rename `pdf_spike_input.swift`, `pdf_spike_input.ps1` and `pdf_spike_dns.c` to `pdf_surface_input.swift`, `pdf_surface_input.ps1` and `pdf_surface_dns.c` and use them; delete `pdf_spike.rs`, `pdf_spike.sh`, `pdf_spike_mac.sh`, `pdf_spike_procs.sh` and `pdf_spike_win.ps1` and any `[[example]]` entry for them (depends on T064)
- [ ] T070 [P] [US1] Write `scripts/pdf-surface-check.sh` (Linux, in the dev container: Xvfb and XTest input, from `pdf_spike.sh`), `scripts/macos/pdf-surface-check.sh` (drives the macOS 26 VM from the Linux host with `scripts/tart-vm.sh`, with `--hud-on`, from `pdf_spike_mac.sh`) and `scripts/windows/pdf-surface-check.ps1` (in the Windows test machine's desktop session, from `pdf_spike_win.ps1`), each building the example, running it with scratch folders and exiting non-zero on any failure (depends on T069)
- [ ] T071 [US1] On Linux, run `scripts/dev-container.sh scripts/pdf-surface-check.sh` until it passes, and record the 10 MB PDF's time to first paint for the PR
- [ ] T072 [US1] On macOS, run `scripts/macos/pdf-surface-check.sh` until it passes, and record the 10 MB PDF's time to first paint for the PR (depends on T065)
- [ ] T073 [US1] On macOS, run `scripts/macos/pdf-surface-check.sh --hud-on` until the watch deletes the copy before Preview opens it, the surface closes with `copyCaught` and the hold is set; then restart with the same version and see PDFs not previewable, and with a different version string and see them previewable again (FR-003a) (depends on T072)
- [ ] T074 [US1] On Windows, run `scripts\windows\pdf-surface-check.ps1` until it passes, and record the 10 MB PDF's time to first paint for the PR (depends on T066)

#### Frontend

- [ ] T075 [P] [US1] In `src/components/Dialog.tsx` and its CSS, add size `"xl"` (90 vw × 90 vh), with a case in its test if one exists
- [ ] T076 [US1] In `src/features/media/mediaService.ts`, add `openPreview`, `setPreviewBounds`, `focusPreview`, `renderPreviewPage` (an `ArrayBuffer` from the binary response) and `closePreview`, and listeners for `preview:pdf-ready`, `preview:pdf-ended`, `preview:escape` and `preview:focus-chrome` that filter by `previewId` (depends on T023)
- [ ] T077 [P] [US1] Implement `src/features/media/useTiffPages.ts` per research.md §14 (visible pages ± one screen, width × DPR capped, 8-bitmap LRU with `blob:` URLs revoked, 150 ms zoom re-render, current page by visible area). Make T040 pass (depends on T076)
- [ ] T078 [P] [US1] Implement `src/features/media/PreviewSurface.tsx` per ui contract §2 "Page area" (a `ResizeObserver` and window resize sending bounds; `visible: false` while a dialog, menu or toast is over the viewer; "Preparing {name}…" until ready; the labelled region and F6 hint). Make T039 pass (depends on T076)
- [ ] T079 [US1] Implement `src/features/media/DocumentPreview.tsx` and its styles in `src/features/media/media.css`, laid out like `PhotoViewer` in `PhotoGallery.tsx`: the header, the TIFF toolbars, the page area for each kind (PDF through `PreviewSurface`, TIFF through `useTiffPages` with 16 px gaps and the page shadow token, text in `<pre class="hd-preview__text">` with `white-space: pre-wrap` and the mono token, shown in 1 MB chunks as the user scrolls past 1 MB), every state of ui contract §3, the footer (previous, next, "Delete document", and "Open in another app…" left for US2), the footer status line that takes toasts while a PDF is shown, the keyboard of ui contract §7 and the accessibility of §8, and cleanup on unmount. Make T038 pass (depends on T075, T077, T078)
- [ ] T080 [US1] In `src/features/media/DocumentList.tsx`, host `DocumentPreview` over the record; replace `kindLabel` with the `DocumentType` labels from `listDocumentTypes`; add the meta line suffixes, the "Preview" ghost button (`eye` icon), and the name opening the viewer; reload after `preview:pdf-ended` with `copyCaught` or `noViewer`. Make T041 and T042 pass (depends on T079)
- [ ] T081 [US1] Pass `HOPLODEX_E2E_PDF_PREVIEW` through to the app per spec in `e2e/wdio.conf.ts` (or `e2e/support/app.ts`, wherever the app's environment is set), and in `scripts/check-no-webdriver.mjs` add `HOPLODEX_E2E_PDF_PREVIEW` as a marker that must be absent from a release binary and present in an E2E one, checked both ways round like `TAURI_WEBDRIVER_PORT`
- [ ] T082 [US1] Make T043 pass: `npm run build && npm run test:e2e -- --spec e2e/specs/us13-document-preview.e2e.ts` in the dev container (depends on T080, T068, T064, T081, T025)

**Checkpoint**: User Story 1 is fully functional on its own: every previewable type is shown inside HoploDex on each OS, the surface reaches nothing, nothing reaches the disk, the preview ends with the session, and document names are searchable.

---

## Phase 4: User Story 2 - Open a Document in Another Program, Knowingly (Priority: P2)

**Goal**: "Open in another app…" shows the operating system's own confirmation naming the document and its consequences before any copy is written, writes the copy only on a "yes" and only into the session that asked, privately, under the type's canonical extension and marked as untrusted; non-document files are refused at attach, and pre-007 rows are never handed on.

**Independent Test**: Choose "Open in another app…" on a PDF; see the native confirmation with the document's name and the four consequences, and nothing on disk before answering; cancel and see nothing written; confirm and see the copy opened and deleted when the database closes. Drop an `.exe` and a JPEG on the Documents list and see each refused.

### Tests for User Story 2 (mandatory per constitution)

- [ ] T083 [P] [US2] Write failing `src-tauri/tests/open_document_test.rs` with a fake `Consent` (records each request, answers as told) and a fake opener (records the path, or fails as no-app):
  - the request names the document's stored filename, with control characters replaced and cut to 120 characters with an ellipsis, and its kind; with the default setting every open asks (FR-008, US2-1);
  - Cancel → `{ opened: false }`, no folder, no file, opener not called (US2-2, SC-004);
  - Open → `{ opened: true }`; the copy is named the stored stem plus the type's canonical extension ("receipt.Pdf" → `receipt.pdf`); on Unix the folders are `0700` and the file `0600` from creation; a symlinked or foreign-owned folder gives `INTERNAL_ERROR` and no file; the opener is called once; a second open reuses the same-length copy; `external_open_confirmed` is set; the copy is cleared on close and by the startup sweep (US2-3, FR-010);
  - a lock or close while the fake `Consent` is answering → `DATABASE_CLOSED`, no file, opener not called; and with a lock racing the write, either the copy is cleared by the close or never written (#65);
  - a failing opener → `NO_APP_FOR_DOCUMENT` and the copy gone (US2-5);
  - a raw-SQL `image/jpeg` row and a raw row of `.pdf` holding HTML are refused with `DOCUMENT_TYPE_NOT_ALLOWED` / `DOCUMENT_CONTENT_MISMATCH` before the fake `Consent` is called, and are still deletable (US2-7, FR-017);
  - the idle clock is paused while the dialog is up and resumed after; a PDF surface shown (recording surface) is hidden while it is up;
  - `services::consent`'s title and body for a document and for the setting match ui contract §4 word for word
- [ ] T084 [P] [US2] Extend `src/features/media/DocumentList.test.tsx` (US2 part): the picker's `accept` is built from `listDocumentTypes`; a dropped `.exe` is refused before any command with the toast of ui contract §6, and a dropped JPEG goes to Photos; a backend `DOCUMENT_TYPE_NOT_ALLOWED` for a photo shows "{name} is a photo. Add it under Photos instead."; the empty-state drop zone text; "Open in another app…" (ghost, `open` icon) shows `pending` while waiting, says nothing on `{ opened: false }`, toasts "Opened {name} in another app." on success and the `NO_APP_FOR_DOCUMENT` message of ui contract §1, and is disabled with its reason when `openable` is false. Extend `src/features/media/filePaths.test.ts`: `isDocumentPath` is "the extension is in the document types" (no longer "not a photo")
- [ ] T085 [P] [US2] Extend `src/features/media/DocumentPreview.test.tsx` (US2 part): the footer's "Open in another app…" calls `openDocument`, hides the PDF surface while waiting, and shows its result in the footer status line while a PDF is shown; the `PREVIEW_UNSUPPORTED`, `PDF_PREVIEW_UNAVAILABLE`, `failed` and `PREVIEW_DAMAGED` states offer it as their primary action, and `DOCUMENT_CONTENT_MISMATCH` doesn't (US2-4)
- [ ] T086 [P] [US2] Extend `e2e/specs/us13-document-preview.e2e.ts` (US2 part): "Open in another app…" on "Purchase receipt.pdf" with `HOPLODEX_E2E_CONSENT=cancel` writes nothing to the opened-documents folder and leaves the name in `HOPLODEX_E2E_CONSENT_LOG`; with `=open` the copy is written and named in `HOPLODEX_E2E_OPENED_LOG`; "Bill of sale.docx" opens the viewer on "can't be previewed here" with "Open in another app…"; "Old scan.jpg" has "Open in another app…" disabled; dropping an `.exe` is refused

### Implementation for User Story 2

- [ ] T087 [US2] Implement `src-tauri/src/services/consent.rs`: the `Consent` trait (`ask(&self, request: ConsentRequest) -> ConsentAnswer`); `ConsentRequest::{Document { name, kind, first_of_session_with_external }, Setting}` with the title and body text of ui contract §4 built here (the name sanitized and cut to 120 characters); `needs_consent(setting, confirmed_this_session)` = `true` unless `setting == External && confirmed_this_session` (research.md §17); the app's `DialogConsent` on `tauri-plugin-dialog`'s Rust API (`message(..).kind(Warning).title(..).buttons(OkCancelCustom("Open in another app", "Cancel"))`, parented to the main window, awaited through a oneshot channel off the session lock); and, in an `e2e` build only, `E2eConsent` reading `HOPLODEX_E2E_CONSENT=open|cancel` and appending each title to `HOPLODEX_E2E_CONSENT_LOG`. Manage the `Consent` as state in `src-tauri/src/main.rs`
- [ ] T088 [US2] In `src-tauri/src/session/mod.rs`, add `generation: u64` (from a per-run counter at each open or unlock) and `external_open_confirmed: bool` to `OpenDatabase`, and a helper that runs a closure under the session mutex only if the open database's generation is the one given, else `DATABASE_CLOSED`, without the pending-changes refusal (research.md §18; data-model.md)
- [ ] T089 [US2] Rewrite `open_document` in `src-tauri/src/commands/documents.rs` in contracts/tauri-commands.md's order of work, through `ops` that take the `Consent`, an `Opener` trait (the app's on `tauri-plugin-opener`, the E2E one appending to `HOPLODEX_E2E_OPENED_LOG` as today) and the session: read the document and the generation; `classify` (refusals before any dialog); if `needs_consent(machine_settings.document_opening(), confirmed)`, hide the PDF surface, pause idle (`IdlePauseReason`), ask, resume; on Cancel return `{ opened: false }`; under the generation helper, set `external_open_confirmed` on a "yes" and write or reuse the copy; release and call the opener; on a no-app error (`SE_ERR_NOASSOC`; `open`'s non-zero exit; `xdg-open`'s exit 3 or 4) securely delete the copy and return `NO_APP_FOR_DOCUMENT`. Replace `write_document_copy` with a private writer: `safe_file_name(stem)` plus the canonical extension; on Unix `DirBuilder`/`OpenOptions` with modes `0700`/`0600` and `create_new`, and an existing folder used only if a directory (not a symlink) owned by the user. Output `{ opened: boolean }`. Make T083 pass on Linux (depends on T087, T088, T048, T024)
- [ ] T090 [US2] On macOS, add a `com.apple.quarantine` attribute to each copy (`setxattr` through `libc`) in `src-tauri/src/commands/documents.rs`, with a `#[cfg(target_os = "macos")]` case in `open_document_test.rs` reading it back, and run `open_document_test` in the macOS 26 VM until it passes
- [ ] T091 [US2] On Windows, give each per-document folder a protected DACL granting only the current user (`SetSecurityInfo`) and each copy a `:Zone.Identifier` stream with `ZoneId=3` in `src-tauri/src/commands/documents.rs`, with `#[cfg(windows)]` cases in `open_document_test.rs` reading both back, and run `open_document_test` on the Windows test machine until it passes
- [ ] T092 [P] [US2] In `scripts/check-no-webdriver.mjs`, add `HOPLODEX_E2E_CONSENT` as a marker absent from a release binary and present in an E2E one, both ways round; in `e2e/wdio.conf.ts`, set `HOPLODEX_E2E_CONSENT_LOG` to a file in the sandbox and `HOPLODEX_E2E_CONSENT=open` by default, overridable per spec; update `e2e/specs/us4-photos-documents.e2e.ts` wherever it attaches a non-document type as a document or expects an open without a confirmation
- [ ] T093 [US2] In `src/features/media/mediaService.ts`, make `openDocument` return `{ opened: boolean }`; in `src/features/media/filePaths.ts`, make `isDocumentPath` take the document types; route drops in `src/features/media/DocumentList.tsx` (and `useFileDrop.ts` if it routes) per ui contract §6, with the picker's `accept` from `listDocumentTypes`, the refusal toasts and the new empty-state text; add "Open in another app…" to each row and to `DocumentPreview.tsx`'s footer and states, with `pending`, the result toasts (or the footer status line while a PDF is shown) and the disabled state. Make T084 and T085 pass (depends on T089, T080)
- [ ] T094 [US2] Make T086 pass, and rerun `us4-photos-documents.e2e.ts`, in the dev container (depends on T093, T092)

**Checkpoint**: User Stories 1 and 2 both work on their own: any document can be previewed or, after the native confirmation, opened in another program; nothing else can be attached or handed on.

---

## Phase 5: User Story 3 - Choose How Documents Open (Priority: P3)

**Goal**: A per-computer setting, "Preview in HoploDex" (default) or "Open in another app", chosen in Database settings; changing to "Open in another app" takes the native confirmation; with it set, opening a document hands it on, asking only for the first one in each database session.

**Independent Test**: Change the setting to "Open in another app", confirm in the native dialog, open two documents from a record (only the first asks), lock and unlock and open another (it asks again), choose "Preview" on a PDF (it previews), set the setting back (no question).

### Tests for User Story 3 (mandatory per constitution)

- [ ] T095 [P] [US3] Extend `src-tauri/tests/open_document_test.rs` (US3 part): `set_document_opening("external")` asks through the fake `Consent` with the setting's text, and Cancel keeps `"preview"` (`{ changed: false }`); `"preview"` never asks; setting the current value is `{ changed: false }`; the setting's dialog doesn't set `external_open_confirmed` (US3-2); with `"external"`, the first open asks with the extra line "You won't be asked again until this database is closed or locked." and the second doesn't; after lock and unlock, close and reopen, and switching to another database, the next open asks again (US3-3, US3-4); with `"preview"` every open asks
- [ ] T096 [P] [US3] Extend `src/features/databases/DatabaseSettingsDialog.test.tsx`: a "Documents" fieldset after "Locking" and before "This computer" with a `SegmentedControl` "Open documents" ("Preview in HoploDex", "Open in another app") and the hint "Applies to every database on this computer. Saved for this computer as soon as you choose."; choosing "Open in another app" calls `setDocumentOpening`, shows the old value as pending meanwhile, and stays at "Preview in HoploDex" on `{ changed: false }`; choosing "Preview in HoploDex" changes at once; neither waits for the dialog's Save (ui contract §5)
- [ ] T097 [P] [US3] Extend `src/features/media/DocumentList.test.tsx` (US3 part): with `"external"`, the name calls `openDocument` and its `title` is "Open {name} in another app"; "Preview" is still offered on previewable documents and opens the viewer (US3-5); after the setting changes back to `"preview"`, the name previews again (US3-6)
- [ ] T098 [P] [US3] Extend `e2e/specs/us13-document-preview.e2e.ts` (US3 part): change the setting in Database settings (the consent log has "Open documents in another app?"); open two documents by name (the consent log gains one entry, the opened log two); lock and unlock and open one more (a new consent entry); "Preview" on the PDF previews it; set the setting back

### Implementation for User Story 3

- [ ] T099 [US3] Add `get_document_opening` and `set_document_opening` to `src-tauri/src/commands/databases.rs` with their `ops`, on `MachineSettings` with no open database needed, per contracts/tauri-commands.md "Setting commands" (the setting's `Consent` request for `"external"`, `{ changed }`, no session flag); make `open_document`'s consent request carry `first_of_session_with_external`; register both in `generate_handler!`, `COMMANDS` and the capability. Make T095 pass (depends on T089)
- [ ] T100 [US3] Add `src/features/media/documentOpening.ts`: a small store holding this computer's `DocumentOpening`, loaded with `getDocumentOpening` once the collection's providers mount and updated by `setDocumentOpening`; add both wrappers to `src/features/media/mediaService.ts` (depends on T099)
- [ ] T101 [P] [US3] Add the "Documents" fieldset to `src/features/databases/DatabaseSettingsDialog.tsx` per ui contract §5, saving through the store at once, outside the dialog's Save. Make T096 pass (depends on T100)
- [ ] T102 [P] [US3] Make the name in `src/features/media/DocumentList.tsx` follow the store's setting (preview or `openDocument`, with its `title`), keeping "Preview" and "Open in another app…" on every row. Make T097 pass (depends on T100, T093)
- [ ] T103 [US3] Make T098 pass in the dev container (depends on T101, T102)

**Checkpoint**: All three stories work on their own.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Performance, screenshots, documentation, cross-cutting checks, the gates on every OS, the manual checks, and the pull request's notes.

### Performance (constitution IV, research.md §22)

- [ ] T104 Extend `src-tauri/tests/performance_test.rs` with research.md §22's rows, each against its 10,000-firearm, 10,000-accessory database (SC-001's collection size): `open_preview` → page 1 PNG for a 10 MB TIFF within 1 s (real helper); `open_preview` of a 10 MB PDF up to the surface's navigation (recording surface) and the `hdpreview` handler serving it; a 10 MB text decoded within budget; `list_firearms` and `list_accessories` searching a document name within 500 ms at 10,000 firearms, 10,000 accessories and 30,000 seeded documents (SC-001, SC-007)
- [ ] T105 On Linux, run `scripts/dev-container.sh cargo test --manifest-path src-tauri/Cargo.toml --release --test performance_test` (as DEVELOPMENT.md runs it) and record each timing of T104, with the us13 E2E time to `preview:pdf-ready` and T071's first paint, for the PR's performance note; fix any operation over its budget
- [ ] T106 On macOS, run the release `performance_test` in the macOS 26 VM and record its timings against the budgets, with T072's first paint, for the PR and #26
- [ ] T107 On Windows, run the release `performance_test` on the Windows test machine and record its timings against the budgets, with T074's first paint, for the PR and #26
- [ ] T108 On Linux, on a host (not the container) where WebKit's sandbox works, measure the sandbox probe's startup cost (reconsider if over 500 ms) and run the full E2E suite with the sandbox on, one spec at a time; record both for the PR (research.md §9, §22)

### Screens, docs and checks

- [ ] T109 [P] Add to the walk in `e2e/screenshots/screens.e2e.ts`, continuing its numbering, in light and dark, every screen of ui contract §10: the Glock's document list (a PDF, the TIFF, "Bill of sale.docx", "Old scan.jpg"); the viewer on the 3-page PDF, taken from the X display (`import -window`) so the surface is in it; the viewer on the multi-page TIFF; on the text; on "can't be previewed here" for the DOCX; on "PDFs can't be previewed on this computer" with `HOPLODEX_E2E_PDF_PREVIEW=off`; and Database settings with the "Documents" fieldset. Run `scripts/dev-container.sh bash -c 'npm run build && npm run screenshots'` and keep before/after images for the PR
- [ ] T110 [P] Add a one-line "Amended by 007 (`specs/007-document-preview/`)" pointer at each amended anchor listed in plan.md's Project Structure: in `specs/001-firearms-inventory/` (spec.md FR-010, FR-013 and User Story 4 scenario 4; data-model.md's DocumentAttachment; contracts/tauri-commands.md's `add_document`, `open_document`, `list_documents` and `list_firearms`); in `specs/003-database-protection-management/` (FR-013's list of per-computer settings; `machine.json` in data-model.md); in `specs/006-accessory-links/` (FR-007a, User Story 1 scenario 12, FR-018; `list_accessories` and the document commands' `owner` in contracts/tauri-commands.md)
- [ ] T111 [P] Update `CLAUDE.md`'s Architecture section for 007: `services/` gains `document_types` (the one allowlist, `classify` at attach, preview and open), `consent` (`Consent`, `needs_consent`, the dialog text) and `preview/` (the `Preview` in `OpenDatabase`, `hdpreview`, the PDF surface with one file per OS, the TIFF render helper started as `hoplodex --render-helper`, `PdfAvailability` and the hold, the Linux sandbox probe); `commands/preview.rs`; a new command must be added to `generate_handler!`, `commands_list.rs`'s `COMMANDS` and `capabilities/default.json` (`acl_manifest_test`); the main window is built in `setup()`; `app_dirs` gives the web views' data folders; the IPC paragraph's codes gain 007's; the frontend's `DocumentPreview`, `PreviewSurface` and `documentOpening`; add `DocumentPreview` to the UI-consistency list. Keep it concise and in CLAUDE.md's voice
- [ ] T112 [P] Update `DEVELOPMENT.md`: the PDF surface check on each OS (its three scripts, the `--hud-on` variant, when to run it); the two new E2E seams (`HOPLODEX_E2E_CONSENT`/`_CONSENT_LOG`, `HOPLODEX_E2E_PDF_PREVIEW`) and that `check-no-webdriver.mjs` keeps them out of release binaries; that `WEBVIEW2_USER_DATA_FOLDER` is no longer set; WebKit's sandbox on a Linux host and why the container runs without it; remove the `pdf_spike` instructions
- [ ] T113 Check the cross-cutting rules in the code and fix anything found: no PDF or TIFF bytes reach the main web view (`grep -rn 'file_bytes\|fileBytes' src-tauri/src/commands src` shows only the attach input); the frontend keeps no list of document types or extensions (`grep -rniE '\.(pdf|tiff?|docx?|xlsx?|od[ts])\b' src --exclude='*.test.*'`); no `mimeType` argument or `mime_type_for` call remains on the document path; every `#[tauri::command]` is in `COMMANDS` and the capability (`acl_manifest_test`); the E2E seams are only under `#[cfg(feature = "e2e")]`; no new migration file (`ls src-tauri/src/db/migrations`); the main window's CSP is unchanged in `tauri.conf.json`; no `examples/pdf_spike*` remains
- [ ] T114 On Linux, run the full gates through `scripts/dev-container.sh`: `cargo test --manifest-path src-tauri/Cargo.toml`, `npm test`, `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets`, `cargo fmt --manifest-path src-tauri/Cargo.toml --check`, `npm run lint`, `npm run format:check`, `npm run audit`, `scripts/pdf-surface-check.sh`, and `npm run build && npm run test:e2e` one spec file at a time (us1–us13, `us7-databases-no-keyring`, `ui-review`); fix every failure (constitution, Development Workflow)
- [ ] T115 On macOS, in the macOS 26 VM, run the build, `cargo test`, `npm test`, lint and the audits, all 15 E2E spec files and the screenshot walk; fix every failure and record the result for #26 (FR-022)
- [ ] T116 On Windows, on the Windows test machine, run the build, `cargo test`, `npm test`, lint and the audits, all 15 E2E spec files and the screenshot walk; fix every failure and record the result for #26 (FR-022)
- [ ] T117 On Linux, run `npm run audit:webdriver` against the release binary and the AppImage's binary, and confirm `TAURI_WEBDRIVER_PORT`, `HOPLODEX_E2E_CONSENT` and `HOPLODEX_E2E_PDF_PREVIEW` are absent from both and present in the E2E binary
- [ ] T118 On Windows, run `npm run audit:webdriver` through Node, passing the release `hoplodex.exe`, and record the result for #29
- [ ] T119 Record in `specs/001-firearms-inventory/` (spec.md's FR-022 and quickstart.md) that FR-022 is verified on Linux, Windows and macOS, citing T114, T115 and T116, and bring `specs/001-firearms-inventory/plan.md` up to date where it is stale (the embedded WebDriver, not `tauri-driver`; macOS 26, Apple silicon only) (plan.md Close-out, #26)
- [ ] T120 Check quickstart.md end to end (every scenario-map row has its test, and `human_seed_coverage_test` passes with `NEVER_SEEDED` unchanged); tell the user their development databases must be recreated (`0002_fts5.sql` was edited in place); draft the PR notes: before/after screenshots from T109; how the persistence and security changes meet Security & Data Handling and User Privacy (research.md §1, §2, §18, §19; the ACL manifest; no cipher change; deleted names merged out of the index; external copies private and marked; plan.md's Constitution Check and Complexity Tracking); the performance figures of T105, T106 and T107 against Principle IV; T108's sandboxed E2E run; the surface check results on each OS; the residual risks of research.md §1

### Manual checks (best effort before a release, not merge gates)

- [ ] T121 [P] On Linux, do quickstart.md's manual check **M1** (the native confirmation is the system's and blocks the window) with `scripts/dev-container.sh --gui scripts/human-testing.sh`, and record the result for the PR
- [ ] T122 [P] On macOS, do quickstart.md's manual check **M1** with `scripts/human-testing.sh` in the macOS 26 VM, and record the result for the PR
- [ ] T123 [P] On Windows, do quickstart.md's manual check **M1** on the Windows test machine, and record the result for the PR
- [ ] T124 [P] On Linux, do quickstart.md's manual check **M2** (no app for the type), and record the result for the PR
- [ ] T125 [P] On macOS, do quickstart.md's manual check **M2**, and record the result for the PR
- [ ] T126 [P] On Windows, do quickstart.md's manual check **M2**, and record the result for the PR
- [ ] T127 [P] On Windows, do quickstart.md's manual check **M3**, steps 1–2 (Word opens the copy in Protected View), and record the result for the PR
- [ ] T128 [P] On macOS, do quickstart.md's manual check **M3**, steps 3–4 (the copy carries `com.apple.quarantine`), and record the result for the PR
- [ ] T129 [P] On Linux, do quickstart.md's manual check **M4** (a screen reader reads a previewed PDF) with Orca, and record the result for the PR
- [ ] T130 [P] On macOS, do quickstart.md's manual check **M4** with VoiceOver, and record the result for the PR
- [ ] T131 [P] On Windows, do quickstart.md's manual check **M4** with NVDA, and record the result for the PR
- [ ] T132 [P] On Linux, do quickstart.md's manual check **M5** (a password-protected PDF uses the viewer's own prompt), and record the result for the PR
- [ ] T133 [P] On macOS, do quickstart.md's manual check **M5**, and record the result for the PR
- [ ] T134 [P] On Windows, do quickstart.md's manual check **M5**, and record the result for the PR
- [ ] T135 [P] On macOS, do quickstart.md's manual check **M6** (Spotlight indexes nothing from a preview), and record the result for the PR

### Close-out (at pull request time)

Done when the pull request is opened, not before (CLAUDE.md; plan.md "Close-out (at pull request time)").

- [ ] T136 Open one follow-up issue with `gh issue create` for the checks #26, #27 and #28 leave: Windows human testing (the `sandbox-dirs` split, WebView2's profile, the seed's safety check, `human-testing.ps1`, the keyring question); sleep, lock and shutdown checked by hand on Windows; sleep, wake, screen lock and log-out or shut-down by hand on a real Mac (a guest can't sleep); saved passphrases in the Keychain under `human-testing.sh`'s `HOME`; and the 001 manual sleep, lock and shutdown checks
- [ ] T137 On issue **#13**, link the pull request (it closes the issue on merge) and reply to each security finding with how it is addressed (native consent, FR-008, research.md §16; the allowlist by content at attach and at open, FR-016, FR-017, §2; canonical extensions, private copies and Zone/quarantine marks, §18; no PDF or TIFF parser in the main web view and the PDF surface's ACL, network, disk and scripting measures, §1, §5–§9; the network consequence in the dialog, FR-009 (c)), noting the residual risks (the once-per-session choice, Linux without WebKit's sandbox, macOS's PDF code in a frame with Tauri's script), with `gh issue comment 13`
- [ ] T138 [P] On issue **#65**, comment that the copy is now written under the session lock and only into the session that asked (research.md §18), naming the regression test in `open_document_test.rs`; the pull request closes it
- [ ] T139 [P] On issue **#71**, comment with the Unix modes from creation, the symlink and owner check and the Windows DACL (§18), naming their tests; the pull request closes it
- [ ] T140 On issue **#26**, comment with T119's record, the release performance results of T106 and T107 against the budgets, and the follow-up issue's link; tick #27, #28 and #29; take CI (#25) off its list, saying it stays open on its own; the pull request closes it (depends on T136)
- [ ] T141 [P] On issue **#27**, comment that the harness is isolated (7810a4e) and ported (3a90476, 47a3bab), the power and session messages are tested against the hidden window (47a3bab), and `scripts/windows/` with DEVELOPMENT.md's Windows section replaces the dropped VM scripts; link the follow-up for Windows human testing and the manual sleep, lock and shutdown checks; the pull request closes it (depends on T136)
- [ ] T142 [P] On issue **#28**, comment that the harness is isolated and ported, `scripts/tart-vm.sh` and DEVELOPMENT.md's macOS section are done, the screenshot walk, `quit-cleanup.py` and `human-testing.sh` run on macOS (b438ebf, a1402d0), and the sleep, wake, lock and power-off handlers are tested (37a6ba5); link the follow-up for the checks on a real Mac and the Keychain; the pull request closes it (depends on T136)
- [ ] T143 [P] On issue **#29**, bring the status at the top up to date (the embedded driver runs the E2E suite and the screenshot walk on Linux, macOS and Windows; both outstanding boxes ticked: all spec files pass on each, the real-input tests pending off Linux) with `gh issue edit 29`, and comment that `npm run audit:webdriver` runs on Windows through Node (3ae66d5) with T118's result; the pull request closes it
- [ ] T144 [P] On issue **#21**, comment what the release security review must cover: the app ACL manifest and the three-list test; the PDF surface's measures on each OS, the WebKit SPI and its startup check, the macOS watch and hold, and the tripwire; the Linux sandbox probe; the TIFF helper and its confinement on each OS; the consent and PDF-availability seams' absence from release builds; `document_types::classify` and the external-copy hardening; and list research.md §1's residual risks
- [ ] T145 [P] On issue **#73**, comment that the app ACL manifest now refuses every command to any page other than the main window's, which narrows its impact, and that the PDF surface has its own navigation allowlist; the main window's own navigation is unchanged, so #73 stays open
- [ ] T146 [P] On issue **#40**, comment that 007 suppresses the context menu and browser shortcuts in the PDF surface only; the main window is unchanged
- [ ] T147 [P] On issue **#42**, comment that WebKit's sandbox doesn't run inside Flatpak, so a Flatpak build previews PDFs without it, under Flatpak's own sandbox (research.md §9)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: none.
- **Foundational (Phase 2)**: depends on Setup; BLOCKS every user story. Within it, first task 1 (T005 → T009) must settle before any later surface task, and first task 2 (T011) before any new command is registered.
- **US1 (Phase 3)**: depends on Foundational only. The MVP.
- **US2 (Phase 4)**: depends on Foundational and on part of US1's backend. Its tests (T083–T086) can be written alongside US1's. Its backend needs US1's `Preview` slot and `PreviewSurface` trait (T048), to hide the PDF surface while asking (a no-op when no preview exists), and the recording surface of T031's tests; and it follows US1's `main.rs` work (T068) and `session/mod.rs` work (T048) in the same-file order below. Its frontend (T093) adds "Open in another app…" to US1's viewer and list, so it follows T080.
- **US3 (Phase 5)**: depends on US2's `open_document` and consent (T089) and its list actions (T093).
- **Polish (Phase 6)**: depends on all three stories. Close-out happens when the pull request is opened.

### Within Each User Story

- Tests first, failing, then implementation (constitution II).
- Backend `ops` before the frontend services that call them; services before components; `PreviewSurface` and `useTiffPages` before `DocumentPreview`, and `DocumentPreview` before `DocumentList` hosts it.
- Each per-OS surface or confinement task follows the common code it plugs into.
- Same-file tasks are sequenced by hand:
  - `src-tauri/src/main.rs`: T011 → T014 → T022 → T049 → T053 → T054 → T058 → T060 → T061 → T067 → T068 → T087 → T099
  - `src-tauri/src/commands_list.rs` and `capabilities/default.json`: T011 → T022 → T049 → T054 → T061 → T099
  - `src-tauri/src/commands/documents.rs`: T020 → T021 → T022 → T045 → T049 → T089 → T090 → T091
  - `src-tauri/src/commands/preview.rs`: T049 → T054 → T061
  - `src-tauri/src/services/preview/mod.rs`: T048 → T053
  - `src-tauri/src/services/preview/availability.rs`: T021 → T058
  - `src-tauri/src/services/preview/surface/mod.rs`: T005 → T062
  - `src-tauri/src/services/preview/confine.rs`: T052 → T055 → T056
  - `src-tauri/src/services/machine_settings.rs`: T024 → T057
  - `src-tauri/src/session/mod.rs`: T048 → T088
  - `src-tauri/Cargo.toml`: T001 → T069
  - `src-tauri/examples/pdf_surface_check.rs`: T005 → T069
  - `src-tauri/examples/human_seed.rs`: T020 → T025
  - `src-tauri/tests/document_types_test.rs`: T017 → T022
  - `src-tauri/tests/machine_settings_test.rs`: T024 → T035
  - `src-tauri/tests/open_document_test.rs`: T083 → T090 → T091 → T095
  - `e2e/wdio.conf.ts`: T014 → T081 → T092
  - `scripts/check-no-webdriver.mjs`: T081 → T092
  - `e2e/specs/us13-document-preview.e2e.ts`: T043 → T086 → T098
  - `src/features/media/mediaService.ts`: T020 → T023 → T076 → T093 → T100
  - `src/features/media/DocumentList.tsx`: T020 → T080 → T093 → T102
  - `src/features/media/DocumentList.test.tsx`: T041 → T084 → T097
  - `src/features/media/DocumentPreview.tsx`: T079 → T093
  - `src/features/media/DocumentPreview.test.tsx`: T038 → T085

### Parallel Opportunities

- Setup: T003 and T004 alongside T001 and T002.
- Foundational: the three per-OS proofs (T006, T007, T008) on their own machines at once; T017, T019, T023 and T024 alongside the ACL and main-window work.
- US1 tests T026 through T043 are all different files and can be written together.
- US1 implementation: search (T044 → T046), text (T047), the TIFF helper (T050, T051, T052) and the PDF pieces (T057, T059, T063) proceed side by side; the three per-OS surface tasks and the two other confinement branches run on their own machines once the common code is in; T075, T077 and T078 alongside the backend.
- US2 tests T083 through T086 together; T090 and T091 on their machines at once; T092 alongside T089.
- US3 tests together; T101 and T102 together.
- Polish: T109, T110, T111 and T112 together; the per-OS gate, performance and manual-check tasks on their own machines; close-out comments T138 through T147 together after T136.

---

## Parallel Example: User Story 1

```bash
# Failing tests, all different files:
Task: "Extend fts_search_test.rs and list_accessories_test.rs with document names"   # T026, T027
Task: "Write preview_text_test.rs"                                                    # T030
Task: "Write pdf_preview_test.rs with a recording PreviewSurface"                     # T031
Task: "Write tiff_preview_test.rs and tiff_helper_test.rs"                            # T032, T033
Task: "Write webkit_sandbox_test.rs"                                                  # T034
Task: "Write DocumentPreview, PreviewSurface and useTiffPages tests"                  # T038, T039, T040
Task: "Write the failing us13 E2E spec"                                               # T043

# Then side by side:
Task: "document_names_fts and the search joins"                                       # T044, T046
Task: "preview/text.rs"                                                               # T047
Task: "helper_protocol.rs and tiff.rs"                                                # T050, T051
Task: "tripwire.rs and frame_script.js"                                               # T059, T063
Task: "Dialog xl, useTiffPages and PreviewSurface"                                    # T075, T077, T078

# Once surface/mod.rs is complete, on three machines at once:
Task: "On Linux, surface/linux.rs"                                                    # T064
Task: "On macOS, surface/macos.rs and the watch"                                      # T065
Task: "On Windows, surface/windows.rs"                                                # T066
```

## Parallel Example: User Story 2

```bash
Task: "Write open_document_test.rs with fake Consent and opener"                      # T083
Task: "Extend DocumentList, filePaths and DocumentPreview tests"                       # T084, T085
Task: "Extend us13 with the consent seam"                                             # T086

# After T089, on two machines at once:
Task: "On macOS, the quarantine attribute"                                            # T090
Task: "On Windows, the DACL and Zone.Identifier"                                      # T091
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Phase 1 Setup and Phase 2 Foundational, settling the child web view on all three OS first.
2. Phase 3: User Story 1.
3. **STOP and VALIDATE**: run `pdf_preview_test`, `tiff_preview_test`, `tiff_helper_test`, `preview_text_test`, `webkit_sandbox_test`, `lock_test`, `fts_search_test`, `list_accessories_test`, `deletion_wipe_test`, `csp_test`, `acl_manifest_test`, the frontend tests and us13; the surface check on each OS.
4. Demo: preview the Glock's receipt, its appraisal scan and its notes, the Leupold's receipt from its own record; lock with a PDF shown; search "appraisal".

### Incremental Delivery

1. Foundational → the surface proved, every command held to `main`, documents recorded by content.
2. US1 → previewing inside HoploDex (MVP).
3. US2 → opening in another app after the native confirmation, attach refusals.
4. US3 → the per-computer setting and its once-per-session confirmation.
5. Polish → performance on each OS, screenshots, amendment pointers, CLAUDE.md and DEVELOPMENT.md, the cross-cutting checks, the gates on all three OS, the manual checks; close-out with the pull request.

Each story adds value without breaking the previous ones; stop at any checkpoint to validate.

## Notes

- [P] tasks = different files, no dependencies on incomplete tasks
- [Story] label maps a task to its user story for traceability
- Verify tests fail before implementing
- Every new command is registered in three places (`generate_handler!`, `COMMANDS`, the capability); `acl_manifest_test` fails otherwise
- Commit after each task or logical group; never touch the real databases
