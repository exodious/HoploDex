---
description: "Task list for the troubleshooting log"
---

# Tasks: Troubleshooting Log

**Input**: Design documents from `/specs/013-troubleshooting-log/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: Every story's tests are written first and expected to fail, at the level plan.md's Test Levels gives them.

## Format: `[ID] [P?] [Story] Description`

Bare IDs here mean this feature's. In code, cite them as `013 FR-001`, `013 US1-2`, `013 T012`, `013 research.md §3`.

---

## Phase 1: Setup

- [X] T001 Confirm the baseline passes: `scripts/dev-container.sh npm test` and the Rust tests, in the background with a log.

---

## Phase 2: Foundational

**Purpose**: the rule every line follows, and the module the stories build on.

- [X] T002 Write failing unit tests in src-tauri/src/logging.rs for `shape` (`013 US2-1`, `013 US2-2`, research.md §5: home, a drive letter, `/run/media`, `/media`, `/mnt`, `/Volumes`, the portal's `/run/user/*/doc`, UNC, a relative path, an extension off the list, a planted name in each position), for `db_error`, and for the line backstop (home replaced, control characters escaped).
- [X] T003 Implement `logging::place`, `logging::db_error` and the line backstop; remove `folder_of`.
- [X] T004 Write the source check, src-tauri/tests/log_lines_test.rs (FR-030): walks `src/**/*.rs`, finds each log macro's whole body, and fails on `.display()`, `to_string_lossy`, a raw path variable or a `{...path...}` capture not going through `place`.
- [X] T005 Bring every existing log line within the rule (FR-014, FR-016; research.md §6): move the 64 path lines to `place`, `reach.describe()` too, and change `commands/backups.rs` row counts, `db/mod.rs` integrity text, `db/upgrade.rs` row id, `services/sandbox.rs` raw entry, rusqlite errors through `db_error`, and `lifecycle.rs`'s `err.message`. Read each `{err}` for what its type can print.
- [X] T006 Update `e2e/specs/flatpak.e2e.ts` and `scripts/flatpak/check.sh` for the reach line's shapes.

**Checkpoint**: T004 passes, with stderr's lines already within the rule.

---

## Phase 3: User Story 3 - Told, and able to say no (Priority: P2, built first: the decision underlies the others)

- [X] T007 [US3] Write failing tests: `logging::decide` for data-model.md's table and FR-001's version cases (unit), and the choice's round trip in machine settings, with an unknown value reading as absent, in src-tauri/tests/machine_settings_test.rs.
- [X] T008 [US3] Implement `decide`, `LogState`, and `machine_settings`' `troubleshootingLog` and `troubleshootingLogNoticeShown`.
- [X] T009 [US3] Add `ChooserNotice::TroubleshootingLog`; queue it once from `main.rs` when data-model.md's table says so.
- [X] T010 [US3] Vitest first, then the chooser: FR-021's text, the Turn off button and its follow-up text, in src/features/databases/DatabaseChooser.tsx, types.ts and databasesService.ts.

---

## Phase 4: User Story 5 - The file stays small and goes away (Priority: P3, built first: the file is what US1 shows)

- [X] T011 [US5] Write failing tests in src-tauri/tests/troubleshooting_log_test.rs: rotation at the cap with one file kept (`013 US5-1`), age (`013 US5-2`), off deletes both (`013 US5-3`), a symlink and a hard link left alone (`013 US5-5`), an unwritable folder doesn't fail (FR-011), mode 0600 on Unix.
- [X] T012 [US5] Implement src-tauri/src/logging/file.rs (research.md §4).

---

## Phase 5: User Story 1 - Gather a log for a repeatable issue (Priority: P1) 🎯 MVP

- [X] T013 [US1] Write failing tests: `013 US1-1`, `013 US1-2`, `013 US1-3`, the table in contracts/tauri-commands.md (a raise saves nothing and lasts until quit, `detailed` from off refused) in src-tauri/tests/troubleshooting_log_test.rs; only HoploDex's targets reach the file and debug lines stay out of stderr (FR-019, FR-026), as unit tests.
- [X] T014 [US1] Wire the file into `logging.rs`: `init`, `start`, the early-line buffer, the second output, the header (FR-018), relative time, `set_max_level` at runtime, and call `start` from `main.rs` once `machine.json` is loaded.
- [X] T015 [US1] Add src-tauri/src/commands/troubleshooting_log.rs with the three commands, and add them to `generate_handler!`, `COMMANDS`, `UNSCOPED_COMMANDS` and the capabilities.
- [X] T016 [US1] Add `Opener::reveal` (AppOpener on each OS, E2eOpener's record) and the portal's `OpenDirectory` in src-tauri/src/commands/documents.rs (research.md §10), with a test in src-tauri/tests/portal_opener_test.rs.
- [X] T017 [US1] Vitest first (`013 US1-5`, `013 US1-6`), then src/features/app/TroubleshootingLogSection.tsx in AboutDialog.tsx and about.css, per contracts/ui.md. Change `AboutDialog.test.tsx`'s "sends no command" to say what it now means, and add `_Amended by_` beside 010 SC-005.
- [X] T018 [US1] Set `HOPLODEX_LOG=off` in e2e/support/sandbox.ts and record it in docs/development/test-isolation.md.
- [X] T019 [US1] E2E journey in e2e/specs/databases.e2e.ts as `describe("013 US1 - Gather a log for a repeatable issue")`.
- [X] T020 [US1] Add the Flatpak's path and Show log file to e2e/specs/flatpak.e2e.ts.

---

## Phase 6: User Story 4 - An issue during startup (Priority: P2)

- [X] T021 [US4] Write failing tests: `013 US4-1` (lines from before `start` reach the file), `013 US4-2`, `013 US4-3`, `013 US4-4` (an unknown value isn't quoted).
- [X] T022 [US4] Implement `HOPLODEX_LOG` in `init`.

---

## Phase 7: User Story 2 - Designed to hold nothing sensitive (Priority: P1)

- [X] T023 [US2] Add the debug lines (FR-025, about 30): open, close, lock and its reason; backups; import phases; helper start and exit; preview; portal calls and results. Each within FR-016, none in search, per-row loops or the idle tick.
- [X] T024 [US2] Add the panic line (FR-027), with a test using a child process, as `crash_reports`' test does.
- [X] T025 [US2] Write src-tauri/tests/log_contents_test.rs (FR-028): planted home, database name, record values, document and photo names, draft text and passphrase through create, save, attach, open, backup, restore, export, import, change passphrase, a wrong passphrase, and failures of attach and backup, at the raised level. Fails if any reaches the file; the backup and the export hold no log (`013 US5-4`).
- [X] T026 [US2] E2E in e2e/specs/databases.e2e.ts as `describe("013 US2 - The log is designed to hold nothing sensitive")`: the same planted names through the real app, scanning the real file (FR-029).
- [X] T027 [US3] E2E in e2e/specs/databases.e2e.ts as `describe("013 US3 - Told, and able to say no, while it is on by default")`: the notice once, Turn off, gone at the relaunch.

---

## Phase 8: Docs and surface

- [X] T028 Update docs/attack-surface.md: `logging` (Code, Ask, Accepted per research.md §9), `ipc-commands` (three commands), `flatpak` (`OpenDirectory`).
- [X] T029 Correct CLAUDE.md's "A log line carries nothing from inside a database" to the new rule.
- [X] T030 Update `logging.rs`'s header, docs/architecture/ (the logger, the IPC list), docs/development/ (how to read the log in a test run).
- [X] T031 README: the log and FR-022's wording, and how to gather a log, with the one-line start on each OS. Flatpak metainfo: the same claim, no wider. Add `_Amended by_` beside 011 research.md §20.
- [X] T032 Add the notice and About's states to e2e/screenshots/screens.e2e.ts.
- [X] T033 Add a note in the release checklist docs only if a step changes: FR-001 means none does. Say so in the PR.

---

## Phase 9: Validation on every environment

- [X] T034 Linux dev container: format, lint, `npm test`, Rust tests, the dependency and license audit, in the background with logs.
- [X] T035 Linux dev container: `npm run build`, then E2E (all specs).
- [X] T036 Flatpak: `scripts/dev-container.sh --flatpak npm run test:flatpak`.
- [X] T037 macOS VM (take the lock): Rust tests and E2E, in the task's folder on the external volume.
- [X] T038 macOS VM: launch with `open --env HOPLODEX_LOG=debug` and read the file; window-only screenshots.
- [X] T039 Windows VM (take the lock): Rust tests and E2E, in the task's folder.
- [X] T040 Windows VM: launch with the variable from a shell and read the file; window-only screenshots.
- [X] T041 Linux: window-only screenshots, before and after.
- [X] T042 Check SC-006 against the performance test with the level raised (`tests/performance_test.rs`). That test installs no logger, so it passed 25/25 without the level raised; SC-006 is shown by a line's measured cost (10,000 lines about 33 ms, `troubleshooting_log_test`) and by no debug line in search, per-row or idle code.
- [X] T043 Report every test that failed then passed on rerun, and every skip, as they happen.

---

## Phase 10: Close-out

- [X] T044 Run `/ai-security-review pr` before the PR; keep its three lines in `## Security review`.
- [X] T045 PR description: `Fixes #128.`, screenshots, the persistence and performance notes, what wasn't run. Check `closingIssuesReferences`.
- [X] T046 Measure the log's volume per run in the Linux dev container and record it in research.md §8.
