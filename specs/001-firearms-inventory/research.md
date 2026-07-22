# Phase 0 Research: Firearms Collection Inventory

All items from the user-provided Technical Context were concrete choices
rather than open unknowns; this document records the rationale for each and
resolves the handful of implementation-detail decisions the input left as
"either/or" (spreadsheet library placement, E2E test harness, UI component
library) so nothing is left as NEEDS CLARIFICATION going into Phase 1.

## 1. Application shell: Tauri 2.x

- **Decision**: Tauri 2.x, one repository, Rust backend + React/TypeScript frontend.
- **Rationale**: Single codebase produces native installers for Windows,
  macOS, and Linux (FR-022) without a server component, satisfies the
  local-only/offline constraint (FR-021, SC-008) by construction (no HTTP
  server, no bundled browser runtime shipping to a remote origin), and its
  IPC layer is async by default — commands return `Future`s executed on
  Tauri's Tokio runtime, which is what the constitution's "no UI-thread
  blocking" rule (Principle IV) requires for import/export/search.
- **Alternatives considered**: Electron (larger bundle, ships its own
  Chromium + Node, heavier memory footprint per Principle IV's budgets);
  a native-per-platform app (WinUI/AppKit/GTK) (would triple the UI
  implementation and testing surface, rejected by Principle I's
  YAGNI/complexity rule since a single Tauri codebase meets every
  requirement).

## 2. Persistence & encryption: rusqlite + bundled-sqlcipher

- **Decision**: `rusqlite` with the `bundled-sqlcipher` Cargo feature for
  the primary encrypted database file, combined with the `fts5` feature in
  the same build (both features can be enabled together — SQLCipher builds
  on the standard SQLite amalgamation and compiles in FTS5 when the feature
  flag is set).
- **Rationale**: SQLCipher gives full-database encryption at rest with a
  single passphrase-derived key (AES-256), which exceeds the constitution's
  "encrypt sensitive fields" floor (Security & Data Handling Constraints)
  by encrypting the entire container — simpler to reason about than
  per-column encryption and avoids leaking metadata (e.g., a firearm's make
  visible even with its serial number encrypted). A single encrypted `.db`
  file also directly satisfies the requirement to keep photos, documents,
  and structured data in "a single encrypted container" rather than a
  database plus a loose, separately-secured photo folder.
- **Alternatives considered**: Field-level encryption with a plain SQLite
  file (rejected — leaves filenames/photo BLOBs/metadata unencrypted unless
  every BLOB column is separately encrypted, adding complexity without
  benefit); an embedded document store (e.g., sled) (rejected — FTS5 and
  mature Excel/CSV import libraries assume a relational/SQL model, and the
  constitution's no-mocks testing rule is easiest to satisfy against a
  well-understood, widely-tested engine like SQLite).

## 3. Full-text search: SQLite FTS5

- **Decision**: One FTS5 virtual table (contentless or external-content,
  pointing at the `firearms` table) indexing make, model, caliber, type,
  serial number, notes, and accessories; kept in sync via SQLite triggers
  on insert/update/delete.
- **Rationale**: Meets FR-013 (search across all recorded information,
  including free-form notes) and the 500ms search budget (Principle IV) at
  the 10,000-record scale via native indexed search rather than an
  application-level scan.
- **Alternatives considered**: `LIKE '%term%'` scans across columns
  (rejected — does not scale to the 500ms/10k-record budget, and cannot
  rank/tokenize free-form notes well); an external search engine
  (Tantivy, Meilisearch) (rejected — adds a second storage engine/process
  to keep in sync and encrypted, violating the "single encrypted
  container" and YAGNI constraints when FTS5 already meets the need).

## 4. Photo/document storage: BLOBs in the encrypted DB, with a cached thumbnail column

- **Decision**: Store original photo/document bytes as BLOBs on their
  respective tables; additionally store a small pre-generated thumbnail
  BLOB (e.g., ~200px JPEG) per photo, generated once at insert time.
- **Rationale**: Keeps the "single encrypted container" property (no
  plaintext files ever touch disk outside of an explicit, disclosed
  export) while still meeting the 500ms browse/search and 1s
  record-open budgets — list/tile views only ever read the small cached
  thumbnail BLOB, never the full-resolution original, so browsing
  performance is independent of photo file size.
- **Alternatives considered**: Store only file paths in the DB with
  photos on the filesystem (rejected — the user's own stated constraint
  is a single encrypted container, and it reintroduces a class of bugs
  around orphaned files on delete); generate thumbnails on-the-fly on
  every browse render (rejected — decoding full-resolution images per
  render risks missing the 500ms budget at collection scale).

## 5. Key management: keyring crate

- **Decision**: `keyring` crate to store/retrieve the SQLCipher passphrase
  from the OS-native secret store (Windows Credential Manager, macOS
  Keychain, Linux Secret Service/libsecret via `zbus`/`dbus` backend).
  First run generates a random high-entropy key (not a user-memorized
  password) and stores it via `keyring`; the app reads it at startup to
  issue `PRAGMA key` before any other DB access.
- **Rationale**: Satisfies "encryption keys MUST never be logged or
  transmitted in plaintext" by relying on the platform's own secured
  storage rather than a config file; a generated key (vs. user password)
  avoids weak-passphrase risk while keeping the unlock flow silent
  (no password prompt) since the OS credential store already gates
  access behind the user's login session.
- **Alternatives considered**: User-supplied master password prompted at
  every launch (rejected as a UX regression the spec never asked for, and
  weaker in practice — users choose weak passphrases; still offered later
  as an optional "extra passphrase" layer if a future feature requests
  it, but out of scope here); storing the key in a config file with
  restrictive file permissions (rejected — explicitly weaker than a
  keychain-backed secret store and closer to "plaintext on disk").

## 6. Export/import format handling: Rust-side libraries

- **Decision**: Do CSV and Excel (.xlsx) export/import entirely in the
  Rust backend: `csv` crate for CSV, `rust_xlsxwriter` for writing .xlsx,
  `calamine` for reading .xlsx (and CSV) on import.
- **Rationale**: Keeps the business logic that must be `cargo test`-ed
  under the constitution's no-mocks rule (import matching, per-row
  validation/error reporting, conflict resolution) in one place, run
  against a real SQLCipher DB in integration tests, rather than splitting
  parsing (JS) from matching (Rust) across the IPC boundary. It also
  keeps export/import naturally asynchronous as a single Tauri command
  emitting progress events, rather than requiring the frontend to stream
  large files through IPC for parsing.
- **Alternatives considered**: `exceljs` in the frontend (rejected —
  photos are BLOBs inside the encrypted DB that only the Rust side can
  decrypt/stream, so a JS-side spreadsheet library would still need most
  data round-tripped from Rust anyway, and it would split the
  well-tested-by-`cargo-test` business logic across two languages).

## 7. UI component library: Radix-based (shadcn/ui pattern)

- **Decision**: One shared component library built on Radix UI primitives
  (unstyled, accessible dialog/dropdown/tabs/form primitives) styled once
  and reused everywhere, following the shadcn/ui pattern of vendoring
  owned component source rather than a black-box npm dependency.
- **Rationale**: Satisfies Principle III's "single, documented set of UI
  components... no screen may invent its own navigation, confirmation, or
  error-handling pattern" — Radix primitives ship with the ARIA roles,
  focus management, and keyboard interaction needed for a WCAG 2.1 AA
  baseline (dialogs trap focus, dropdowns are keyboard-navigable, etc.),
  and owning the component source means any deviation gets reflected back
  into the shared library rather than left as a one-off, as the
  constitution requires.
- **Alternatives considered**: Adobe React Spectrum (also strong
  accessibility, but a more opinionated design language harder to
  reskin — rejected only for iteration speed, not accessibility);
  hand-rolled components (rejected — reinventing accessible dialog/
  combobox/focus-trap behavior is exactly the kind of speculative
  complexity Principle I warns against when mature accessible primitives
  exist).

## 8. E2E testing: WebdriverIO + tauri-driver

- **Decision**: WebdriverIO as the E2E test runner, driven through
  `tauri-driver`, Tauri's own WebDriver-protocol harness for its native
  window.
- **Rationale**: Tauri 2.x's officially documented E2E path is
  `tauri-driver` (which wraps the platform's native WebDriver server —
  WebView2 on Windows, WebKitWebDriver on Linux, and the equivalent on
  macOS) plus a WebDriver-protocol client. Playwright's driver targets
  Chromium/Firefox/WebKit browser binaries via CDP/its own protocol and
  has no first-party support for automating a Tauri-hosted native
  webview, so it cannot reliably drive the actual shipped window.
- **Alternatives considered**: Playwright (rejected for driving the
  native Tauri window itself, for the reason above; may still be used
  later purely for isolated frontend-in-browser smoke tests, but that is
  not a substitute for the acceptance-scenario E2E coverage this feature
  needs); manual QA only (rejected — conflicts with Principle II's
  requirement that every feature ship with automated tests covering its
  acceptance scenarios).

## 9. Business logic placement: pure Rust services, thin IPC commands

- **Decision**: `services::valuation`, `services::insurance_status`, and
  `services::import_matching` are pure functions/modules operating on
  already-loaded Rust structs (no direct SQL inside them); `commands::*`
  are thin `#[tauri::command] async fn` handlers that load data via `db`,
  call into `services`, and return results/emit progress events.
- **Rationale**: Makes the calculations behind FR-015/016/017/024/028
  (value summary, under/uninsured, policy-expiry warnings) and
  FR-026/030 (import matching/conflict resolution) directly unit-testable
  in `cargo test` without spinning up Tauri's IPC layer, while the
  integration tests around `commands::*` still exercise the real
  SQLCipher DB end-to-end per the constitution's no-mocks rule.
- **Alternatives considered**: Business logic inline inside command
  handlers (rejected — harder to unit test in isolation from the DB and
  IPC layer, and tends to grow into non-single-purpose functions,
  against Principle I).

## 10. Generic per-type thumbnails

- **Decision**: Ship a small set of bundled generic thumbnail images (one
  per initial firearm type: Handgun, Rifle, Shotgun, Other) as Tauri app
  resources, referenced by type when a firearm has no photos (FR-009).
- **Rationale**: Matches the spec's Assumptions section (initial common
  type set, each with its own generic thumbnail, growable later) and
  keeps generic thumbnails out of the encrypted DB (they contain no user
  data, so there is no privacy reason to encrypt them, and bundling them
  as app resources means they load instantly with no DB round-trip).
- **Alternatives considered**: Storing generic thumbnails as rows in the
  DB (rejected — unnecessary complexity for static, non-user data; adding
  a new firearm type would require a data migration instead of just an
  app resource + lookup entry).

## Summary of open questions

None remaining. All Technical Context items are resolved above; the three
items the user's input phrased as "or" choices (spreadsheet library
placement, E2E harness, component library) are resolved in sections 6, 7,
and 8.
