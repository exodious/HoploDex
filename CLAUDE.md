# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

HoploDex is a local-only firearm collection inventory app: Tauri 2 with a Rust backend (`src-tauri/`) that owns persistence, encryption and business logic, and a React 18 + TypeScript frontend (`src/`) that owns the UI. The README has full setup instructions (prerequisites per OS, WebKitWebDriver on Arch, gnome-keyring for headless sessions).

## Commands

```bash
# Build
npm run build                                          # tsc typecheck + vite build -> dist/
cargo build --manifest-path src-tauri/Cargo.toml
npm run tauri dev                                      # full app, hot reload

# Tests
cargo test --manifest-path src-tauri/Cargo.toml                                # all Rust tests
cargo test --manifest-path src-tauri/Cargo.toml --test firearm_lifecycle_test  # one integration-test file
cargo test --manifest-path src-tauri/Cargo.toml --test firearm_lifecycle_test <name_substring>
npm test                                               # Vitest (jsdom)
npx vitest run src/features/firearms/FirearmForm.test.tsx
npm run test:e2e                                       # WebdriverIO against a release build, under Xvfb
npm run test:e2e -- --spec e2e/specs/us1-record-firearm.e2e.ts
xvfb-run -a python3 e2e/scripts/quit-cleanup.py        # decrypted-document cleanup on quit (Linux)

# Lint / format (CI is intentionally disabled; run these locally)
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo clippy --all-targets --manifest-path src-tauri/Cargo.toml -- -D warnings
npm run lint                                           # eslint, --max-warnings 0
npm run format:check

# UI evidence and manual testing
npm run build && npm run screenshots                   # real-app WebKitGTK shots -> e2e/screenshots-out/
scripts/human-testing.sh [--reset] [--extra N]         # seeded collection in .human-testing/, then tauri dev
```

## Run tests in the dev container

On a host with podman, run tests, lint and screenshots through the dev container by default, not directly on the host. Put each command above after the wrapper:

```bash
scripts/dev-container.sh cargo test --manifest-path src-tauri/Cargo.toml
scripts/dev-container.sh npm test
scripts/dev-container.sh bash -c 'npm run build && npm run test:e2e -- --spec e2e/specs/us1-record-firearm.e2e.ts'
scripts/dev-container.sh bash -c 'npm run build && npm run screenshots'
```

The container has none of the host's app data, so tests can't reach the real database (see below). It also has every E2E dependency: Xvfb, `tauri-driver`, `WebKitWebDriver` and a keyring.
- **Already inside?** Check first. The container's working directory is `/workspace` and its hostname is `hoplodex-dev`. If both match, run the commands directly and don't nest the wrapper.
- **First runs are slow.** The first call builds the image. After that, `node_modules` and `src-tauri/target` live in per-checkout volumes, separate from the host's copies, so the first `npm ci` and cargo build start from scratch. Give long commands a generous timeout, or run them in the background.
- Don't use the options that pass the host's identity through (`--git-config`, `--ssh-agent`, `--gh-token`, `--anthropic-api-key`) unless the user asks. Commit from the host. `--gui` is only for `tauri dev` and human testing; tests don't need it.
- Use `--build` after a `Dockerfile` change. `--reset-volumes` rebuilds this checkout's `node_modules` and `target` from scratch. Never use `--reset-volumes=all` without asking, because it logs the user out of `gh` and `claude`.
- Run tests directly on the host only if podman isn't available or the user asks. In that case, follow the isolation rules below.

**Run `npm run build` before `test:e2e` or `screenshots`.** The E2E harness runs `cargo build --release --features custom-protocol,mock-keyring`, which embeds whatever is currently in `dist/`.

## Never touch the real database

The developer uses the app day to day. Their real encrypted DB is at `~/.local/share/com.hoplodex.app/hoplodex.db`, and its key is in the OS keyring. Never open, modify or delete it from a session.
- Rust tests use `tests/support::TestDb`, a real SQLCipher DB in a temp dir.
- `e2e/wdio.conf.ts` gives each session throwaway `XDG_*` dirs and a stub `xdg-open`. E2E builds use the `mock-keyring` feature, which generates a fresh key per launch, so a DB left over from one spec breaks the next.
- `scripts/human-testing.sh` and `examples/human_seed.rs` point the app at `.human-testing/` via `XDG_*_HOME` and refuse to target the real data dir.
- Running inside `scripts/dev-container.sh` gives you isolation automatically, which is why it's the default.

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

Features are specified under `specs/NNN-name/` (`spec.md`, `plan.md`, `data-model.md`, `contracts/tauri-commands.md`, `contracts/spreadsheet-format.md`, `tasks.md`), driven by the `/speckit-*` skills. `.specify/feature.json` names the active feature. Code comments cite requirement IDs (`FR-033`, `research.md §5`). Keep those references accurate, and update the contracts when a command's shape changes. The constitution in `.specify/memory/constitution.md` overrides other guidance. Its gates: lint and tests pass, UI PRs carry before/after screenshots, persistence PRs describe how they meet the security/data-handling constraints, and performance-sensitive PRs note their impact against the budgets (search ≤500ms and actions ≤1s at 10,000 items).

`spec_TODO.md` is temporary. Never reference it (or its item labels like "A1" or "Track B") from specs, code, comments, tests or commit messages. Copy what's needed into the spec's `## Source Request` section instead.
