# Quickstart: Validate Document Preview and Consent

This is a runnable validation guide, not an implementation spec. It maps
each user story's independent test and each success criterion in
[spec.md](./spec.md) to the automated tests that cover it. Names refer to
[data-model.md](./data-model.md) and [contracts/](./contracts). Setup is the
same as [001's quickstart](../001-firearms-inventory/quickstart.md).

## Prerequisites

- The development container (`scripts/dev-container.sh`; see
  DEVELOPMENT.md), rebuilt with `--build` once, because its `Dockerfile`
  now fetches PDFium. On the host instead, run `scripts/fetch-pdfium.sh`
  and export `HOPLODEX_PDFIUM_DIR=src-tauri/pdfium/<target>`.
- A **fresh database**: `0002_fts5.sql` was edited in place. Nothing here
  opens the real databases. Every test, E2E run and screenshot uses
  throwaway locations (DEVELOPMENT.md, "Test isolation").

## Automated test commands

Run these through the container wrapper on a host with podman (CLAUDE.md):

```bash
scripts/dev-container.sh cargo test --manifest-path src-tauri/Cargo.toml
scripts/dev-container.sh npm test
scripts/dev-container.sh bash -c 'cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets \
  && cargo fmt --manifest-path src-tauri/Cargo.toml --check && npm run lint && npm run format:check'
scripts/dev-container.sh npm run audit
scripts/dev-container.sh bash -c 'npm run build && npm run test:e2e -- --spec e2e/specs/us13-document-preview.e2e.ts'
scripts/dev-container.sh bash -c 'npm run build && npm run screenshots'
```

One file at a time while working:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --test preview_test
npx vitest run src/features/media/DocumentPreview.test.tsx
```

All of these MUST pass before merge (constitution, Development Workflow).
Write each test to fail first (red-green, constitution II).

## Scenario map: story → tests

New backend test files are marked *(new)*. Backend tests run the real
`ops` against a temporary SQLCipher database, and the real helper (the
built `hoplodex` binary, `CARGO_BIN_EXE_hoplodex`) with the fetched PDFium.

| Story / criterion | What is checked | Where it is automated |
|---|---|---|
| US1-1, FR-005: PDF preview | 3-page PDF: `open_preview` gives 3 page sizes; page 1 PNG decodes to the requested width; page text matches the fixture's text in order | `tests/preview_test.rs` *(new)*; `DocumentPreview.test.tsx` *(new)* (page indicator "Page 1 of 3", toolbar, name and date) |
| US1-2, US1-3: TIFF | Single-page TIFF: fit, zoom, actual size; 4-page TIFF (LZW, Deflate, CCITT G4 and JPEG pages): 4 sizes, each page renders; one undecodable page fails alone | `tests/preview_test.rs`; `DocumentPreview.test.tsx` (single-page toolbar) |
| US1-4: text and CSV | UTF-8, UTF-8 BOM, UTF-16 LE/BE, Windows-1252 decode; control characters replaced; `<b>`, `=SUM(A1)` and a URL shown literally; CSV not laid out | `tests/preview_text_test.rs` *(new)*; `DocumentPreview.test.tsx` (the text is a text node in a `<pre>`, no `<a>`, no `<table>`) |
| US1-5, FR-003, SC-002: nothing on disk | One document of each previewable type with a unique marker; the isolated XDG and temp folders and the system temp folder scanned during and after: no marker | `tests/preview_no_disk_test.rs` *(new)*; `e2e/specs/us13-document-preview.e2e.ts` *(new)* (scan around a real preview) |
| US1-6, FR-007: previous and next | Only the record's own documents, in list order; an accessory's documents never reached from its host firearm; the photo viewer's arrow behavior | `DocumentPreview.test.tsx`; `e2e/specs/us13-document-preview.e2e.ts` (firearm with a mounted accessory with documents) |
| US1-7: close | Escape and the close control; focus returns to the opener | `DocumentPreview.test.tsx` |
| US1-8, FR-014, SC-006: lock and close | Helper process gone and `preview` dropped after lock, close, switch, sleep and shutdown; viewer unmounted before the chooser; `blob:` URLs revoked | `tests/preview_test.rs` (`OpenDatabase` drop kills the helper; child PID no longer exists); `tests/lock_test.rs` (+ preview open at each lock cause); `SessionProvider.test.tsx` (+ viewer gone before chooser); `DocumentPreview.test.tsx` (`URL.revokeObjectURL` on unmount) |
| US1-9, FR-015, SC-007: search by document name | "appraisal" finds the firearm; gone after the document is deleted; the Accessories page finds the accessory by "receipt", the collection doesn't find its host; 1–2 character `LIKE` path; firearm deletion cascades the index | `tests/fts_search_test.rs`, `tests/list_accessories_test.rs` (+ document names); `tests/deletion_wipe_test.rs` (+ a unique document filename absent from the raw file after `delete_document` and after deleting the record) |
| US2-1, US2-2, FR-008: native confirmation | With the fake `Consent`: asked with this document's stored name; Cancel → `{ opened: false }`, no folder, no file, opener not called | `tests/open_document_test.rs` *(new)* |
| US2-3, FR-010: confirmed open | Copy written under the canonical extension, `0600` in `0700` folders (Unix), Zone mark (Windows) or quarantine (macOS); opener called once; cleared on close and at startup | `tests/open_document_test.rs` (fake opener records the path); `tests/document_test.rs` (existing FR-035 clean-up assertions still hold) |
| US2-4: unpreviewable type | Word document: `PREVIEW_UNSUPPORTED`; the viewer shows the state with "Open in another app…" | `tests/preview_test.rs`; `DocumentPreview.test.tsx` |
| US2-5: no app | Opener error → `NO_APP_FOR_DOCUMENT`, the copy gone | `tests/open_document_test.rs` (fake opener fails); manual check M2 for each OS |
| US2-6, FR-016, SC-008: attach refusals | `.exe` (PE header), `.bat`, `.ps1`, `.lnk`, `.html`, `.svg`, `.jpg`, `.png`, `.gif`, `.heic`, `.docm`, `.docx` with `vbaProject.bin`, `.doc` with `Macros` storage, `.pdf` holding HTML, `.txt` starting with `<!DOCTYPE html>`: each refused with the right code; nothing stored; recorded type is canonical for every accepted type | `tests/document_types_test.rs` *(new)*; `tests/document_test.rs` (+ both attach paths); `DocumentList.test.tsx` (picker `accept`, drop refusal toast, photo message); `filePaths.test.ts` (document routing from `list_document_types`) |
| US2-7, FR-017: pre-007 rows | A raw-SQL row of type `image/jpeg` and one `.pdf` holding HTML: `open_document` refuses before asking (the fake `Consent` is never called); `openable` false; still deletable | `tests/open_document_test.rs`; `DocumentList.test.tsx` (disabled button with reason) |
| US3-1, FR-011: default | Fresh `machine.json` and one without the field read `"preview"` | `tests/machine_settings_test.rs` (+ `documentOpening`) |
| US3-2, FR-012: changing the setting | `"external"` asks through `Consent`; Cancel keeps `"preview"`; `"preview"` never asks; the change doesn't set the session flag | `tests/open_document_test.rs`; `DatabaseSettingsDialog.test.tsx` (Documents fieldset, pending state, saved at once) |
| US3-3, US3-4: once per session | With `"external"`: first open asks, second doesn't; after lock and unlock, close and reopen, or switching databases, the next asks again; with `"preview"` every open asks | `tests/open_document_test.rs` (`needs_consent` and the flag across `OpenDatabase` lifetimes) |
| US3-5, US3-6, FR-013: following the setting | Name opens per setting; "Preview" and "Open in another app…" always available; setting back to preview | `DocumentList.test.tsx` |
| SC-001: speed | 10 MB PDF and 10 MB TIFF page 1 within 1 s; 10 MB text decoded within budget; progress shown before page 1 for a large PDF | `tests/performance_test.rs` (+ preview); `DocumentPreview.test.tsx` (progress state) |
| SC-003, FR-004: hostile documents | The generated corpus (research.md §19): each shown inertly or reported; no connection to the loopback canary; no read of the canary file; the test process and the session survive a helper crash, time-out and memory limit | `tests/preview_hostile_test.rs` *(new)* |
| Helper confinement | The helper can't open a file or a socket after loading (Linux Landlock in the container); it dies when its parent does; no core dump | `tests/preview_helper_test.rs` *(new)* |
| CSP unchanged | `frame-src`, `worker-src` and `script-src` still forbid frames, workers, wasm and eval | `tests/csp_test.rs` (+ the preview assertion) |
| Seed in step | The seed has a PDF, a TIFF, a text, a CSV and a Word document, an accessory document, and a raw pre-007 JPEG row; the coverage test treats `document_names_fts` as an index | `examples/human_seed.rs`; `tests/human_seed_coverage_test.rs` |
| E2E | Keyboard-only: open a PDF from the record, page with PageDown, zoom with +, move to the next document with →, Escape; "Open in another app…" with `HOPLODEX_E2E_CONSENT=cancel` (no file written, the consent log has the name) and `=open` (copy written, and named in `HOPLODEX_E2E_OPENED_LOG`, where an E2E build logs what it would have handed to the OS); the setting change; lock with the viewer open | `e2e/specs/us13-document-preview.e2e.ts` *(new)* |
| Release seam | The `HOPLODEX_E2E_CONSENT` marker is absent from a release binary and present in an E2E one | `scripts/check-no-webdriver.mjs` (in `npm run audit`, and on the AppImage binary) |

## Manual checks (best effort before a release, not merge gates)

These need a real desktop and real default apps, which the container
doesn't have. Each is done on Windows, macOS and Linux.

### M1. The native confirmation is the system's and blocks the window

1. Start the app with `scripts/human-testing.sh` (a throwaway database).
2. Open the seeded firearm "Glock 19 Gen5".
3. In Documents, choose "Open in another app…" on "Purchase receipt.pdf".
4. **Expected**: a system dialog titled "Open “Purchase receipt.pdf” in
   another app?", with the four consequences and the buttons "Open in
   another app" and "Cancel". The HoploDex window doesn't respond to
   clicks while it is shown.
5. Press Escape.
6. **Expected**: the dialog closes, no app starts, and the
   `opened-documents` folder under the app's cache folder has no
   `Purchase receipt.pdf`.

### M2. No app for the type

1. On a computer with no app for `.ods`, start the app as in M1.
2. Choose "Open in another app…" on the seeded "Range log.ods".
3. Choose "Open in another app" in the system dialog.
4. **Expected**: the toast "This computer has no app that opens
   OpenDocument spreadsheet documents. HoploDex deleted the copy it made."
   The `opened-documents` folder has no `Range log.ods`.

### M3. The copy is marked untrusted

1. On Windows, open the seeded "Bill of sale.docx" in another app and
   confirm.
2. **Expected**: Word opens it in Protected View.
3. On macOS, run `xattr -l` on the copy in the `opened-documents` folder.
4. **Expected**: a `com.apple.quarantine` attribute is listed.

### M4. Screen reader reads a PDF page but can't select it

1. With Orca (Linux), NVDA (Windows) or VoiceOver (macOS) running, preview
   "Purchase receipt.pdf".
2. Move the reading cursor into the page area.
3. **Expected**: the page's text is read, introduced as "Page 1 of 3".
4. Press Ctrl/⌘+A, then Ctrl/⌘+C, and paste into a text editor.
5. **Expected**: nothing from the document is pasted.
