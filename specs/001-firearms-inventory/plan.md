# Implementation Plan: Firearms Collection Inventory

**Branch**: `001-firearms-inventory` | **Date**: 2026-07-22 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/001-firearms-inventory/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

A single-user, cross-platform (Windows/macOS/Linux) desktop application for
recording, browsing, valuing, and insuring a firearms collection, plus
photo/document attachments and spreadsheet export/import — all data stored
and encrypted locally with no cloud dependency. Built as a Tauri 2.x app: a
React + TypeScript frontend for UI, and a Rust backend owning all
persistence, encryption, business logic (value/insurance calculations,
import matching), and file-format I/O, communicating over Tauri's async
IPC command layer so no data-mutating or long-running operation blocks the
UI thread. Dollar amounts are stored as whole-dollar integers (FR-037);
insurance is scheduled per firearm under a policy, and every unscheduled
firearm is implicitly covered by the one blanket policy in force, computed
from policy dates and never assigned per firearm (FR-014, FR-036).

## Technical Context

**Language/Version**: Rust 1.75+ (backend, `src-tauri`), TypeScript 5.x /
React 18+ (frontend, `src`)

**Primary Dependencies**: Tauri 2.x (app shell, IPC, async commands, native
dialogs/file access); `rusqlite` with the `bundled-sqlcipher` and `fts5`
features (encrypted persistence + full-text search); `keyring` (OS-native
credential store for the SQLCipher passphrase); `rust_xlsxwriter` (Excel
export) and `calamine` (Excel import) plus `csv` (CSV export/import); an
accessible React component library built on Radix UI primitives (e.g.
shadcn/ui) for WCAG 2.1 AA-compliant, consistent UI; `tauri-plugin-dialog`
for file/folder pickers on export/import

**Storage**: A single encrypted SQLite (SQLCipher) database file on the
local filesystem, containing all structured data (firearms, insurance
policies, coverage) plus photographs and document attachments as BLOB
columns, plus an FTS5 virtual table indexing all searchable fields
(structured + free-form notes). No cloud storage, no external database
server.

**Testing**: `cargo test` for all Rust business logic (value-summary /
insurance-warning calculations, import matching and conflict resolution,
encryption/keyring integration) run against a real temporary SQLCipher
database per the constitution's no-mocks rule; Vitest + React Testing
Library for frontend unit tests; WebdriverIO driven through `tauri-driver`
(Tauri's officially supported WebDriver harness) for E2E tests covering the
five user-story acceptance scenarios end-to-end against the built app. Format,
lint, test, and build are defined as a CI workflow for Windows, macOS, and
Linux so FR-022's cross-platform requirement can be verified rather than
assumed. The workflow is intentionally kept disabled
(`.github/workflows-disabled/`) until the owner enables it; until then the
same checks are run locally, which is a documented, owner-approved deviation
from Constitution I's automated-gate requirement.

**Target Platform**: Desktop — Windows 10+, macOS 12+, Linux (glibc,
WebKitGTK) — single codebase, no server component, fully offline-capable.

**Project Type**: Desktop application (Tauri: React frontend + Rust
backend in one repo, not a client/server web app).

**Performance Goals**: Interactive actions (open/save a record, navigate
screens) give feedback within 100ms and complete within 1s; search/filter/
group across the full collection returns within 500ms; value-summary
recalculation after any mutation completes without a perceptible UI stall.
These hold at the constitution's stated scale (up to 10,000 firearm
records).

**Constraints**: No operation may block the UI thread — import, export,
and full-collection search/group MUST run as async Tauri commands (Rust
`async fn` on the Tokio runtime) with progress events streamed to the
frontend for bulk operations; the app MUST function fully with no network
connection; the encryption key MUST never be logged, written to disk in
plaintext, or transmitted; all photo/document BLOB reads for the tile/list
view MUST use a small cached thumbnail rather than the full-resolution
original to stay within the 500ms search/browse budget.

**Scale/Scope**: Single-user local collection, up to ~10,000 firearm
records per the constitution's performance baseline (spec's own tested
floor is 500 records per SC-002); 5 user stories (P1–P5); photos and
documents of typical consumer sizes (a few MB each).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Requirement | How this plan satisfies it |
|---|---|---|
| I. Code Quality | Linting/static analysis, peer review, small single-purpose modules, no speculative abstraction | `clippy` + `rustfmt` for Rust, `eslint`/`prettier` for TS wired into CI; Rust backend split into focused modules (`db`, `models`, `commands`, `services::{valuation, insurance, import_export}`) with no premature abstraction beyond what FR-001–FR-041 require |
| II. Testing (NON-NEGOTIABLE) | Tests before done, red-green, real persistence (no mocks), regression tests for bugs | `cargo test` integration tests run against a real temp SQLCipher DB (via `rusqlite`'s in-memory-file or tempdir DB, never a mock connection); one test per acceptance scenario in spec.md; Vitest for pure frontend logic; WebdriverIO/`tauri-driver` E2E for full user-story flows |
| III. UX Consistency | Single shared component library, consistent confirmation pattern, WCAG 2.1 AA | Single Radix-based component library (shadcn/ui) is the only source of buttons/dialogs/forms/tables; one shared `<ConfirmDialog>` component used for every destructive action (delete firearm, delete policy, bulk import overwrite); components chosen/audited for WCAG 2.1 AA |
| IV. Performance | 100ms feedback / 1s completion for interactive ops, 500ms search, no UI-thread blocking, progress indication for bulk ops | All DB access happens in Rust via async Tauri commands off the UI thread; FTS5 index keeps search sub-500ms at 10k-row scale; import/export run as async commands emitting `tauri::Emitter` progress events consumed by a shared progress-bar component |
| V. User Privacy | Local/encrypted storage, no unconsented transmission, no telemetry on collection contents, clear export disclosure, real deletion | SQLCipher encrypts the entire DB file at rest; `keyring` stores the passphrase in the OS credential store, never logged; no analytics/telemetry dependency is introduced; export dialog explicitly states the destination folder and that files leave the device unencrypted (T139–T140); the webview gets a restrictive CSP and no network-capable plugin is enabled (T141–T142); deleting a firearm, photo, or document issues a real `DELETE` rather than a soft-delete flag, with `PRAGMA secure_delete = ON` zeroing freed pages and a `VACUUM` after deletion returning the space, so deleted BLOB content does not linger in the file (T137–T138). _Amended by [spec 003](../003-database-protection-management/plan.md): the database is keyed by the user's passphrase, and the keyring holds only that passphrase, only on opt-in._ |
| Security & Data Handling | Encryption at rest, opt-in-only network features, vetted dependencies, no unauthorized external access | No network/sync feature exists in this feature at all (FR-021); all chosen dependencies (`rusqlite`, `keyring`, `rust_xlsxwriter`, `calamine`) are local-only, reviewed for absence of phone-home behavior in research.md; the DB file lives in the OS app-data directory, not a shared/exposed location. _Amended by [spec 003](../003-database-protection-management/plan.md): databases and their backups live in folders the user chooses._ |

**Result**: PASS — no violations requiring Complexity Tracking justification.

## Project Structure

### Documentation (this feature)

```text
specs/001-firearms-inventory/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

Tauri desktop app: one repository, one Rust backend crate (`src-tauri`)
owning persistence/business logic/IPC commands, one React+TypeScript
frontend (`src`) owning presentation only. Neither side has its own HTTP
API — the "contract" between them is the set of Tauri IPC commands
documented in `contracts/`.

```text
src-tauri/
├── src/
│   ├── main.rs               # Tauri app bootstrap, plugin/command registration
│   ├── db/
│   │   ├── mod.rs            # SQLCipher connection pool, key unlock, migrations
│   │   └── migrations/       # versioned SQL migrations (schema + FTS5 tables)
│   ├── models/                # Firearm, DispositionHistory, Photo, DocumentAttachment,
│   │                          # InsurancePolicy, InsuranceCoverage, GenericThumbnail
│   ├── commands/               # #[tauri::command] async handlers (thin IPC layer)
│   │   ├── firearms.rs
│   │   ├── photos.rs
│   │   ├── documents.rs
│   │   ├── insurance.rs
│   │   └── import_export.rs
│   └── services/                # pure business logic, unit/integration tested
│       ├── secure_delete.rs     # overwrite-then-unlink for decrypted temp copies (FR-035)
│       ├── thumbnails.rs        # generic-thumbnail fallback resolution (FR-009)
│       ├── valuation.rs         # value-summary computation (FR-015)
│       ├── insurance_status.rs  # under/uninsured + policy-expiry rules (FR-016/017/024/028)
│       ├── import_matching.rs   # make+model+serial matching & conflict resolution (FR-026/030)
│       └── spreadsheet.rs       # CSV/XLSX export (rust_xlsxwriter) + import (calamine)
├── tests/
│   ├── valuation_test.rs
│   ├── insurance_status_test.rs
│   ├── import_export_test.rs
│   └── encryption_test.rs       # keyring + SQLCipher unlock/lock integration (amended by spec 003: replaced by passphrase_protection_test.rs and keyring_test.rs)
├── Cargo.toml
└── tauri.conf.json

src/                              # React + TypeScript frontend
├── components/                   # shared, WCAG 2.1 AA component library (Radix-based)
├── features/
│   ├── firearms/                 # record form, detail view (US1)
│   ├── browse/                   # list/tile views, grouping, search (US2)
│   ├── insurance/                 # policy management, value summary, warnings (US3)
│   ├── media/                    # photo/document attach + thumbnail picker (US4)
│   └── import-export/            # export/import wizards, conflict resolution UI (US5)
├── services/                     # thin wrappers around Tauri `invoke()` calls
├── hooks/
└── main.tsx

src/**/*.test.tsx                 # Vitest unit tests, colocated with components

e2e/
├── wdio.conf.ts                  # WebdriverIO config using tauri-driver
└── specs/
    ├── us1-record-firearm.e2e.ts
    ├── us2-browse-search.e2e.ts
    ├── us3-value-insurance.e2e.ts
    ├── us4-photos-documents.e2e.ts
    └── us5-export-import.e2e.ts
```

**Structure Decision**: A single Tauri 2.x repository with two source
trees — `src-tauri` (Rust: all persistence, encryption, business rules,
IPC commands) and `src` (React/TypeScript: presentation only) — plus a
top-level `e2e/` WebdriverIO suite. This matches the "desktop-app" project
type: there is no separate network-facing backend, so the web-application
(client/server) template option does not apply; Tauri's IPC command
boundary plays the role that an HTTP API would play in that option, and is
documented as such in `contracts/`.

## Complexity Tracking

*No entries — Constitution Check above shows no violations requiring justification.*

## Post-Design Constitution Check

*Re-evaluated after Phase 1 (data-model.md, contracts/, quickstart.md).*

- **Code Quality**: schema and command surface stay in the small,
  single-purpose modules laid out in Project Structure; no new
  abstraction was introduced beyond what FR-001–FR-041 require. Still PASS.
- **Testing**: data-model.md's validation rules and contracts/'s command
  error cases give concrete, real-persistence test targets for
  `cargo test` (one per acceptance scenario, per quickstart.md); nothing
  here requires mocking. Still PASS.
- **UX Consistency**: contracts/tauri-commands.md standardizes a single
  `CommandError` shape and a single confirmation gate (`confirmed: true`)
  for every destructive command (delete firearm, delete policy, delete
  photo/document), so no screen invents its own pattern. Still PASS.
- **Performance**: `list_firearms` and `get_value_summary` are backed by
  the FTS5 index and indexed columns from data-model.md, and
  export/import commands emit progress events per contracts/ — no design
  element introduces a synchronous, UI-blocking path. Still PASS.
- **User Privacy**: no entity in data-model.md stores anything off-device;
  `export_collection` is the only path data leaves the encrypted DB, and
  it requires an explicit user-chosen destination folder (contracts/),
  matching the constitution's export-disclosure requirement. Still PASS.

**Result**: PASS — design artifacts introduce no new violations; no
Complexity Tracking entries needed.
