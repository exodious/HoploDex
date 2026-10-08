# Quickstart: Validate Document Preview and Consent

This is a runnable validation guide, not an implementation spec. It maps
each user story's independent test and each success criterion in
[spec.md](./spec.md) to the automated tests that cover it. Names refer to
[data-model.md](./data-model.md), [research.md](./research.md) and
[contracts/](./contracts). Setup is the same as
[001's quickstart](../001-firearms-inventory/quickstart.md).

## Prerequisites

- The development container (`scripts/dev-container.sh`; see
  DEVELOPMENT.md). Nothing is fetched: no new binary, and every new crate
  comes through `cargo`.
- A **fresh database**: `0002_fts5.sql` was edited in place (and `0001_initial.sql`, for the accessories' make and model indexes). Nothing here
  opens the real databases. Every test, E2E run, surface check and
  screenshot uses throwaway locations (DEVELOPMENT.md, "Test isolation").
- For the macOS and Windows surface checks: the macOS 26 VM and the
  Windows test machine (DEVELOPMENT.md; `scripts/windows/*.ps1`).

## Automated test commands

Run these through the container wrapper on a host with podman (CLAUDE.md):

```bash
scripts/dev-container.sh cargo test --manifest-path src-tauri/Cargo.toml
scripts/dev-container.sh npm test
scripts/dev-container.sh bash -c 'cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets \
  && cargo fmt --manifest-path src-tauri/Cargo.toml --check && npm run lint && npm run format:check'
scripts/dev-container.sh npm run audit
scripts/dev-container.sh bash -c 'npm run build && npm run test:e2e -- --spec e2e/specs/us13-document-preview.e2e.ts'
scripts/dev-container.sh scripts/pdf-surface-check.sh
scripts/dev-container.sh bash -c 'npm run build && npm run screenshots'
```

On the other systems, for this feature's pull request and whenever the
surface code changes (research.md §23):

```bash
scripts/macos/pdf-surface-check.sh              # from the Linux host, drives the macOS 26 VM
scripts/macos/pdf-surface-check.sh --hud-on     # FR-003a: the watch catches Open in Preview's copy
```

```powershell
scripts\windows\pdf-surface-check.ps1           # in the Windows test machine's desktop session
```

One file at a time while working:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test tiff_preview_test
npx vitest run src/features/media/DocumentPreview.test.tsx
```

All of these MUST pass before merge (constitution, Development Workflow).
Write each test to fail first (red-green, constitution II).

## Scenario map: story → tests

New test files are marked *(new)*. Backend tests run the real `ops`
against a temporary SQLCipher database; TIFF tests run the real helper
(the built `hoplodex` binary, `CARGO_BIN_EXE_hoplodex`).

| Story / criterion | What is checked | Where it is automated |
|---|---|---|
| US1-1, FR-005: PDF preview | `open_preview` on a 3-page PDF returns `kind: "pdf"`; the protocol serves exactly the document's bytes at the token's URL with `no-store`, then 404 after close; `preview:pdf-ready` follows the serve. The viewer shows the name, kind and date, and "Preparing…" until ready | `tests/pdf_preview_test.rs` *(new)* (protocol handler, recording `PreviewSurface`); `DocumentPreview.test.tsx` *(new)* |
| US1-1 on each OS: the viewer shows it | the PDF renders in the surface; time to first paint | the surface check (`pdf-surface-check`), all three OS |
| US1-2, US1-3: TIFF | Single-page: fit, zoom, actual size. 4-page (LZW, Deflate, CCITT G4 and JPEG pages): 4 sizes, each page renders; one undecodable page fails alone | `tests/tiff_preview_test.rs` *(new)*; `DocumentPreview.test.tsx` (toolbars) |
| US1-4: text and CSV | UTF-8, UTF-8 BOM, UTF-16 LE/BE, Windows-1252; control characters replaced; `<b>`, `=SUM(A1)` and a URL shown literally; CSV not laid out | `tests/preview_text_test.rs` *(new)*; `DocumentPreview.test.tsx` (a text node in a `<pre>`, no `<a>`, no `<table>`) |
| US1-5, FR-003, SC-002: nothing on disk | A unique marker in one document of each previewable type; the isolated XDG and temp folders and the system temp folder scanned during and after a real preview. For PDF, also after every toolbar button, menu item and shortcut of the viewer | `e2e/specs/us13-document-preview.e2e.ts` *(new)* (PDF, TIFF, text in the real app); the surface check (every control, each OS); `tests/tiff_preview_test.rs` (helper writes nothing) |
| FR-003a: the viewer writes a copy anyway (macOS) | With the HUD left on, Open in Preview's copy is deleted before Preview opens it, the surface closes with `copyCaught`, `pdfPreviewHold` is set to this version, and a restart with the same version shows PDFs as not previewable; a different version clears it; leftovers are swept at close and start, and other `WebKitPDFs-*` folders are left alone | the surface check `--hud-on` (macOS); `tests/pdf_preview_test.rs` (hold, version change, sweep of recorded paths only, on a scratch folder) |
| FR-003a: the startup check fails | PDFs are `previewAvailable: false`, `open_preview` gives `PDF_PREVIEW_UNAVAILABLE`, TIFF and text still preview | `tests/pdf_preview_test.rs` (availability states); `DocumentList.test.tsx`, `DocumentPreview.test.tsx`; E2E with `HOPLODEX_E2E_PDF_PREVIEW=off` |
| FR-004, SC-003: hostile documents | **PDF**: the surface check's PDF (JavaScript open action, link, remote image, remote font, form submit, embedded file, launch action) reaches neither the local test service, the outside, a command nor the disk; clicking the link sends nothing. A truncated and a bit-flipped PDF reach the viewer, which reports them in its own words; the app stays up and "Open in another app…" stays offered (FR-006). **TIFF**: truncated and bit-flipped files, 2^31 pages, a 100,000 × 100,000 page: each fails cleanly, the app process alive. **Mismatch**: a "PDF" that is HTML is refused before any surface exists | the surface check (each OS); `us13` E2E (damaged PDFs); `tests/tiff_preview_test.rs`; `tests/document_types_test.rs` |
| FR-004: no commands from the surface | Every command is refused to a web view other than `main`; the three command lists agree | the surface check (a command call from the surface); `tests/acl_manifest_test.rs` *(new)* |
| FR-004: WebKit's sandbox (Linux) | The probe's decisions: exit 0 → on; crash, non-zero, time-out, container, Flatpak → off, never forced | `tests/webkit_sandbox_test.rs` *(new)* (decision logic); the full E2E suite once on a Linux host with the sandbox on (noted in the pull request) |
| TIFF helper confinement | The helper can't open a file or a socket after loading (Linux Landlock in the container); it dies when its parent does; no core dump | `tests/tiff_helper_test.rs` *(new)* |
| US1-6, FR-007: previous and next | Only the record's own documents, in list order; an accessory's documents never reached from its host firearm; moving PDF → PDF keeps the surface, PDF → TIFF closes it | `DocumentPreview.test.tsx`; `tests/pdf_preview_test.rs` (surface kept and navigated, token revoked); `us13` E2E (a firearm with a mounted accessory with documents) |
| US1-7: close | Escape on the controls and `preview:escape` from the surface; the close control; focus returns to the opener | `DocumentPreview.test.tsx`; the surface check (Escape in the surface emits the event, each OS) |
| Keyboard into and out of a PDF | F6 calls `focus_preview`; `preview:focus-chrome` moves focus to the first control | `DocumentPreview.test.tsx`; the surface check (F6 in the surface, each OS) |
| US1-8, FR-014, SC-006: lock and close | Preview dropped and its surface closed (recording `PreviewSurface`) or helper killed (PID gone) after lock, close, switch, sleep and shutdown; viewer unmounted before the chooser; `blob:` URLs revoked | `tests/pdf_preview_test.rs`, `tests/tiff_preview_test.rs`; `tests/lock_test.rs` (+ a preview open at each lock cause); `SessionProvider.test.tsx` (+ viewer gone before the chooser); `us13` E2E (lock with a PDF shown: the window screenshot shows no PDF once the chooser is up) |
| Idle while reading a PDF | Input in the surface calls `note_activity`; no input lets the idle lock fire | the surface check (scrolling in the surface keeps the idle clock fresh, each OS); `us13` E2E (idle lock with a PDF shown, `HOPLODEX_E2E_IDLE_MINUTE_SECONDS`) |
| US1-9, FR-015, SC-007: search by document name | "appraisal" finds the firearm; gone after the document is deleted; the Accessories page finds the accessory by "receipt", the collection doesn't find its host; 1–2 character `LIKE` path; deleting the record cascades the index | `tests/fts_search_test.rs`, `tests/list_accessories_test.rs` (+ document names); `tests/deletion_wipe_test.rs` (+ a unique document filename absent from the raw file after `delete_document` and after deleting the record) |
| US2-1, US2-2, FR-008: native confirmation | With the fake `Consent`: asked with this document's stored name; Cancel → `{ opened: false }`, no folder, no file, opener not called | `tests/open_document_test.rs` *(new)* |
| US2-3, FR-010: confirmed open | Copy under the canonical extension, `0600` in `0700` folders created that way (Unix), a protected DACL (Windows), Zone mark (Windows) or quarantine (macOS); a symlinked or foreign-owned folder refused; opener called once; cleared on close and at startup | `tests/open_document_test.rs` (fake opener records the path); `tests/document_test.rs` (FR-035 clean-up still holds) |
| Lock during the dialog (#65) | A lock or close while the fake `Consent` is answering → `DATABASE_CLOSED`, no file written, opener not called; and with a lock racing the write, either the copy is cleared by the close or never written | `tests/open_document_test.rs` |
| US2-4: unpreviewable type | Word document: `PREVIEW_UNSUPPORTED`; the viewer shows the state with "Open in another app…" | `tests/pdf_preview_test.rs`; `DocumentPreview.test.tsx` |
| US2-5: no app | Opener error → `NO_APP_FOR_DOCUMENT`, the copy gone | `tests/open_document_test.rs` (fake opener fails); manual check M2 |
| US2-6, FR-016, SC-008: attach refusals | `.exe` (PE header), `.bat`, `.ps1`, `.lnk`, `.html`, `.svg`, `.jpg`, `.png`, `.gif`, `.heic`, `.docm`, `.docx` with `vbaProject.bin`, `.doc` with `Macros` storage, `.pdf` holding HTML, `.txt` starting with `<!DOCTYPE html>`; and, since 2026-10-07, an RTF with an object or `\template`, and a DOCX or XLSX with an OLE object, ActiveX, an external workbook link, or an outside template, picture or frame (character-escaped `TargetMode`, DTD and UTF-16 parts included): each refused with the right code; nothing stored; recorded type canonical for every accepted type; hyperlinks, a chart's workbook and RTF fields still accepted | `tests/document_types_test.rs` *(new)* (`refuses_every_hostile_document_with_its_code` over `hostile::refused_documents()`, `hyperlinks_pictures_charts_and_rtf_fields_are_accepted`); `tests/document_test.rs` (`refuses_what_is_not_a_document_and_stores_nothing`, + both attach paths); `tests/open_document_test.rs` (`a_row_the_attach_rules_would_refuse_is_refused_before_the_dialog_and_can_be_deleted`, whose remote-template DOCX is a row attached before the amendment, SC-008); `DocumentList.test.tsx` (picker `accept`, drop refusal, photo message); `filePaths.test.ts` |
| US2-7, FR-017: pre-007 rows | A raw-SQL row of type `image/jpeg` and one `.pdf` holding HTML: `open_document` refuses before asking (the fake `Consent` never called); `openable` false; still deletable | `tests/open_document_test.rs`; `DocumentList.test.tsx` |
| US3-1, FR-011: default | Fresh `machine.json` and one without the field read `"preview"` | `tests/machine_settings_test.rs` (+ `documentOpening`, `pdfPreviewHold`) |
| US3-2, FR-012: changing the setting | `"external"` asks through `Consent`; Cancel keeps `"preview"`; `"preview"` never asks; the change doesn't set the session flag | `tests/open_document_test.rs`; `DatabaseSettingsDialog.test.tsx` |
| US3-3, US3-4: once per session | With `"external"`: first open asks, second doesn't; after lock and unlock, close and reopen, or switching databases, the next asks again; with `"preview"` every open asks | `tests/open_document_test.rs` |
| US3-5, US3-6, FR-013: following the setting | The name opens per setting; "Preview" and "Open in another app…" always available; the list marks what can't be previewed and why | `DocumentList.test.tsx` |
| SC-001: speed | At 10,000 firearms and 10,000 accessories: 10 MB TIFF page 1 within 1 s, a 10 MB PDF served to the surface, 10 MB text decoded within budget; time to `preview:pdf-ready` for a 10 MB PDF within 1 s; the 10 MB PDF's first paint per OS; progress shown before `open_preview` returns, and the viewer closable meanwhile | `tests/performance_test.rs` (+ TIFF, PDF, text); `us13` E2E (PDF ready); the surface check (first paint); `DocumentPreview.test.tsx` (progress, closing); `tests/tiff_preview_test.rs` (closing during a large `Load`) |
| CSP unchanged | `frame-src`, `worker-src` and `script-src` still forbid frames, workers, wasm and eval | `tests/csp_test.rs` (+ the assertion) |
| Seed in step | The seed has a PDF (3 pages, with text), a password-protected PDF, a multi-page TIFF (G4), a text, a CSV, a DOCX and an ODS, an accessory's PDF, and a raw pre-007 JPEG row; the coverage test treats `document_names_fts` as an index | `examples/human_seed.rs`; `tests/human_seed_coverage_test.rs` |
| E2E | Keyboard-only: open a PDF from the record, wait for ready, F6 into it and back, → to the next document (a TIFF: the surface closed), PageDown and + in the TIFF, Escape; "Open in another app…" with `HOPLODEX_E2E_CONSENT=cancel` (nothing written; the consent log has the name) and `=open` (copy written and named in `HOPLODEX_E2E_OPENED_LOG`); the setting change; lock with a PDF shown | `e2e/specs/us13-document-preview.e2e.ts` *(new)* |
| Release seams | `HOPLODEX_E2E_CONSENT` and `HOPLODEX_E2E_PDF_PREVIEW` are absent from a release binary and present in an E2E one | `scripts/check-no-webdriver.mjs` (in `npm run audit`, and on the AppImage binary) |

## Manual checks (best effort before a release, not merge gates)

These need a real desktop, real default apps or a screen reader, which
the container doesn't have. Each is done on Windows, macOS and Linux
unless it says otherwise.

### Setup (every check starts here)

`scripts/human-testing.sh` seeds **two** databases into
`.human-testing/HoploDex/`: "Main collection" and "Shared collection".
Every check uses **"Main collection"**. "Shared collection" has three
firearms and no documents, and asks to take over from "Workshop PC"
before it opens.

1. Start the app against the sandbox. On Linux, run
   `scripts/dev-container.sh --gui scripts/human-testing.sh`. On macOS,
   run `scripts/human-testing.sh` in the macOS 26 VM. (The script is for
   Linux and macOS only. Human testing on Windows is #81.)
2. **Expected**: the database chooser lists "Main collection" and
   "Shared collection" under recent databases.
3. Choose "Main collection" and enter the passphrase
   `human testing passphrase` (the script prints it too).
4. Open the firearm "Glock 19 Gen5". Every document the checks use is in
   its Documents section.

The `opened-documents` folder the checks look in is under the
sandbox's cache folder:
`.human-testing/cache/io.github.exodious.HoploDex/opened-documents` on
Linux, and
`.human-testing/home/Library/Caches/io.github.exodious.HoploDex/opened-documents`
on macOS.

### M1. The native confirmation is the system's and blocks the window

1. Do the Setup steps 1–4.
2. In Documents, choose "Open in another app…" on "Purchase receipt.pdf".
3. **Expected**: a system dialog titled "Open “Purchase receipt.pdf” in
   another app?", with the four consequences and the buttons "Open in
   another app" and "Cancel". The HoploDex window doesn't respond to
   clicks while it is shown.
4. Press Escape.
5. **Expected**: the dialog closes, no app starts, and the
   `opened-documents` folder (Setup) has no `Purchase receipt.pdf`.
6. Open the database menu, then Database settings, and in its Documents
   section change "Open documents" to "Open in another app".
7. **Expected**: a system dialog titled "Open documents in another
   app?" with the buttons "Open in another app" and "Cancel", and the
   HoploDex window doesn't respond to clicks while it is shown. Press
   Escape: the dialog closes and the setting still reads "Preview in
   HoploDex".

### M2. No app for the type

1. On a computer with no app for `.ods`, do the Setup steps 1–4.
2. In Documents, choose "Open in another app…" on "Range log.ods".
3. Choose "Open in another app" in the system dialog. (On Windows there is
   no dialog: the check comes first, amended 2026-10-07.)
4. **Expected**: the toast "This computer has no app that opens
   OpenDocument spreadsheet documents." (On Linux and macOS, which find
   out only when they start the app, it continues "HoploDex deleted the
   copy it made.") The `opened-documents` folder (Setup) has no
   `Range log.ods`.

### M3. The copy is marked untrusted (Windows, macOS)

1. On Windows, with "Main collection" open on "Glock 19 Gen5" (Setup
   steps 3–4), choose "Open in another app…" on "Bill of sale.docx".
2. **Expected**: Word opens it in Protected View.
3. On macOS, do the Setup steps 1–4, choose "Open in another app…" on
   "Bill of sale.docx", and choose "Open in another app" in the system
   dialog.
4. Run `xattr -l` on `Bill of sale.docx` in the `opened-documents` folder
   (Setup).
5. **Expected**: a `com.apple.quarantine` attribute is listed.

### M4. A screen reader reads a previewed PDF

_Deferred 2026-10-08 to #48, which runs the screen reader checks of 004, 005, 006 and 007 together (tasks.md T129–T131)._

1. With Orca (Linux), NVDA (Windows) or VoiceOver (macOS) running, do
   the Setup steps 1–4 and preview "Purchase receipt.pdf".
2. Move to the page area.
3. **Expected**: the region is announced as "Purchase receipt.pdf, PDF",
   with the F6 hint.
4. Press F6.
5. **Expected**: the screen reader reads the page's text.
6. Press F6 again.
7. **Expected**: focus is back on the viewer's first control, and the
   screen reader announces it.

### M5. A password-protected PDF uses the viewer's own prompt

1. Do the Setup steps 1–4 and preview "Appraisal (protected).pdf".
2. **Expected**: the computer's PDF viewer asks for the document's
   password inside the page area. HoploDex shows no dialog of its own.
3. Type the password `hoplodex-test` and confirm.
4. **Expected**: the document is shown.
5. Close the viewer and preview the same document again.
6. **Expected**: the password is asked for again.

### M6. Spotlight doesn't index anything from a preview (macOS, indexing on)

1. On a Mac with Spotlight indexing on for the start-up volume, do the
   Setup steps 1–4 and preview "Purchase receipt.pdf".
2. Close the viewer and quit HoploDex.
3. Search Spotlight for `waiting period`, a phrase from that document's
   second page.
4. **Expected**: no result outside the repository checkout
   (`src-tauri/examples/human_seed.rs`, which writes the document, holds
   the phrase too).
