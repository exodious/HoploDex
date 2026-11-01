---

description: "Task list for feature 012: deprecated dependencies and yanked crates in the dependency audit"
---

# Tasks: Deprecated Dependencies and Yanked Crates in the Dependency Audit

**Input**: Design documents from `/specs/012-deprecated-dependencies/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/audit-commands.md, quickstart.md

**Tests**: Constitution II is NON-NEGOTIABLE. Each story's tests are written first and fail, then the implementation makes them pass, at the level plan.md's Test Levels gives. No story changes the app, so there is no E2E journey. The E2E suite runs only to check the WebdriverIO update (US5).

**Organization**: Tasks are grouped by user story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependencies)
- **[Story]**: US1–US5 from spec.md
- In this file, bare IDs mean this feature's. In code and test names, cite them as `012 FR-005`, `012 US1-3` or `012 research.md §4`; `npm test` fails on a bare one.
- Run every test, lint, audit and build through `scripts/dev-container.sh` (CLAUDE.md).

---

## Phase 1: Setup

**Purpose**: wiring that every story's command needs.

- [X] T001 Add `"audit:deprecated": "node scripts/check-npm-deprecated.mjs"` to package.json `scripts`, and insert `npm run audit:deprecated` into the `audit` chain between `audit:npm` and `audit:cargo` (contracts/audit-commands.md).
- [X] T002 [P] Create audit-deprecated.json at the repository root, as `{ "//": "Deprecated npm packages kept for now. See \"Dependency audit\" in DEVELOPMENT.md.", "exceptions": [] }`, formatted like the repository's other JSON (two-space indent, final newline).

---

## Phase 2: Foundational

**Purpose**: the module skeleton the US1 and US2 tests import.

- [X] T003 Create scripts/check-npm-deprecated.mjs, modelled on scripts/check-npm-licenses.mjs, with these parts:
  - A header comment: what it checks, a pointer to "Dependency audit" in DEVELOPMENT.md, no dependencies of its own, and the scope definition from `scripts/licenses/shipped.mjs` (010 research.md §5, §6).
  - An exported `checkDeprecated(lock, exceptionsFile, today)` that returns `{ findings: [], problems: [] }` for now.
  - An exported `satisfiesRange(version, range)` stub.
  - A CLI main guard, as in scripts/citations.mjs (`process.argv[1]` resolved and compared to `fileURLToPath(import.meta.url)`). It reads `package-lock.json` and `audit-deprecated.json` from the repository root (a missing exceptions file counts as `{ exceptions: [] }`), passes `new Date().toISOString().slice(0, 10)` as today, prints per contracts/audit-commands.md, and exits 1 when `problems` is non-empty.

**Checkpoint**: `npm run audit:deprecated` runs and passes trivially.

---

## Phase 3: User Story 1 - The Audit Stops a Deprecated npm Package That Has No Decision (Priority: P1) 🎯 MVP

**Goal**: every deprecated lockfile location is reported. Those without a valid exception fail, and a shipped direct dependency always fails.

**Independent Test**: the US1 Vitest cases pass against inline fixture lockfiles.

### Tests for User Story 1 (mandatory per constitution)

- [X] T004 [P] [US1] Write the US1 tests in scripts/check-npm-deprecated.test.ts. Inline fixture lockfiles follow scripts/licenses/shipped.test.ts: a root entry with `dependencies` and `devDependencies`, and `dev`/`devOptional` flags. `today = "2026-11-01"`. Cases:
  - `012 US1-1`: a shipped transitive deprecated package with no exception fails, and the problem names `name@version`, the location and "ships".
  - `012 US1-2`: a dev-only deprecated package with no exception fails, and the problem says development-only.
  - `012 US1-3`: a matching exception (same package, version in range, same scope, review ≥ today) makes the finding `excepted`, and there are no problems.
  - `012 US1-4`: an exception whose range doesn't cover the version doesn't apply.
  - `012 US1-5`: a shipped direct dependency (`node_modules/<name>` with `<name>` in the root's `dependencies`) fails even with a matching exception, and the message says to replace it.
  - `012 US1-6`: a nested copy (`node_modules/a/node_modules/glob`) and a top-level copy are reported separately, each judged on its own version.
  - `012 US1-7`: no deprecations and no exceptions gives no findings and no problems.
  - An empty `deprecated` string is not a finding.
  - A `devOptional` package counts as dev (012 FR-004).
  - A dev direct dependency (in `devDependencies`) with a valid exception is excepted (012 FR-006).
- [X] T005 [P] [US1] Write the `satisfiesRange` tests in scripts/check-npm-deprecated.test.ts (012 research.md §4):
  - exact `10.5.0`;
  - `>=10 <11`, `>=10.0.0 <11.0.0`, `<=1.2.3`, `>1`;
  - `^10.2.2` (and `^0.2.3`, which stays below `0.3.0`), `~1.2.3`;
  - `*` and the empty string;
  - `1.x || >=3`;
  - a prerelease `10.5.0-rc.1` compared on `10.5.0`;
  - unreadable ranges (`latest`, `>=a`, `1.2.3 - 2`) throw.

### Implementation for User Story 1

- [X] T006 [US1] Implement `satisfiesRange` in scripts/check-npm-deprecated.mjs. It supports exact, `=`, `>`, `>=`, `<`, `<=`, `^`, `~`, `*`/empty, partial versions (`10`, `10.2`, `1.x`), space-joined AND and `||` OR. It throws on anything else. Add the comment `// shortcut: a prerelease is compared on major.minor.patch only; upgrade if a deprecated prerelease ever needs its own exception` (012 research.md §4).
- [X] T007 [US1] Implement finding detection and outcomes in `checkDeprecated`, per data-model.md "Rules, in order":
  - Finding detection: use `npmPackages(lock)` from scripts/licenses/shipped.mjs for name, version, location and `development`. Report entries whose `deprecated` is a non-empty string. Compute `direct` from `lock.packages[""].dependencies` / `devDependencies` at location `node_modules/<name>`.
  - Rule 1: a shipped direct dependency fails.
  - Rule 2: otherwise, the first exception with the same `package`, version within `range` and the same `scope`.
  - Rule 3: otherwise, the finding fails.
  - Messages follow contracts/audit-commands.md.
- [X] T008 [US1] Implement the CLI's report in scripts/check-npm-deprecated.mjs: one line per finding (`<scope>  <name>@<version>  <location>  <outcome>`, the message indented below), the problems on stderr under a count line, and `npm deprecations ok: …` on success. Run `scripts/dev-container.sh npx vitest run scripts/check-npm-deprecated.test.ts`.

**Checkpoint**: the US1 tests pass.

---

## Phase 4: User Story 2 - Exceptions Come Back When Their Review Date Passes (Priority: P1)

**Goal**: expired, stale and malformed exceptions fail the audit.

**Independent Test**: the US2 Vitest cases pass.

### Tests for User Story 2 (mandatory per constitution)

- [X] T009 [US2] Add the US2 tests to scripts/check-npm-deprecated.test.ts:
  - `012 US2-1`: `review: "2026-10-31"` with today `2026-11-01` fails, naming the exception's index, package and date. The finding isn't reported as stale.
  - `012 US2-2`: `review` equal to today applies.
  - `012 US2-3`: an exception matching no finding fails with "remove the exception".
  - `012 US2-4`: each of these fails, naming the index and field:
    - a missing `package`, `range`, `scope`, `reason`, `via`, `successor` or `review`;
    - `scope: "prod"`;
    - `review: "2026-02-30"` or `"31/12/2026"`;
    - `range: "latest"`;
    - an empty `reason` or `via`;
    - an unknown key `reviewed`;
    - a top-level key other than `"//"` and `exceptions`;
    - `exceptions` not an array.

    `successor: null` is accepted.
  - `012 US2-5`: a `scope: "dev"` exception doesn't cover the same package when it ships, so that finding fails as shipped.

### Implementation for User Story 2

- [X] T010 [US2] Implement exception validation in `checkDeprecated` (scripts/check-npm-deprecated.mjs), per data-model.md's field table:
  - `package` non-empty string;
  - `range` readable by `satisfiesRange`;
  - `scope` is `"shipped"` or `"dev"`;
  - `reason` non-empty;
  - `via` non-empty;
  - `successor` a string or `null`;
  - `review` a real calendar date matching `YYYY-MM-DD` (round-trip it through `Date` in UTC);
  - no other keys, and only `"//"` and `exceptions` at the top level.

  An invalid exception never matches.
- [X] T011 [US2] Implement expiry (`review < today`, compared as strings, which works because both are ISO dates) and staleness (a valid exception that matches no finding) in `checkDeprecated`, with the messages from contracts/audit-commands.md. Run `scripts/dev-container.sh npx vitest run scripts/check-npm-deprecated.test.ts`.

**Checkpoint**: Stories 1 and 2 pass. `scripts/dev-container.sh npm run audit:deprecated` on the real lockfile shows `glob@10.5.0` (dev) failing. That is expected until T018.

---

## Phase 5: User Story 3 - A Yanked Rust Crate Always Fails the Audit (Priority: P2)

**Goal**: yanked crates are denied, and nothing can except one.

**Independent Test**: scripts/deny-config.test.ts passes, and `npm run audit:cargo` passes with `yanked = "deny"`.

### Tests for User Story 3 (mandatory per constitution)

- [X] T012 [P] [US3] Write scripts/deny-config.test.ts (012 research.md §6). It reads src-tauri/deny.toml as text and takes the `[advisories]` table up to the next `[` header. Cases:
  - `012 US3-1`: the table sets `yanked = "deny"`.
  - `012 US3-2`: every entry of its `ignore = [ … ]` array is either an inline table that has `id = "RUSTSEC-…"`, `"GHSA-…"` or `"CVE-…"` and no `crate` key, or a bare string matching `^(RUSTSEC|GHSA|CVE)-`. Strip `#` comments before parsing.
  - The guard itself: a fixture string with `{ crate = "foo@1.0.0", reason = "x" }`, and one with a bare `"foo@1.0.0"`, are both rejected. To make that testable, export the checking function from the test's own small helper, or keep it in the test file.
  - Red step: `yanked = "deny"` landed before this test (bbc2a04). So run the guard once with that line removed from a copy of the file's text, and confirm that `US3-1` fails (constitution II).

### Implementation for User Story 3

- [X] T013 [US3] Confirm that src-tauri/deny.toml has `yanked = "deny"` with its comment (already on the branch at bbc2a04). Extend the file's header comment so it states the three Rust rules separately:
  - Advisories, as today.
  - Unmaintained crates: say whether the crate is direct or indirect. An indirect one names its parent. A direct one is only in a special circumstance, which is stated (012 FR-019).
  - Yanked crates: always fail, never excepted (012 FR-014).

  Run `scripts/dev-container.sh npx vitest run scripts/deny-config.test.ts` and `scripts/dev-container.sh npm run audit:cargo`.

**Checkpoint**: the US3 tests and `audit:cargo` pass.

---

## Phase 6: User Story 5 - The Policy Is Written Down and the Current Findings Are Cleared (Priority: P2)

**Goal**: DEVELOPMENT.md has the three rules, the exception format and the before-a-release list, and the audit passes on the branch.

**Independent Test**: `npm run audit` passes, and the DEVELOPMENT.md diff answers the issue's five decisions, with the rules in separate subsections.

### Tests for User Story 5 (mandatory per constitution)

- [X] T014 [US5] `012 US5-5` is shown by `scripts/dev-container.sh npm run audit:deprecated` passing on the real lockfile once T018 is done. Before T018 it fails on `glob@10.5.0`, which is the red step. Record both runs in the commit message.

### Implementation for User Story 5

- [X] T015 [US5] Rewrite "Dependency audit" in DEVELOPMENT.md (012 FR-015, FR-016, FR-019):
  - Add `npm run audit:deprecated` to the command list, and change "all four" to "all five".
  - "What fails the audit": add an npm bullet for deprecated versions without a valid exception (always, for a shipped direct dependency). Add a separate Rust bullet for yanked crates (always, never excepted).
  - A new `### Deprecated npm packages` subsection covers:
    - what counts (FR-001, FR-002);
    - scope, from the license audit's definition;
    - direct against transitive;
    - the rule for adding a dependency (FR-012);
    - the `audit-deprecated.json` format, with the data-model.md field table and an example;
    - expiry on the review date, and staleness;
    - "fix it in its own commit".
  - A new `### Unmaintained crates` subsection: direct against indirect, and what the comment says (FR-019).
  - A new `### Yanked crates` subsection: always fails, shipped or build-only, never excepted. Update the crate, or move off the parent that pins it.
  - A `### Before a release` list (research.md §8):
    - review every npm deprecation exception and unmaintained-crate exception;
    - check direct dependencies for archived repositories or a named successor;
    - check `NODE_VERSION` in Dockerfile against Node's release schedule.

  Keep the three rules apart: no sentence groups yanked crates with deprecations.
- [X] T016 [P] [US5] Extend "Linting & formatting" in DEVELOPMENT.md:
  - ESLint's `@typescript-eslint/no-deprecated` reads the TypeScript types, so lint takes about 12 s instead of about 4 s.
  - How to justify a use: `// eslint-disable-next-line @typescript-eslint/no-deprecated -- <reason>`.
  - Rust's `deprecated` warnings already fail through clippy's `-D warnings` (012 FR-018, research.md §7).
- [X] T017 [P] [US5] Update the header comment of audit-ci.jsonc only if it now reads as the whole npm policy: point to the deprecation check (`npm run audit:deprecated`, audit-deprecated.json) in one line.
- [X] T018 [US5] Clear `glob@10.5.0` (012 FR-017, research.md §2):
  - In package.json, raise `@wdio/cli`, `@wdio/display-server`, `@wdio/globals`, `@wdio/local-runner`, `@wdio/mocha-framework`, `@wdio/spec-reporter` and `webdriverio` to `^10.1.0`.
  - Run `scripts/dev-container.sh npm install`.
  - Confirm that `package-lock.json` has no `deprecated` entry, and that `node_modules/glob` is gone or no longer deprecated.
- [X] T019 [US5] Regenerate the Flatpak's npm sources for the new lockfile: `scripts/dev-container.sh --flatpak npm run flatpak:sources`. Commit flatpak/node-sources.json.
- [X] T020 [US5] On Linux, run `scripts/dev-container.sh bash -c 'npm run build && npm run test:e2e'` after T018. Expected: the suite passes.
- [ ] T021 [P] [US5] On Windows, run the E2E suite after T018. Not verified on Windows in this lane: leave it unchecked and list it in the PR.
- [ ] T022 [P] [US5] On macOS, run the E2E suite after T018. Not verified on macOS in this lane: leave it unchecked and list it in the PR.

**Checkpoint**: `scripts/dev-container.sh npm run audit` passes.

---

## Phase 7: User Story 4 - Lint Stops New Uses of Deprecated APIs (Priority: P3)

**Goal**: `@typescript-eslint/no-deprecated` is on, and each existing hit is justified.

**Independent Test**: scripts/eslint-config.test.ts passes, and `npm run lint` passes.

### Tests for User Story 4 (mandatory per constitution)

- [X] T023 [P] [US4] Write scripts/eslint-config.test.ts, with `// @vitest-environment node` and a header citing 012 FR-018. `012 US4-1`, `US4-2`: `new ESLint({ cwd: repoRoot }).calculateConfigForFile()` for `src/App.tsx` and for `src/components/lock.ts` gives `rules["@typescript-eslint/no-deprecated"]` with severity 2 (`"error"`), and `languageOptions.parserOptions.projectService` is set. Fails first, before T024.

### Implementation for User Story 4

- [X] T024 [US4] In eslint.config.js's `**/*.{ts,tsx}` block, add:
  ```js
  parserOptions: {
    projectService: {
      allowDefaultProject: ["vite.config.ts", "tools/*/vite.config.ts", "scripts/*.ts", "scripts/*/*.ts"],
      maximumDefaultProjectFileMatchCount_THIS_WILL_SLOW_DOWN_LINTING: 50,
    },
    tsconfigRootDir: import.meta.dirname,
  }
  ```
  Add the rule `"@typescript-eslint/no-deprecated": "error"`, with a one-line comment on why the default project covers those files (012 research.md §7).
- [X] T025 [US4] Justify the three hits with `// eslint-disable-next-line @typescript-eslint/no-deprecated -- <reason>`:
  - src/components/Combobox.tsx and src/components/ConfirmDialog.tsx (`keyCode === 229`): "Safari ends an IME composition with isComposing false but keyCode 229; there is no standard replacement (#44)".
  - src/components/lock.ts (`navigator.platform`): "navigator.userAgentData, its replacement, isn't in WebKit".

  Run `scripts/dev-container.sh npm run lint` (`012 US4-3`) and `scripts/dev-container.sh npx vitest run scripts/eslint-config.test.ts`.

**Checkpoint**: lint and the US4 test pass.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [X] T026 Run the full gates in the dev container:
  - `scripts/dev-container.sh bash -c 'npm run format:check && npm run lint && npm test'`;
  - `scripts/dev-container.sh npm run audit`;
  - the clippy commands from DEVELOPMENT.md "Linting & formatting".

  `npm test` includes scripts/citations.mjs, so every `012 …` citation must exist in spec 012.
- [X] T027 Time `scripts/dev-container.sh bash -c 'time npm run audit:deprecated'` (SC-005, under 2 s) and lint (record it in the PR's performance note).
- [X] T028 Walk quickstart.md's automated steps, and its manual step 2 in a scratch copy (never on `main`).
- [X] T029 Check that CLAUDE.md, docs/architecture.md and .github/workflows/audit.yml need no change (plan.md "Structure Decision"). Edit CLAUDE.md only if this change made its text wrong.

### Close-out (at pull request time)

- [X] T030 Check the issues' state (`gh issue view 43 --json state`, `gh issue view 19 --json state`). In pr-notes.md, draft "Fixes #43", with a summary of the five decisions as clarified and what is left: archived-repository and Node.js end-of-life checks are by hand, and the advisory exceptions' comment dates aren't expired. Also draft the comment for #19 (the new "Before a release" list) for firstmate to post. Don't comment on any issue from this lane.

---

## Dependencies & Execution Order

- Setup (T001–T002) → Foundational (T003) → US1 (T004–T008) → US2 (T009–T011).
- US2 extends the same file as US1, so it runs after it.
- US3 (T012–T013) is independent of US1 and US2, and can run any time after Setup.
- US5:
  - T014 is the red/green bracket around T018.
  - T015–T017 are docs, independent.
  - T018 → T019 → T020.
  - T021 and T022 are per-OS and not done in this lane.
- US4 (T023–T025) is independent of the others. Do it after US5's T018, so a lint run sees the final lockfile.
- Polish (T026–T029) comes after every story. Close-out (T030) comes last.

## Parallel Opportunities

- T002 can run beside T001.
- T004 and T005 can be written together.
- US3 (T012), US4 (T023) and the docs tasks (T016, T017) touch separate files, so they can run beside US1 and US2.

## Implementation Strategy

- MVP: US1 and US2, which together are the npm deprecation check (Phases 1–4).
- Then US3 (the yanked-crate guard) and US5 (policy text and clearing glob), so the audit passes.
- Then US4 (lint), and the polish phase.

---

## Phase 9: Convergence

**Lead's decisions, 2026-11-01** (none goes to the owner: each is an implementation choice, or follows a rule the owner already set):

- The unmaintained entries' missing review dates are an older gap. Fix them here: the constitution requires a review date on every exception, and FR-019 requires "direct" or "indirect" in each comment. The comments change; the entries don't.
- Two exceptions that match one finding: every matching exception counts as matched. The finding is excepted if any of them hasn't expired, and an expired one is still reported as expired.
- US4's Independent Test asks for linting a sample with a deprecated use. Add that test if ESLint's `lintText` works with the project service; otherwise keep the config-only test and record the reason in research.md §7. Enforcing the `-- reason` text is left to review, because a plugin for it would be a new dependency.
- The two commits on `main` since the branch (including the vitest bump) are left to the pipeline's rebase, per the lane brief.

- [X] T031 [US2] Read every comparator of every `||` alternative when validating a range, so that `"<11 || garbage"` and `"* || latest"` are unreadable (FR-011). Add both to the US2-4 tests. Source: converge S1-F1, partial, MEDIUM. Files: scripts/check-npm-deprecated.mjs, scripts/check-npm-deprecated.test.ts.
- [X] T032 [US2] Implement the multiple-match rule above: mark every matching valid exception `matched`, and except the finding when any match hasn't expired. Add a test with two matching exceptions, one expired and one not: the finding is excepted, the expired one is reported, and neither is called stale. Source: converge S1-D1. Files: scripts/check-npm-deprecated.mjs, scripts/check-npm-deprecated.test.ts.
- [X] T033 [P] [US1] Tighten the US1 tests:
  - US1-1 asserts that the problem includes the deprecation message;
  - US1-4 asserts "development-only";
  - add US2-4 cases for an exception that isn't an object, and a file that isn't an object.

  Source: converge S1-F2–F4, partial, LOW. File: scripts/check-npm-deprecated.test.ts.
- [X] T034 [US5] In src-tauri/deny.toml, add to each unmaintained entry's comment: "Indirect, via <parent>." and "Review by 2027-01-31, or …" (the existing "Goes when …" condition).
  - proc-macro-error: via gtk-rs 0.18's macros in Tauri.
  - the five unic-* crates: via urlpattern in tauri-utils.
  - zvariant, zbus_names, zvariant_derive: via zbus 5, required by secret-service and tauri-plugin-opener. Their comment already has a review date.

  Also reword the header's "Three separate rules, each with its own exception form", because yanked crates have none. Then update research.md §9's finding ("none needs its comment changed") with an `_Amended 2026-11-01:_` note. Source: converge S2-F1 (CRITICAL by the constitution) and S2-F5, LOW.
- [X] T035 [US3] Extend scripts/deny-config.test.ts:
  - fail if deny.toml sets `exclude` or `exclude-dev` anywhere, or drops `[graph] all-features = true`, since either would hide a yanked crate from the check;
  - give the yanked assertions the message "a yanked crate is updated, never excepted (012 FR-014)";
  - rename scripts/eslint-config.test.ts's describe to `012 US4 - Lint Stops New Uses of Deprecated APIs`.

  Source: converge S2-F2, F3, F9.
- [X] T036 [US4] If ESLint's `lintText` with `filePath` under `scripts/` works with the project service, add to scripts/eslint-config.test.ts:
  - a snippet using a deprecated API (for example a `/** @deprecated */` function, declared and called in the snippet) gives a `@typescript-eslint/no-deprecated` error (US4-1);
  - the same snippet with the justified disable comment gives none (US4-2).

  Otherwise, record why in research.md §7. Source: converge S2-F8.
- [X] T037 [P] [US5] Fix the docs:
  - In DEVELOPMENT.md "Dependency audit", limit "To fix a finding … Add an exception …" to advisories ("To fix an advisory"), so it no longer reads as covering deprecations, unmaintained or yanked crates.
  - Add to "Yanked crates" that a crate yanked after it entered `Cargo.lock` fails a branch that changed nothing, and is updated in its own commit.
  - In docs/architecture.md's `scripts/licenses/` paragraph, note that `check-npm-deprecated.mjs` (012) uses `shipped.mjs`'s npm definition too.
  - Correct plan.md's Structure Decision with an `_Amended 2026-11-01:_` note.

  Source: converge S2-F4, F6, F10.
- [X] T038 Rerun T026's gates (the dev container), then finish T027–T029.
