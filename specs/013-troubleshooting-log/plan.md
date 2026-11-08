# Implementation Plan: Troubleshooting Log

**Branch**: `fm/hoplodex-troubleshooting-log` | **Date**: 2026-11-07 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/013-troubleshooting-log/spec.md`

## Summary

`logging.rs` keeps its stderr output and gains a second output: a plain-text file in the app's cache directory, for HoploDex's own lines only. Whether it runs comes from one function of the app's version, the choice saved in `machine.json`, and `HOPLODEX_LOG`. About gets a small section to turn it on and off, raise the level until quit, see the path and show the file. A one-time chooser notice tells a user of a 0.x build. Every log line in the app is brought within a tighter rule: paths become shapes (`logging::place`), and counts, row ids and SQLite's text go, on every output. About 30 debug lines are added. A journey test, an E2E journey and a source check keep the rule (research.md §1–§10).

## Technical Context

**Language/Version**: Rust 1.97, TypeScript with React 18 (Tauri 2)

**Primary Dependencies**: Existing only: `log`, `tauri-plugin-opener` (`reveal_item_in_dir`), `zbus` (the OpenURI portal), `services::scratch`, `services::secure_delete`. No new crate.

**Storage**: `<app cache dir>/logs/hoplodex.log` and `hoplodex.log.1`; two fields in `machine.json` (data-model.md). Nothing in any database.

**Testing**: `cargo nextest`, Vitest, WebdriverIO E2E; real persistence, never mocked.

**Target Platform**: Linux, Linux Flatpak, macOS, Windows.

**Project Type**: Desktop app.

**Performance Goals**: One unbuffered write per line, on the calling thread, only on paths that already do slower work. Search ≤500 ms and actions ≤1 s at 10,000 items with the level raised (SC-006): no debug line sits in search or per-row loops.

**Constraints**: A log failure never stops the app. At most 4 MiB. No file access for the helpers.

**Scale/Scope**: 198 existing log calls, 64 of them naming a path, in 15 files; about 30 new; 3 new commands; one new About section; one new notice.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Code Quality**: no new crate or layer; the file output is one small module beside the logger. PASS.
- **II. Testing**: each story has tests at the levels below; tests take the folder and home as parameters and stay out of real data. The E2E sandbox isolates the file through `app_dirs`. PASS.
- **III. UX Consistency**: About's section uses the shared `Button` and banner patterns, the notice uses the chooser's notice pattern, and no dialog gains an input, so the Enter rule is untouched. New screens join the screenshot walk. PASS.
- **IV. Performance**: research.md §8. PASS.
- **Security gates**: persistence and attack surface below; the per-PR AI review runs before the PR.

**Attack surface** (`docs/attack-surface.md`): `logging` changes: new **Code** paths (`src-tauri/src/logging/**`, `src-tauri/src/commands/troubleshooting_log.rs`), new **Ask** questions, and the **Accepted** line replaced by the risks in research.md §9. `ipc-commands` gains three unscoped commands. `flatpak` gains the portal's `OpenDirectory`. The list is updated in this feature's pull request.

## Test Levels

The E2E sandbox sets `HOPLODEX_LOG=off`, so the other specs and the screenshot walk don't see a notice or write a file; these journeys clear it and relaunch.

| Story | E2E journey | Other E2E (error mechanism, real app, security) | Scenarios tested in Rust or Vitest |
|-------|-------------|-------------------------------------------------|------------------------------------|
| US1 | `databases.e2e.ts` (the chooser's setup): from the chooser, open About, turn on, raise, do something, read the sandbox file, Show log file (recorded by `E2eOpener`) | Flatpak: `flatpak.e2e.ts`, the file is at the Flatpak's cache path and Show log file reaches the portal | US1-1, US1-2, US1-3 → Rust `tests/troubleshooting_log_test.rs`; US1-5, US1-6 → Vitest `AboutDialog.test.tsx` |
| US2 | `databases.e2e.ts`: a database with planted names and a record with planted values, at the raised level; the real file holds none of them (security: what is left on disk) | | US2-1, US2-2 → Rust unit tests of the shape; US2-3 to US2-6 → Rust `tests/log_contents_test.rs` (journey, FR-028); source check `tests/log_lines_test.rs` (FR-030) |
| US3 | `databases.e2e.ts`: a first start with the variable cleared shows the notice once; Turn off removes the file and it stays off at the relaunch | | US3-1 to US3-5 → Rust `tests/troubleshooting_log_test.rs` (the decision and the deletion), FR-001 unit test; the notice → Vitest `DatabaseChooser.test.tsx` |
| US4 | Covered by the journey above starting with `HOPLODEX_LOG=debug` for the planted run | | US4-1 to US4-4 → Rust unit and integration tests |
| US5 | None: every scenario can be shown with a real folder | | US5-1 to US5-5 → Rust `tests/troubleshooting_log_test.rs`; backups and exports → asserted in `tests/log_contents_test.rs` |

## Project Structure

### Documentation (this feature)

```text
specs/013-troubleshooting-log/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/tauri-commands.md
├── contracts/ui.md
└── tasks.md
```

### Source Code

```text
src-tauri/src/logging.rs                     # outputs, shape, line rules, panic line
src-tauri/src/logging/file.rs                # the file: create, append, rotate, age, delete
src-tauri/src/commands/troubleshooting_log.rs # the three commands
src-tauri/src/commands/documents.rs          # Opener::reveal, the portal's OpenDirectory
src-tauri/src/services/machine_settings.rs   # the choice and the notice flag
src-tauri/src/models/database.rs             # ChooserNotice::TroubleshootingLog
src-tauri/src/models/troubleshooting_log.rs  # LogState, LogChoice, what the commands return
src-tauri/src/main.rs, commands/mod.rs, models/mod.rs, commands_list.rs, capabilities/  # wiring
src-tauri/src/**                             # every log line within the rule; debug lines
src-tauri/tests/troubleshooting_log_test.rs, log_contents_test.rs, log_lines_test.rs
src/features/app/AboutDialog.tsx, TroubleshootingLogSection.tsx, troubleshootingLog.ts, about.css
src/features/databases/DatabaseChooser.tsx, types.ts
e2e/support/sandbox.ts, e2e/specs/databases.e2e.ts, e2e/specs/flatpak.e2e.ts, e2e/screenshots/screens.e2e.ts
README.md, flatpak/*.metainfo.xml, CLAUDE.md, docs/attack-surface.md, docs/architecture/, docs/development/
```

**Structure Decision**: The existing Tauri layout. `logging` stays one module with a child for the file, as `keyring` has `keyring/`.

## Close-out (at pull request time)

- Issue #128 is closed by the pull request's `Fixes #128.`; check `closingIssuesReferences` lists it.
- No other issue is settled by this work.

## Complexity Tracking

None.
