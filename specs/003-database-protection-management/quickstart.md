# Quickstart: Validate Database Protection, Portability & Management

This is a runnable validation guide, not an implementation spec. It proves
the feature works end to end by walking through each user story's
independent test from [spec.md](./spec.md). Command, event, error and table
names refer to [data-model.md](./data-model.md) and [contracts/](./contracts).
The design reasons are in [research.md](./research.md).

## Prerequisites

- The development container (`scripts/dev-container.sh`, DEVELOPMENT.md) on
  Linux, or Rust, Node.js and the Tauri prerequisites on macOS or Windows.
- Node via nvm on `PATH`. Use `npm@11` when the lockfile changes: this
  feature adds `@zxcvbn-ts/core`, `@zxcvbn-ts/language-common`,
  `@zxcvbn-ts/language-en` and `@radix-ui/react-dropdown-menu`, and the Rust
  crates `unicode-normalization`, `gethostname` and `fs4`. It makes `zeroize`,
  `zbus`, `same-file` and the platform crates direct dependencies (research
  §1, §4, §6, §14, §18). `npm run audit` must pass afterwards, including the
  license audit.
- **Never the real database.** The app no longer opens anything by itself:
  it shows the chooser. Test runs use scratch `XDG_*` directories (E2E,
  screenshots, human testing) or temp directories (`cargo test`). The
  developer's pre-feature database at `~/.local/share/com.hoplodex.app/hoplodex.db`
  and its `sqlcipher-key` keyring entry are left alone. No code reads them
  now, and nothing may delete them (research §10).

## Automated test commands

Run through the dev container wrapper, as in CLAUDE.md:

```bash
scripts/dev-container.sh cargo test --manifest-path src-tauri/Cargo.toml
scripts/dev-container.sh cargo test --manifest-path src-tauri/Cargo.toml --features mock-keyring --test keyring_test
scripts/dev-container.sh npm run test
scripts/dev-container.sh bash -c 'cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets && cargo fmt --manifest-path src-tauri/Cargo.toml --check && npm run lint && npm run format:check'
scripts/dev-container.sh bash -c 'npm run build && npm run test:e2e -- --spec e2e/specs/us7-databases.e2e.ts'
```

E2E specs run one at a time. `npm run build` comes first, because the E2E
build embeds `dist/`.

## Scenario map: story → tests

Write each test to fail first, then pass (constitution II). Every Rust test
uses a real SQLCipher file in a temp directory, created through
`db::create_database` with the fixed test passphrase and the production
cipher settings (research §20).

| Story | Independent test (spec) | Automated in |
|---|---|---|
| US1 — passphrase | Create with a passphrase, add a firearm, reopen: correct passphrase opens, a wrong one gives `PASSPHRASE_INCORRECT` and leaves the file byte-identical; the keyring gains nothing; min length, NFC, NUL; the acknowledgement is required | `tests/passphrase_protection_test.rs`, `PassphraseField.test.tsx`, `CreateDatabaseDialog.test.tsx`, `e2e/specs/us7-databases.e2e.ts` |
| US2 — several databases, portability | Two databases in two folders with different passphrases, switch between them; recent list order, unavailable entry, locate, remove (file untouched); newer version refused, file unchanged; in use (second connection gets `DATABASE_IN_USE`); open elsewhere → refusal → take over; own stale marker cleared; unsaved changes → save, discard or cancel | `tests/database_open_test.rs`, `tests/take_over_test.rs`, `tests/machine_settings_test.rs`, `tests/portability_test.rs`, `DatabaseChooser.test.tsx`, `SessionProvider.test.tsx`, `us7-databases.e2e.ts` |
| US3 — backups and restore | With a simulated date: backup on close after changes; none on a second close the same day; the waiting changes backed up the next day; none after an unchanged session; rotation keeps N; backups off → none; location missing → close goes ahead and notice; partial file after an interrupted copy never listed and cleaned at next launch; marker and pending changes not in the copy; restore brings back exactly the earlier state and makes a "before restoring" backup | `tests/backup_test.rs`, `tests/backup_due_tracking_test.rs`, `tests/restore_test.rs`, `DatabaseSettingsDialog.test.tsx`, `RestoreBackupDialog.test.tsx`, `e2e/specs/us8-backups.e2e.ts` |
| US4 — change passphrase | Opens only with the new passphrase, content identical (row counts, blobs); previous file gone; not enough space → refused before starting; interrupt at copy, at verify and just before the rename → opens with the old passphrase, no `.new` left; hard-link-less fallback recovers from each gap; saved keyring copy updated | `tests/passphrase_change_test.rs`, `tests/file_swap_test.rs`, `ChangePassphraseDialog.test.tsx` |
| US5 — remember on this computer | Opt in → next open needs no passphrase; other database still asks; forget → asks again and the entry is gone; unavailable keyring (`HOPLODEX_E2E_KEYRING=unavailable`) → option disabled with explanation; stale saved passphrase → prompt, then updated | `tests/keyring_test.rs` (`--features mock-keyring`), `DatabaseChooser.test.tsx`, `us7-databases.e2e.ts` |
| US6 — locking | Lock now → `session:closed` then chooser with the database selected, no collection DOM, document copies gone, due backup made; idle clock (injected clock): locks at 10:00–10:01, input restarts it, operations and native dialogs pause it, wall-clock jump after sleep locks; sleep procedure order (stop import mid-run keeps imported rows; stop passphrase change leaves old file; pending draft written before the connection closes; no backup); screen lock → normal close with backup; pending changes resume and discard; not in backups; passphrase fields cleared on the system event | `tests/lock_test.rs`, `tests/pending_changes_test.rs`, `useIdleActivity.test.ts`, `usePendingDraft.test.tsx`, `PendingChangesDialog.test.tsx`, `e2e/specs/us9-locking.e2e.ts` |
| Housekeeping vs changes | Every collection table has the three triggers; opening and closing without changes never sets `changes_waiting` | `tests/backup_due_tracking_test.rs` |
| Performance | Open at 10,000 firearms ≤ 2 s including key derivation (SC-003); the backup progress event within 100 ms of starting (SC-005); `session.write`'s fingerprint check adds no measurable cost to the existing budgets | `tests/performance_test.rs` |
| Seed in step | New tables and backup-only columns are seeded | `tests/human_seed_coverage_test.rs` |

## Walkthroughs (by hand, against scratch data)

Use `scripts/human-testing.sh` (it now seeds two databases: see
plan.md), or `scripts/dev-container.sh --gui scripts/human-testing.sh`.
The seed's passphrase is printed when it runs.

1. **First run** (US1): with no recent databases, the chooser offers Create
   and Open. Create "Test" in a scratch folder. A short passphrase is
   blocked, the strength hint moves as you type, and Create stays disabled
   until the acknowledgement is ticked. The disk-encryption note appears once
   and stays away after dismissal. Quit, relaunch: the passphrase is asked for
   before anything is shown.
2. **Portability** (US2-6, SC-001): copy `Test.hoplodex` to a different OS
   (or a fresh account), open it with **Open another database file…** and
   the passphrase, and check every record, photo and document.
3. **Open elsewhere** (US2-9, US2-10): open the seeded "Shared collection".
   It is refused, naming the seed's fictitious computer. Take over after the
   confirmation.
4. **Backups** (US3): change something, close. The chooser returns, and a
   `HoploDex backups` folder appears next to the file with one backup. Change
   and close again: no second backup today. Restore it with its passphrase:
   the "before restoring" backup appears, and the content goes back.
5. **Change passphrase** (US4): change it, reopen with the new one, and check
   that the old one fails and that the backups still open with the old one.
6. **Lock** (US6): press Ctrl+L while editing a firearm. Reopen: the
   pending-changes prompt names the firearm, and Resume brings back the exact
   input. Set the idle lock to 1 minute and wait.

### Platform checks (manual, each OS, before merge)

The OS notices (research §14) cannot be driven from CI or the container.
Check on each available OS, and record the results in the PR description:

| Check | Linux (GNOME/KDE) | macOS | Windows |
|---|---|---|---|
| Suspend with a database open and the idle lock on → locked before or on waking; no collection visible on wake | `systemctl suspend` | Apple menu → Sleep | Start → Power → Sleep |
| Suspend during a large backup or passphrase change → stopped; original intact | same | same | same |
| Screen lock with "lock when the screen locks" on → locks with a backup | Super+L / `loginctl lock-session` | Ctrl+⌘+Q | Win+L |
| Screen lock clears a half-typed passphrase in the chooser | same | same | same |
| Logout or shutdown while editing → pending changes offered at the next open | log out | log out | sign out |
| Screen-lock option shown as unavailable where it cannot work | a bare window manager without logind session locking | — | — |

## Done when

- Every acceptance scenario in spec.md maps to a passing test above, or to a
  recorded platform check.
- `cargo test`, `npm run test`, lint, format and `npm run audit` pass; the
  three new E2E specs pass one at a time.
- `npm run screenshots` produces the new screens in contracts/ui-databases.md
  §15 for the PR's before/after evidence.
- The dependency audit passes with no new exception for the dependencies
  this feature adds or promotes (constitution 1.1.0).
- The pull request carries the security and data-handling note. The release
  security review is not a merge gate for this feature; it runs at the first
  release (spec Assumptions).
