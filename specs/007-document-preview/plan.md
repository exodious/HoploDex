# Implementation Plan: Document Preview and Consent Before Opening Externally

**Branch**: `007-document-preview` | **Date**: 2026-10-03 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/007-document-preview/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

Attached documents are **previewed inside HoploDex** by default:
- PDF and TIFF with page and zoom controls;
- text and CSV as plain text.

**Opening one in another program** takes a confirmation that states the
consequences. A per-computer setting can make "Open in another app" the
default, asking once per database session. Each page's search also
matches its own records' **document names**.

**Issue #13's security findings** changed the design:
- **[High]** any attachment reached `ShellExecuteExW`, so a program would
  run;
- **[Medium]** HTML and SVG loaded network content in the external viewer;
- both note that a React-only confirmation can be bypassed by a
  compromised web view.

On 2026-10-03 the owner decided:
- the confirmation is the **operating system's dialog**, shown by Rust;
- documents take an **allowlist of document types confirmed by content**:
  PDF, TIFF, TXT, CSV, RTF, DOC/DOCX, XLS/XLSX, ODT/ODS. Photos (JPEG,
  PNG) go under Photos only.

spec.md was amended accordingly (FR-008, FR-009, FR-012, FR-016, FR-017,
SC-008).

**Technical approach**:
- **Trust boundary** (research.md §1): nothing that parses a document runs
  in the web view, which holds the IPC bridge. It receives only PNGs that
  HoploDex encoded, and plain strings.
- **One type check** (§2): `services::document_types::classify` runs at
  attach, at preview and at external open. The recorded type is the one
  it finds.
- **PDF** (§3): PDFium (plain build, no V8 or XFA) through
  `pdfium-render`.
- **TIFF** (§7): the pure-Rust `tiff` and `fax` crates.
- **Isolation** (§4, §5): both engines run in a **render helper**, the
  HoploDex executable restarted with a hidden argument. It talks over
  pipes and holds no key, database or IPC. It is confined per OS (no
  files, no network, memory and time limits, no core dumps) and killed
  with the session.
- **Text** (§8): decoded in Rust and shown as a React text node.
- **Native consent** (§12): a `Consent` trait implemented with
  `tauri-plugin-dialog`'s Rust API. The session's "already confirmed"
  flag lives in `OpenDatabase`.
- **The setting** (§13): in `machine.json`.
- **External copies** (§14): the canonical extension, owner-only
  permissions, and a Zone or quarantine mark.
- **Search** (§15): a third trigram index, `document_names_fts`.

## Technical Context

**Language/Version**: Rust 1.97+ (edition 2024, `src-tauri`), TypeScript 5.x /
React 18 (`src`), unchanged

**Primary Dependencies**:
- **New**:
  - `pdfium-render` 0.9 with a pinned, checksummed PDFium shared library
    from pdfium-binaries, shipped as a bundle resource and loaded only by
    the helper;
  - `tiff`, `fax`;
  - `landlock` (Linux).
- **Promoted from transitive, at locked versions**: `zip`, `cfb`,
  `encoding_rs`, `png`.
- **Reused**: `tauri-plugin-dialog` (its Rust message API),
  `tauri-plugin-opener`, `zeroize`, `windows-sys` (plus the job-object and
  threading features), `libc`.
- **Frontend**: no new npm package. It reuses `Dialog` (gaining an `xl`
  size), `Button`, `Menu`, `SegmentedControl` and `useToast`.

**Storage**: The existing encrypted SQLCipher database. `0002_fts5.sql` is
edited in place to add `document_names_fts` and three triggers. No column
changes, and an existing development database must be recreated.
`machine.json` gains `documentOpening` (defaulted, so no version bump).

**Testing**:
- **cargo**: integration tests against a real temporary SQLCipher database
  and the real helper binary, with the fetched PDFium. They include:
  - a generated hostile corpus (research.md §19);
  - a marker scan of every isolated folder for SC-002;
  - confinement checks in the dev container;
  - a fake `Consent` and a fake opener.
- **Vitest + React Testing Library**: the viewer, the list, the settings
  fieldset and the session unmount.
- **WebdriverIO**: one spec with real keyboard input and an E2E-only
  consent seam, plus the screenshots walk.

**Target Platform**: Desktop: Windows 10+, macOS 26+ (raised from 12+ on
2026-10-06: the owner tests only macOS 26), Linux.
PDFium targets: linux-x64, linux-arm64, mac-univ, win-x64, win-arm64.

**Project Type**: Desktop application (Tauri: React frontend + Rust backend in
one repo)

**Performance Goals**:
- PDF or TIFF page 1 within 1 s at 10 MB, and progress within 1 s for
  larger files (SC-001);
- search with document names within 500 ms at 10,000 firearms + 10,000
  accessories (SC-007);
- no UI-thread decoding.

The table is in research.md §18.

**Constraints**:
- Fully offline.
- No document bytes to disk while previewing (FR-003), including
  helper core dumps.
- Nothing in a document runs or fetches (FR-004).
- The CSP is unchanged (no frames, workers, wasm or eval).
- Cipher settings are untouched.
- No external open without a "yes" in the native dialog, except as FR-012
  allows within a session.

**Scale/Scope**: Three user stories (P1–P3), 17 functional requirements
and 8 success criteria.
- **Interface**: 7 new commands and 6 amended; 9 new error codes.
- **Schema**: 1 new FTS table.
- **Process**: 1 new helper process mode.
- **Settings**: 1 new machine setting.
- **UI**: 1 new viewer component and 1 new settings fieldset.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Requirement | How this plan satisfies it |
|---|---|---|
| I. Code Quality | Lint/static analysis, review, small single-purpose modules, no speculative abstraction | **One rule, one place**: `document_types::classify` for every type decision (§2), `needs_consent` for the session rule (§13), `services::consent` for the dialog text. **Isolated engine**: the helper's protocol is a small module (`services::preview::helper`), so the engine choice (§3) is replaceable without touching commands. **Refactor, not workaround**: `add_document` loses its `mimeType` argument and every caller is updated, rather than adding a variant. **Speculative options rejected**: a wasm sandbox (§4), seccomp (§5), viewer search. `clippy`, `rustfmt`, `eslint` and `prettier` run locally. CI stays disabled by the owner's choice |
| II. Testing (NON-NEGOTIABLE) | Tests first, real persistence, one test per acceptance scenario, test isolation | Every acceptance scenario and success criterion maps to a named test (quickstart.md). Tests run the real `ops` and the real helper binary against temporary databases and isolated folders. SC-002 is a marker scan, and SC-003 and SC-008 use a generated corpus. The native dialog has a fake in Rust tests and an E2E-only seam, checked absent from release builds. Four manual checks are numbered procedures, not merge gates |
| III. UX Consistency | One component set and pattern, WCAG 2.1 AA | The viewer reuses `PhotoViewer`'s layout, footer, ← / → and Escape behavior, and the delete `ConfirmDialog`. The list's actions are the existing ghost buttons. The setting is a `SegmentedControl` in Database settings. **One deliberate deviation**: the external-open confirmation is the OS dialog, for security (Complexity Tracking). Keyboard control, hidden page text for screen readers and the copy block are specified in contracts/ui-document-preview.md §7–§8 |
| IV. Performance | 100 ms feedback / 1 s completion, 500 ms search, no UI-thread blocking | Progress shows at once; page 1 within 1 s at 10 MB. Only visible pages render, capped in size, with at most 8 kept. Rendering happens off the session lock and off the UI thread. Search adds one indexed subquery. `performance_test.rs` holds each budget (research.md §18) |
| V. User Privacy | Local only, real deletion, no hidden copies | **No disk**: previews write nothing to disk; the helper's core dumps are disabled. **Real deletion**: deleting a document now merges the search indexes, so its name leaves no FTS segment (`deletion_wipe_test.rs`). **The external copy**: it stays under 001 FR-035's clean-up, gains owner-only permissions, and the confirmation states it plainly, including the new network consequence. **Nothing leaves the device**: the helper cannot reach the network |
| Security & Data Handling | Encryption at rest, vetted dependencies, no cipher change | The trust boundary (research.md §1) answers both #13 findings: native consent, an allowlist by content, and no parser in the web view. No key or cipher change. **Vetted**: PDFium is the plain build (no V8 or XFA), checksummed and pinned, with advisories checked by hand since `cargo deny` can't see it (§6). None of the new crates collects data. Persistence PR notes come from research.md §1, §2, §14 and §15 |
| Licensing | GPLv3-compatible dependencies and bundled assets with recorded source | **Crates**: `pdfium-render`, `tiff`, `fax` and `landlock` are MIT and/or Apache-2.0. **PDFium**: BSD-3-Clause, with its bundled FreeType (FTL), libjpeg-turbo, OpenJPEG, Little CMS, zlib, libpng and abseil recorded with source and version in `docs/third-party/pdfium.md` before it is added. Its license file ships beside it. The list is checked by hand against the pinned build (§6) |

**Result**: PASS, with two justified entries in Complexity Tracking.

## Project Structure

### Documentation (this feature)

```text
specs/007-document-preview/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   ├── tauri-commands.md        # document types; attach and open amended; preview commands;
│   │                            #  setting commands; search; error codes
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
├── fetch-pdfium.sh                  # NEW: pinned download + SHA-256 check → src-tauri/pdfium/<target>/
├── pdfium.lock                      # NEW: release tag and per-target checksums
├── check-no-webdriver.sh            # + HOPLODEX_E2E_CONSENT marker, both ways round
└── build-appimage.sh                # runs fetch-pdfium.sh; bundles the library
Dockerfile                           # fetches PDFium; sets HOPLODEX_PDFIUM_DIR
docs/third-party/pdfium.md           # NEW: PDFium and its bundled code, sources, versions, licenses
DEVELOPMENT.md                       # PDFium fetch; HOPLODEX_PDFIUM_DIR; manual advisory check; consent seam

src-tauri/
├── Cargo.toml                       # pdfium-render, tiff, fax, landlock (linux); zip, cfb, encoding_rs, png
│                                    #  promoted; windows-sys job-object/threading features
├── tauri.conf.json                  # bundle.resources: the PDFium library and its LICENSE (CSP unchanged)
├── .gitignore                       # pdfium/
├── src/
│   ├── main.rs                      # `--render-helper` checked first; register 7 commands; Consent and
│   │                                #  opener state
│   ├── db/
│   │   ├── migrations/0002_fts5.sql # document_names_fts + 3 triggers
│   │   └── mod.rs                   # reclaim_deleted_record merges document_names_fts too
│   ├── models/
│   │   └── document_attachment.rs   # DocumentSummary +previewKind, +openable
│   ├── services/
│   │   ├── document_types.rs        # NEW: the allowlist, classify(), signatures, canonical MIME/extension
│   │   ├── consent.rs               # NEW: Consent trait, ConsentRequest text (FR-009), dialog impl,
│   │   │                            #  E2E seam (cfg feature = "e2e"), needs_consent()
│   │   ├── preview/                 # NEW
│   │   │   ├── mod.rs               # Preview, HelperHandle (spawn, frames, restart, Drop kills)
│   │   │   ├── protocol.rs          # frame encoding shared by parent and helper
│   │   │   ├── helper.rs            # helper entry: confine, load PDFium, serve Load/Render/Text
│   │   │   ├── confine.rs           # per-OS confinement (Landlock, sandbox_init, job object, rlimits)
│   │   │   ├── pdf.rs               # PDFium: sizes, render → PNG, text, password/damaged mapping
│   │   │   ├── tiff.rs              # IFD walk, page decode (tiff + fax), downsample → PNG
│   │   │   └── text.rs              # BOM/UTF-8/UTF-16/Windows-1252 decode, control characters
│   │   ├── attachments.rs           # mime_type_for removed in favor of document_types (photos keep theirs)
│   │   ├── machine_settings.rs      # documentOpening, get/set
│   │   └── mod.rs
│   ├── commands/
│   │   ├── documents.rs             # classify at attach; open_document with consent, generation re-check,
│   │   │                            #  hardened copy, NO_APP_FOR_DOCUMENT; list_document_types;
│   │   │                            #  delete reclaims indexes
│   │   ├── preview.rs               # NEW: open_preview, render_preview_page, get_preview_page_text,
│   │   │                            #  close_preview + ops
│   │   ├── databases.rs             # get_document_opening, set_document_opening
│   │   ├── firearms.rs              # list_firearms search joins document names
│   │   ├── accessories.rs           # list_accessories search joins document names
│   │   └── mod.rs
│   └── session/
│       └── mod.rs                   # OpenDatabase +preview, +external_open_confirmed; generation
├── examples/human_seed.rs           # PDF (3 pages, with text), TIFF (multi-page, G4), TXT, CSV, DOCX, ODS,
│                                    #  an accessory's PDF, a password-protected PDF, a raw pre-007 JPEG row
└── tests/
    ├── document_types_test.rs       # NEW: FR-016 table, every refusal, canonical types
    ├── preview_test.rs              # NEW: US1, FR-001, FR-005–FR-007, FR-014 (helper killed with the session)
    ├── preview_text_test.rs         # NEW: §8 decoding
    ├── preview_no_disk_test.rs      # NEW: SC-002 marker scan
    ├── preview_hostile_test.rs      # NEW: SC-003 corpus
    ├── preview_helper_test.rs       # NEW: confinement, parent death, limits, no core dump
    ├── open_document_test.rs        # NEW: US2, US3, FR-008–FR-013, FR-017 with fake Consent and opener
    ├── support/hostile_documents.rs # NEW: corpus generators
    ├── fixtures/documents/          # NEW: small PDF/TIFF/DOCX/ODS fixtures with their source note
    ├── document_test.rs             # attach paths refuse; mimeType from content
    ├── fts_search_test.rs, list_accessories_test.rs   # + document names, not through mounts
    ├── deletion_wipe_test.rs        # + filename gone after delete_document and record delete
    ├── lock_test.rs                 # + preview open at each lock cause
    ├── machine_settings_test.rs     # + documentOpening default and round trip
    ├── csp_test.rs                  # + preview assertions
    ├── performance_test.rs          # + research.md §18
    └── human_seed_coverage_test.rs  # document_names_fts is an index

src/
├── components/Dialog.tsx            # + size "xl"
├── features/
│   ├── media/
│   │   ├── DocumentList.tsx         # name follows setting; Preview; Open in another app…; meta; refusals;
│   │   │                            #  accept from list_document_types; hosts DocumentPreview
│   │   ├── DocumentPreview.tsx      # NEW: viewer, toolbar, pages, text, states, keyboard, a11y
│   │   ├── usePreviewPages.ts       # NEW: visible-page rendering, zoom debounce, 8-bitmap cache, blob URLs
│   │   ├── mediaService.ts          # addDocument without mimeType; openDocument → {opened}; preview,
│   │   │                            #  document-type and setting wrappers
│   │   ├── filePaths.ts             # isDocumentPath from the document types
│   │   ├── types.ts                 # previewKind, openable, PreviewInfo, DocumentType, DocumentOpening
│   │   ├── media.css                # viewer and page styles
│   │   └── *.test.tsx
│   ├── databases/
│   │   └── DatabaseSettingsDialog.tsx  # "Documents" fieldset
│   └── session/SessionProvider.test.tsx  # viewer gone before the chooser on lock

e2e/
├── specs/us13-document-preview.e2e.ts   # NEW
├── wdio.conf.ts                         # HOPLODEX_E2E_CONSENT and its log in the scratch dir
└── screenshots/screens.e2e.ts           # + contracts/ui-document-preview.md §10
```

**Structure Decision**: No new project or layer.
- **Rust** owns:
  - every security decision: types, consent, the session flag, copies;
  - the engines, which run in the helper;
  - text decoding and search.
- **React** owns the viewer's layout, controls and accessibility.
- **The preview** lives in `features/media/` beside `PhotoViewer`, which
  it mirrors.
- **The E2E spec** is numbered `us13`, continuing the series.

### Close-out (at pull request time)

The final phase of tasks.md updates the related issues as part of opening
the pull request:

- **#13** (this feature): link the pull request (it closes the issue on
  merge). Reply to each security finding with how it is addressed:
  - native consent (FR-008, research.md §12);
  - the allowlist by content at attach and at open (FR-016, FR-017, §2);
  - canonical extensions, owner-only permissions and Zone/quarantine
    marks (§14);
  - no parser in the web view (§1);
  - the network consequence in the dialog (FR-009 (c)).

  Note the residual risk accepted for the once-per-session choice.
- **#21** (release security review): comment that the review must cover:
  - the render helper and its confinement on each OS;
  - the pinned PDFium build and its advisories (not visible to
    `cargo deny`);
  - the `Consent` seam's absence from release builds;
  - `document_types::classify`;
  - the external-copy hardening.

  List the residual risks from research.md §1.
- **#73** (web view navigation): comment that 007 adds no navigable
  element (research.md §9), so it neither fixes nor widens #73.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|---|---|---|
| A new native library (PDFium) shipped per platform, with a fetch script and a hand-checked advisory process | FR-001's PDF preview, FR-002's same-on-every-OS rule, and FR-006's password and damage reporting need a mature PDF engine | pdf.js runs in the privileged web view and needs `'wasm-unsafe-eval'` (FR-004, finding 1). hayro is experimental, can't report password protection and has no text for FR-005. MuPDF is AGPL (research.md §3) |
| A second process mode (render helper) with per-OS confinement | FR-004: a crafted document must not reach the app's data or commands or bring it down. FR-003 rules out core dumps of content. The release build aborts on panic | An in-process engine shares the key-holding process and dies with it on a decoder panic. An iframe sandbox is unreliable in Tauri (GHSA-57fm-592m-34r7) (research.md §4) |
| The external-open confirmation is the OS dialog, not the app's `ConfirmDialog` (constitution III) | Issue #13 finding 1: a confirmation the web view draws can be answered by a compromised web view. The owner chose a native dialog on 2026-10-03 | An in-app dialog plus a backend `confirmed` flag is bypassable. Showing both would ask twice for one decision (research.md §12) |

## Post-Design Constitution Check

*Re-evaluated after Phase 1 (research.md, data-model.md, contracts/,
quickstart.md).*

- **Code Quality**: each rule lives in one place:
  - document types: `document_types::classify`;
  - the consent rule: `needs_consent`;
  - the consent text: `services::consent`;
  - the helper protocol: `preview::protocol`, shared by both ends.

  The frontend reads the types from `list_document_types` and holds no
  copy of the list. The engines sit behind the protocol, so replacing
  PDFium later (hayro, for example) changes `preview/pdf.rs` only. Still
  PASS.
- **Testing**: every acceptance scenario and success criterion has a
  named test (quickstart.md). SC-002, SC-003 and SC-008 have generated,
  reproducible inputs. The native dialog is covered by a fake and a
  release-checked E2E seam. The four manual checks are numbered
  procedures. Still PASS.
- **UX Consistency**: the viewer mirrors `PhotoViewer`, and the one new
  shared change is `Dialog`'s `xl` size, which the photo viewer may adopt.
  The OS dialog is the single recorded deviation. Still PASS.
- **Performance**: the budgets have explicit tests. Rendering is bounded
  by visibility and size caps, and search adds one indexed subquery.
  Still PASS.
- **User Privacy**: no new storage outside the encrypted database and
  `machine.json`, which holds only a choice. Deleted document names are
  merged out of the index. External copies are owner-only and marked, and
  the dialog discloses the network consequence. Still PASS.

**Result**: PASS. The design adds no violations beyond the three justified
above.

## Findings confirmed with the user

Confirmed by the owner on 2026-10-03, after reviewing issue #13's security
findings:

- **The confirmation is the operating system's dialog** (research.md
  §12), for both "Open in another app…" and changing the setting to "Open
  in another app". It is a recorded exception to constitution III.
- **Documents are document types only, confirmed by content** (research.md
  §2):
  - accepted: PDF, TIFF, TXT, CSV, RTF, DOC/DOCX, XLS/XLSX, ODT/ODS;
  - photos (JPEG, PNG) belong under Photos;
  - refused: other images, programs, scripts, installers, shortcuts, web
    pages, SVG and macro-enabled Office files.

  spec.md's FR-001, FR-016 and FR-017 say so.
- **TIFF is the one image type documents keep**, as the format of scanned
  paperwork. The preview covers PDF, TIFF, text and CSV.

Design choices made at planning that the spec left open:

- **The engine**: PDFium, plain build, in a confined helper process,
  rather than pdf.js in the web view (research.md §3–§5).
- **Fonts**: no system fonts reach the helper, so pages render the same
  on every OS (§5).
- **The setting's place**: a "Documents" fieldset in Database settings,
  saved at once, with a hint that it applies to every database on this
  computer (ui contract §5).
- **The session flag**: any confirmed external open sets it, whatever the
  setting at the time (§13).
- **The copy's name**: its stored name's stem with the type's canonical
  extension (§14).
- **Pre-007 rows** of other types: listed, deletable, never previewed or
  opened, with no migration (data-model.md).
