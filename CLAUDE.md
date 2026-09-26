# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

HoploDex is a local-only firearm collection inventory app: Tauri 2 with a Rust backend (`src-tauri/`) that owns persistence, encryption and business logic, and a React 18 + TypeScript frontend (`src/`) that owns the UI.

## Commands

`DEVELOPMENT.md` is the single source for development instructions: the dev container, prerequisites, and the build, test (including single-file and single-spec runs), lint, dependency and license audit, screenshot and human-testing commands. Read the relevant section before running any of them. One thing that catches people out: run `npm run build` before `test:e2e` or `screenshots`, since the E2E build embeds whatever is in `dist/`.

## Run tests in the dev container

On a host with podman, run tests, lint, the dependency audit and screenshots through `scripts/dev-container.sh` by default, not directly on the host: put the command after the wrapper, and use `bash -c '...'` to chain several.
- **Already inside?** Check first. The container's working directory is `/workspace` and its hostname is `hoplodex-dev`. If both match, run the commands directly and don't nest the wrapper.
- **First runs are slow** (image build, then an empty `node_modules` and `target`). Give long commands a generous timeout, or run them in the background.
- **Waiting on a background run:** redirect its output to a log file, end it with a short summary (`echo "exit $?"` plus a `grep` for passing/failing), and wait for the completion notification. Don't write `until …; sleep` poll loops. Don't end a background command with `| tail -N` either: nothing reaches the output file until the command exits, so it looks hung.
- Don't use the options that pass the host's identity through (`--git-config`, `--ssh-agent`, `--gh-token`, `--anthropic-api-key`) unless the user asks. Commit from the host. `--gui` is only for `tauri dev` and human testing; tests don't need it.
- Use `--build` after a `Dockerfile` change. Never use `--reset-volumes=all` without asking, because it logs the user out of `gh` and `claude`.
- Run tests directly on the host only if podman isn't available or the user asks.

## Never touch the real database

The developer uses the app day to day. Their real encrypted DB is at `~/.local/share/com.hoplodex.app/hoplodex.db`, and its key is in the OS keyring. Never open, modify or delete it from a session. Tests, E2E runs, screenshots and human testing are all isolated from it; "Test isolation" in `DEVELOPMENT.md` says how. Keep it that way when you change any of them, and don't run the app outside that tooling.

## Commit and push

When the user says "commit and push", commit every change git doesn't ignore: modified, deleted and untracked files, including changes made before the session started or outside it, not only your own. Review `git status` and `git diff` first. Stop and ask if something looks like it shouldn't be committed, such as secrets, credentials, large binaries or stray scratch files.

## Architecture

**IPC boundary.** The frontend reaches the backend only through `invoke()` in `src/services/tauriClient.ts`. It turns a backend rejection into a typed `CommandFailure { code, message, fieldErrors }`, which mirrors `CommandError` in `src-tauri/src/commands/error.rs`. That is the only error shape a command returns. Codes are stable strings (`VALIDATION_ERROR`, `NOT_FOUND`, `INTERNAL_ERROR`, domain codes such as `ORIGINAL_MARKS_MATCH`). Raw DB errors are logged and never forwarded (`CommandError::from_db`). Some warnings can be overridden: the command fails with a warning code, and the UI resends it with `confirmedWarnings: true`.

**Backend layering** (`src-tauri/src/`):
- `commands/<area>.rs` holds each area's `#[tauri::command]` functions. They are thin: lock `DbHandle` (a single `Mutex<Connection>`, deliberately with no pool), then call the matching function in that file's `pub mod ops`. `ops` functions take `&Connection` and hold the actual logic. Integration tests and `examples/human_seed.rs` call `ops` directly, so they exercise the same code paths as the app.
- `models/` holds the serde types (camelCase over IPC), input validation (`validate_*_input`) and enums.
- `services/` holds pure/domain logic: spreadsheet import/export (`COLUMNS` is the canonical column list), import matching, insurance status, valuation, attachment storage, secure delete.
- `db/mod.rs` gets or creates a random 256-bit SQLCipher key in the OS keyring (or an in-memory mock under `mock-keyring`), opens the encrypted DB and applies `db/migrations/*.sql`, tracked in `schema_migrations`.
- `main.rs` registers plugins and state, and lists every command in `generate_handler!`. A new command must be added there. It also clears decrypted document copies on exit, on SIGTERM/SIGHUP/SIGINT and at startup (FR-035).

**Frontend** (`src/`): `features/<area>/` each has a `*Service.ts` (typed wrappers over `invoke`, one per command), `types.ts` mirroring the Rust models, and the components and CSS. `features/app/` holds the shell, navigation, theme, and `CollectionProvider`/`collectionStore` (shared collection state: policies, value summary). Shared primitives, built on Radix, live in `src/components/` and are exported from `index.ts`. Design tokens are in `src/styles/tokens.css`. Money values are **whole dollars** (integers), not cents; see `src/lib/money.ts`.

**File drops** arrive as filesystem paths through Tauri's native drag-drop event (`listenForFileDrops`). They go to the `add_*_from_path` commands. WebKitGTK gives HTML5 drops no `File`, so leave `dragDropEnabled` at its default and don't switch to HTML5 file drops.

## Conventions that aren't obvious from the code

- **Schema changes edit the existing migrations in place** (`0001_initial.sql`, `0002_fts5.sql`) while the app is unreleased. Don't add a data migration. Existing local DBs become incompatible, so tell the user instead of touching their DB.
- **Keep the human-testing seed in step with the data model.** When you add a column, table or spreadsheet column, seed a record that uses it in `src-tauri/examples/human_seed.rs` and add it to the import samples, in the same change. `tests/human_seed_coverage_test.rs` fails otherwise. Fix it by seeding, not by loosening the test.
- **UI consistency is a constitution principle.** When you change a pattern in one form or dialog (field widths, grouping, labels, hints), carry it to the others: FirearmForm, DisposeDialog, RestoreDialog, CoverageDialog, InsurancePolicyForm, PolicyDeleteDialog, Import/ExportDialog. Short fields use the `hd-field--quarter`/`hd-field--third` and `hd-form-grid--*` classes in `src/features/firearms/forms.css`. If a change adds a screen, add it to the walk in `e2e/screenshots/screens.e2e.ts`.
- **Tests hit real persistence and never mock the DB.** Every behavior change needs a test, and bug fixes need a regression test (constitution II).
- CI is disabled on purpose (`.github/workflows-disabled/`). Don't re-enable it or flag its absence.

## Spec Kit workflow

Features are specified under `specs/NNN-name/` (`spec.md`, `plan.md`, `data-model.md`, `contracts/tauri-commands.md`, `contracts/spreadsheet-format.md`, `tasks.md`), driven by the `/speckit-*` skills. `.specify/feature.json` names the active feature. Code comments cite requirement IDs (`FR-033`, `research.md §5`). Keep those references accurate, and update the contracts when a command's shape changes. The constitution in `.specify/memory/constitution.md` overrides other guidance. Its gates: lint, tests and the dependency audit pass, UI PRs carry before/after screenshots, persistence PRs describe how they meet the security/data-handling constraints, and performance-sensitive PRs note their impact against the budgets (search ≤500ms and actions ≤1s at 10,000 items). Before every release, a whole-codebase AI-assisted security review runs and its dated report is committed.

`spec_TODO.md` is temporary. Never reference it (or its item labels like "A1" or "Track B") from specs, code, comments, tests or commit messages. Copy what's needed into the spec's `## Source Request` section instead.
