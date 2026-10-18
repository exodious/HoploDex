# Implementation Plan: License Notices in the App and the License Audit's Last Items

**Branch**: `010-license-notices` | **Date**: 2026-10-18 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/010-license-notices/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

Two issues that stand between HoploDex and its first release under
GPL-3.0-only.

- **#18, About HoploDex (P1)**: one dialog, opened from three places: the
  chooser's top bar, the database menu, and on macOS the application menu's
  About item, which no longer opens the system's panel.
  - **The program block** comes first: name, version, the copyright line, the
    GPL statement, no warranty, the new §7 permission, and the source address
    as selectable text.
  - **Below it**: the full GPL-3.0 text in a disclosure, then a filterable
    list of every third-party component, each one opening its notice texts.
  - **No database or command is involved**: the dialog reads a static file of
    the page.
- **#18, notices that match the build (P2)**: `npm run build` produces
  `dist/notices.json` and `dist/NOTICES.txt` for the build's target, through
  a Vite plugin and a new `scripts/licenses/` module. It shares the license
  audit's definition of "shipped":
  - **npm**: the lockfile's non-development packages, with `devOptional` now
    counted as development. The real bundle's module list is checked against
    that set.
  - **Rust**: `cargo tree` with the release feature for the target.
  - **Texts** come from the packages' own files, and the build fails closed:
    no silent generic text.
  - **The tools' blind spots** are recorded and checked. Hand-recorded
    components have carrier checks: SQLCipher, SQLite, OpenSSL on Windows
    (its version now checked at build), Microsoft's WebView2 loader, and the
    word lists' attributions. Exceptions, clarifications and a guard for new
    native code complete them.
  - **The audit** runs the check for all three OSes on every pull request.
- **#20, the audit's last items (P3)**:
  - **The audit is recorded** (research.md §1).
  - **The word lists**: `wikipedia.json` no longer ships (CC BY-SA 3.0 /
    GFDL), and the other seven ship with recorded sources and attributions
    (§2).
  - **The WebView2 loader** is settled both as a System Library and by a
    GPL-3.0 §7 additional permission (§3, §15).
  - **Published Linux bundles** carry no LGPL libraries (§4).

**Decisions taken at planning**: the owner chose our own script over
`cargo-about` (research.md §5). FR-015 was widened so `wordSequences.json`
(written by hand) and `lastnames.json` (public domain by statute) ship. The
WebView2 loader is settled both ways, which adds FR-019. The spec's
clarifications record all three. Everything else is listed under "Design
choices worth a look" at the end.

## Technical Context

**Language/Version**: TypeScript 5.x / React 18 (`src`), Node 24 ES modules
(`scripts/`), Rust 1.97+ edition 2024 (`src-tauri`, the macOS menu only),
unchanged

**Primary Dependencies**: no new dependency, runtime or development.
- **The notices generator** uses Node's standard library, `cargo tree`,
  `cargo metadata` and `cargo fetch`, and the SPDX evaluator that
  `check-npm-licenses.mjs` already has, which moves into
  `scripts/licenses/spdx.mjs`.
- **The menu** uses Tauri 2's `Menu`, `MenuItem` and `on_menu_event`, which
  are already in `tauri`.
- **`cargo-about` was considered and not taken** (research.md §5).
- **The strength hint** keeps `@zxcvbn-ts/*` and imports their per-list
  modules (research.md §2).

**Storage**: none. No database, setting or `machine.json` change.
- **Build output**: `dist/notices.json` and `dist/NOTICES.txt`, git-ignored
  like the rest of `dist/`.
- **New committed records**: `licenses/additional-permission.txt`,
  `licenses/hand-recorded.json`, `licenses/exceptions.json`,
  `licenses/clarify.json`, `licenses/native-code.json` and
  `licenses/texts/…` (contracts/notices.md).

**Testing**:
- **Vitest** for the generator, with fixtures (`scripts/licenses/*.test.ts`;
  Vitest already runs `scripts/citations.test.ts`). One test runs it against
  the real trees and is skipped, with a message, where `cargo` isn't on the
  PATH.
- **Vitest for the interface**: `AboutDialog.test.tsx`, plus amended
  `DatabaseChooser.test.tsx`, `DatabaseMenu.test.tsx`,
  `SessionProvider.test.tsx` and `strength.test.ts`.
- **A Rust unit test** for the macOS menu's item id and event name.
- **One E2E journey** in `databases.e2e.ts`, with OS-specific steps (the
  macOS menu bar, Windows' linked components).
- **The screenshot walk** gets five new shots in both themes.
- **`npm run audit:licenses`** gains the all-targets check.

**Target Platform**: desktop: Windows 10+, macOS 26+, Linux (WebKitGTK
2.40+), unchanged. Each OS's build gets its own notices. The menu change is
macOS only.

**Project Type**: desktop application (Tauri: React frontend + Rust backend in
`src-tauri/`), plus build tooling in `scripts/`.

**Performance Goals** (constitution IV, SC-004):
- About opens within 1 s. The program block shows at once from compiled-in
  constants; the about 300 KB `notices.json` loads in milliseconds from the
  app's own origin.
- Filtering about 340 names takes well under 500 ms, so it runs without a
  debounce.
- Closed rows render only a title, and a text renders when its row opens.
- **The build**: `cargo tree` and `cargo metadata` add a few seconds to
  `npm run build`. The PR notes the measured cost.

**Constraints**:
- **No silent omission or generic text** (FR-010).
- **No list of shipped dependencies kept by hand** (FR-008).
- **No network from the app** (FR-006): no link, no opener, the CSP as it is.
- **No new command or capability**, and no database touched (FR-001, SC-005).
- **The build stays offline** once Cargo's sources are fetched, and every
  exception's text is committed.
- **`LICENSE` is not modified**: the permission sits beside it (research.md
  §15).

**Scale/Scope**: three user stories (P1–P3), 19 functional requirements, 6
success criteria.
- **What changes in the app**: one new dialog, three entry points (one of them
  native), one event, and one change to the strength hint's dictionary.
- **New tooling**: a notices generator with five kinds of record and about 21
  exceptions.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Requirement | How this plan satisfies it |
|---|---|---|
| I. Code Quality | Lint/static analysis, review, small single-purpose modules, no speculative abstraction | **One definition of "shipped"** (`scripts/licenses/shipped.mjs`), used by the audit and the notices. **One SPDX evaluator and allow list**, moved out of `check-npm-licenses.mjs`, which now imports them (refactor, not a copy). **One About**, rendered by `SessionProvider` for all three entry points. **One owner for each kind of record**: hand-recorded, exception, clarification and native code each have their own file and check. No new dependency: the generator replaces a tool. `eslint`, `prettier`, `clippy`, `rustfmt`, the citations check |
| II. Testing (NON-NEGOTIABLE) | Tests first, real persistence, one test per acceptance scenario, isolation | Every scenario and criterion maps to a test (quickstart.md). Each failure rule has a red fixture before it is implemented. The real trees are checked by the audit and a Vitest test. The E2E journey runs in the sandbox on the seeded database, and shows its file unchanged by SHA-256. No test reads real data |
| III. UX Consistency | One component set and pattern, WCAG 2.1 AA | The shared `Dialog`, `Disclosure`, `TextField`, `Button` and `MenuItem`. The select-all path style from export. The closing screen's delay rule for the loading line. No new navigation pattern. Keyboard paths, focus return and region names in contracts/ui.md §3. The new screen joins the walk. `AboutDialog` joins CLAUDE.md's list of forms and dialogs that share patterns |
| IV. Performance | 100 ms feedback / 1 s completion, 500 ms search, no UI-thread blocking | Program block at once, the list in milliseconds, filtering without a debounce, texts rendered on demand (Technical Context). The E2E journey times both budgets. The build-time cost is measured and noted |
| V. User Privacy | Local only, no telemetry, nothing leaves without consent | About reads no collection data and sends nothing. The source address is text. The CSP and `csp_test.rs` stay as they are |
| Security & Data Handling | Encryption at rest, vetted dependencies, no cipher change | No cipher, key or database path touched. No new dependency to review. The build reads local files, and its one fetch is the lockfile's crates. PR notes come from research.md §16 |
| Licensing | GPLv3-compatible dependencies and assets with recorded source; release carries notices | This feature is that section: the notices every release carries (FR-008–FR-011), the audit record (§1), the word lists settled (§2), the loader (§3, FR-019), the build's OpenSSL version check (§7), and the native-code guard making a manual check automatic |

**Result**: PASS, with the entries in Complexity Tracking.

## Test Levels

*Every acceptance scenario needs a test (constitution II), but not an E2E one.
E2E is the slowest layer and runs on every OS, some of them small VMs, so place
each scenario at the cheapest level that can show it, and say why wherever the
plan departs from these defaults.*

- **E2E, one journey per user story**: Story 1's in `databases.e2e.ts`. Its
  setup is the chooser and opening the seeded database, which is the story's
  path. Stories 2 and 3 are the maintainer's and the reviewer's: their
  journeys are the build and audit checks themselves, and the research record.
- **E2E, one test per error mechanism per form or dialog**: About takes no
  input and has no error path except a failed load, which only a broken build
  could produce. Vitest shows it.
- **E2E, what only the real app shows**:
  - the fetch of `notices.json` through the app's protocol;
  - the macOS menu bar item, through `chooseMenuBarItem`;
  - Windows' linked components, OpenSSL's version among them;
  - the database file left unchanged;
  - the walk.
- **Security**: About's "no network, no database" is enforced by the CSP and
  by calling no command. `csp_test.rs` already guards the first. The E2E
  journey's SHA-256 and Vitest's "no `invoke`" assertion guard the second.
- **Everything else** in Vitest, as the table says. Each test cites its
  scenario (`010 US2-3`).

| Story | E2E journey | Other E2E (error mechanism, real app, security) | Scenarios tested in Rust or Vitest |
|-------|-------------|-------------------------------------------------|------------------------------------|
| US1 | In `databases.e2e.ts` (its setup: the chooser and opening the seeded database). From the chooser: **About HoploDex**, check the program block and the selected source address, open the license, filter "sqlcipher" and open SQLCipher's notice, filter "zzzz" for no match, Escape (focus back on the button). Then open "Main collection" (`human testing passphrase`), **About HoploDex** from the database menu, Ctrl/⌘+L closes it with the lock, unlock, and the file's SHA-256 is unchanged. Timed for SC-004 | macOS only: the menu bar's **About HoploDex**, from the chooser and with a database open. Windows only: OpenSSL 3.x and the WebView2 loader are listed. Both need the real OS build | US1-1 to US1-6, the edge cases (no match, lock, narrow), FR-005, FR-007's region names → Vitest (`AboutDialog.test.tsx`, `DatabaseChooser.test.tsx`, `DatabaseMenu.test.tsx`, `SessionProvider.test.tsx`); US1-7's item id and event → Rust unit test in `main.rs`'s menu module |
| US2 | None. The story is the maintainer's build, and its journey is the build and audit checks | — | US2-1 → Vitest (`notices.test.ts` against the real Linux trees) and `audit:licenses --all-targets`; US2-2 to US2-5 → Vitest with fixtures (`notices.test.ts`, `licenseFiles.test.ts`, `handRecorded.test.ts`, `vitePlugin.test.ts`, `policy.test.ts`) |
| US3 | None. The record is reviewed in the PR; the strength hint's part is unit-level | — | US3-2's shipped lists and US3-3 → Vitest (`strength.test.ts`) and the worker-bundle check; US3-1, US3-4 → research.md and DEVELOPMENT.md, reviewed; FR-019 → `policy.test.ts` (README matches) and Vitest (About shows it) |

## Project Structure

### Documentation (this feature)

```text
specs/010-license-notices/
├── plan.md              # This file
├── research.md          # Phase 0: the audit record (§1–§4) and decisions (§5–§16)
├── data-model.md        # Phase 1: program block, notices document, the licenses/ records
├── quickstart.md        # Phase 1: automated coverage and manual checks
├── contracts/
│   ├── notices.md             # commands, failures, notices.json, NOTICES.txt, licenses/, the Vite plugin
│   ├── ui.md                  # About HoploDex: ways in, layout, wording, keyboard, shots
│   └── tauri-commands.md      # no commands; the app:about-requested event and the macOS menu
├── checklists/requirements.md
└── tasks.md             # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
scripts/
├── check-npm-licenses.mjs            # imports policy, spdx, shipped from licenses/; devOptional is dev
└── licenses/                         # NEW
    ├── policy.mjs                    # ALLOWED, EXCEPTIONS scoped to packages or entries (OFL-1.1 fonts; blessing, CC-BY-4.0, ODC-By-1.0, public domain for hand-recorded entries), PREFERENCE
    ├── spdx.mjs                      # the SPDX evaluator (moved), choose()
    ├── shipped.mjs                   # npmShipped(lock), crateShipped(triple): one definition for audit and notices
    ├── licenseFiles.mjs              # finds and places license, NOTICE and attribution files
    ├── records.mjs                   # reads and checks hand-recorded, exceptions, clarify, native-code
    ├── notices.mjs                   # generator and CLI (--check, --all-targets, --out); notices.json and NOTICES.txt
    ├── vitePlugin.mjs                # hoplodexNotices(): define constants, bundled-package check, emit, dev serving
    ├── *.test.ts                     # policy, spdx, licenseFiles, records, notices, vitePlugin
    └── fixtures/                     # fake registry, node_modules, cargo tree outputs, red cases

licenses/                             # NEW, committed records (contracts/notices.md)
├── additional-permission.txt
├── hand-recorded.json
├── exceptions.json
├── clarify.json
├── native-code.json
└── texts/…                           # SQLite blessing, OpenSSL 3 LICENSE.txt, WebView2 SDK LICENSE.txt, word-list credits and attributions, exception texts, Big Shoulders OFL

vite.config.ts                        # hoplodexNotices() in plugins and worker.plugins
package.json                          # audit:licenses += notices --check --all-targets
README.md                             # NEW "License" section: copyright, GPL notice, the additional permission
.gitignore                            # unchanged (dist/ already ignored)

src/
├── vite-env.d.ts                     # __HOPLODEX_PROGRAM__; the zxcvbn per-list modules
├── components/strength/estimate.ts   # the dictionary from per-list modules, without wikipedia
├── components/strength/strength.test.ts
├── features/app/
│   ├── AboutDialog.tsx               # NEW
│   ├── AboutDialog.test.tsx          # NEW
│   ├── about.css                     # NEW: .hd-about__text and the program block
│   └── aboutNotices.ts               # NEW: fetch and validate notices.json, filter
├── features/session/
│   ├── SessionProvider.tsx           # about state, showAbout(), closed on phase change, onAboutRequested
│   └── sessionService.ts             # onAboutRequested
└── features/databases/
    ├── DatabaseChooser.tsx           # top bar button
    └── DatabaseMenu.tsx              # menu item

src-tauri/src/
├── main.rs                           # macOS: the menu and on_menu_event; ABOUT_REQUESTED
└── app_menu.rs                       # NEW, macOS: the default menu with the About item replaced; its test

e2e/
├── specs/databases.e2e.ts            # 010 US1 journey
└── screenshots/screens.e2e.ts        # contracts/ui.md §5

scripts/build-appimage.sh             # header: AppImages are built, not published (010 FR-017)
DEVELOPMENT.md                        # "License audit" rewritten; new "License notices" (FR-018)
CLAUDE.md                             # architecture: scripts/licenses, licenses/, About; AboutDialog in the UI list
```

**Structure Decision**: no new project or layer.
- **The build tooling** (`scripts/licenses/`) owns the notices. The interface
  only shows them, and the backend only adds the macOS menu item.
- **`AboutDialog` lives in `features/app/`**, not `features/databases/`,
  because it belongs to the whole app: it opens from the chooser, the open
  database and the native menu.
- **The amendments to earlier specs** are made at implementation, as one-line
  italic notes (CLAUDE.md "Spec Kit workflow"):
  - 003's dictionary exception (clarification of 2026-09-26 and research.md
    §18): the other lists are settled and `wikipedia.json` is dropped
    (010 research.md §2);
  - 003 FR-030, beside "About databases and security": About HoploDex is a
    separate screen.

**First tasks**, because later ones depend on them holding:
1. **Move the SPDX evaluator and allow list** into `scripts/licenses/`, with
   `check-npm-licenses.mjs` importing them and its output unchanged, except for
   the four `devOptional` packages that leave the count. Add the
   `ALLOWED` ≡ `deny.toml` test.
2. **Drop `wikipedia.json`** (research.md §2), so Story 2's notices start from
   the lists that ship. The worker-bundle check proves it's gone.
3. **The generator with its fixtures, red first**, for each failure rule.
   Then the records for the real trees on all three targets, until
   `--check --all-targets` passes on Linux. The Windows OpenSSL version
   reading is verified on the Windows VM, as a per-OS batch.
4. **Find the WebView2 SDK version and its LICENSE.txt** for
   `webview2-com-sys` 0.38.2, and **read the seven `woff2` files' copyright
   strings** (research.md §3, §12). If the WebView2 SDK's license turns out
   not to be BSD-3-Clause, stop and tell the owner before going on.
5. **Then the interface and the menu**, against a `notices.json` from a real
   build.

### Close-out (at pull request time)

The final phase of tasks.md, done as part of opening the pull request. Check
each issue's state first (`gh issue view <n> --json state`), and don't comment
on a closed one unless the owner asks.

- **#18**: the pull request closes it ("Fixes #18"). Comment item by item
  against its checklist:
  - the Rust notices by our own script, not `cargo about`, and why;
  - the npm notices from the audit's shipped set, checked against the bundle;
  - SQLCipher, OpenSSL 3, and also the WebView2 loader and the word lists'
    attributions, as hand-recorded entries with carrier checks;
  - one generated `notices.json` fetched by the app and produced by every
    `npm run build`;
  - `NOTICES.txt` ready for #19;
  - the About screen, and its shots in the walk.
- **#20**: the pull request closes it ("fixes #20"; one keyword per issue).
  Comment on:
  - where the audit record is (010 research.md §1);
  - the word lists' verdicts (§2), with `wikipedia.json` dropped;
  - the WebView2 loader's two-way settlement and the §7 permission;
  - the LGPL decision;
  - the new native-code guard and OpenSSL build check, which replace two
    manual checks.
- **#19** (release process): comment with what the written process must take
  from this feature:
  - each bundle carries `dist/NOTICES.txt` beside the program;
  - published Linux bundles are the Flatpak, `.deb` and `.rpm`, never an
    AppImage without a decision on its LGPL source;
  - the Windows build must have `OPENSSL_DIR` at OpenSSL 3, now enforced by
    the build;
  - the release review's manual license checks are those in DEVELOPMENT.md,
    including rechecking exceptions whose recorded version is behind.
- **#42** (Flatpak): comment that the Flatpak's LGPL libraries come from its
  Flathub runtime, which Flathub distributes with source (010 FR-017), and
  that the bundle should carry `NOTICES.txt` too.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|---|---|---|
| A notices generator of our own (`scripts/licenses/`, about six modules) rather than a ready-made tool | FR-010 forbids a silently omitted or generic notice. FR-008 needs the exact shipped set per target. The owner chose no new build tool (research.md §5) | `cargo-about` would have had to be installed on every build machine, falls back silently to the generic text, and needs a post-check against `cargo tree` anyway |
| Four kinds of committed record (`hand-recorded`, `exceptions`, `clarify`, `native-code`), each with its own check | Each covers a different blind spot of the tools, with a different reason to go stale: a carrier's version, a package that starts shipping its license, a changed file, new native code (data-model.md) | One free-form list would lose the per-kind checks, and with them FR-009's and FR-010's failures |
| A native menu on macOS, replacing Tauri's default About item | The spec's clarification: one About, carrying the GPL notices, never the system panel | Filling the system panel with `AboutMetadata` can't hold the third-party list or the full license |

## Post-Design Constitution Check

*Re-evaluated after Phase 1 (research.md, data-model.md, contracts/,
quickstart.md).*

- **Code Quality**: each rule has one home: what ships (`shipped.mjs`), which
  licenses are allowed (`policy.mjs`, tested against `deny.toml`), which file
  is which license (`licenseFiles.mjs`), and each kind of record's check
  (`records.mjs`). Still PASS.
- **Testing**: every scenario and criterion has a named test, and every
  failure rule has a red fixture (quickstart.md). Three manual procedures,
  none a gate. Still PASS.
- **UX Consistency**: one dialog built from shared parts. Every wording is in
  contracts/ui.md. Still PASS.
- **Performance**: as designed. The build-time cost is measured. Still PASS.
- **User Privacy**: nothing read and nothing sent. Still PASS.
- **Licensing**: met by the feature itself.

**Result**: PASS. The design adds no violations beyond those justified above.

## Design choices worth a look

The owner settled the spec's four clarifications and three questions at
planning (the generator, FR-015's grounds, the WebView2 loader). These were
decided at planning, inside what the spec allows:

- **The notices are a static file the dialog fetches** (`dist/notices.json`),
  not a backend command and not part of the start-up script (research.md
  §10). The program block is compiled in, so the legal statement never waits.
- **`devOptional` packages stop counting as shipped** in the audit too. Four
  type packages leave its count. Packages the bundler drops as unused code
  stay listed, since they are runtime dependencies (research.md §6).
- **A version bump alone doesn't fail an exception**, only a license change
  or the package gaining its own license file. Hand-recorded entries do fail
  on a version bump, as FR-009 asks (research.md §8).
- **The native-code guard** goes beyond FR-009's named list. It is how the
  WebView2 loader was found, and it turns DEVELOPMENT.md's manual "C code
  built inside a crate" check into an automated one (research.md §7).
- **The OpenSSL version is read from the Windows build's headers** and
  anything below 3 is refused. Today only the machine setup checks it
  (research.md §7).
- **Components the bundler drops still appear**, and a crate's choice of
  licenses shows the one used, with MIT first (research.md §5, §6).
- **About is a single scrolling dialog** with disclosures: the full license
  and each component's text open in place (research.md §11). It is not two
  pages, and not a list beside a detail pane.
- **The copyright years come from the build's commit**, so rebuilding a tag
  gives the same text (research.md §13).
- **The Big Shoulders notice uses the font's own copyright**, not the
  package's "Google Inc." line (research.md §12).
- **The §7 permission sits in `README.md` and `licenses/`**, and `LICENSE`
  stays the GPL's unmodified text. The SPDX identifier stays `GPL-3.0-only`
  (research.md §15).
- **The chooser's way in is a top-bar button** beside the theme toggle,
  because the chooser has no menu (contracts/ui.md §1).
