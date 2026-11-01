# Implementation Plan: Deprecated Dependencies and Yanked Crates in the Dependency Audit

**Branch**: `012-deprecated-dependencies` (worked on `fm/hoplodex-deprecated-deps`) | **Date**: 2026-11-01 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/012-deprecated-dependencies/spec.md`

## Summary

Three separate dependency rules join the audit, and the policy is written down in `DEVELOPMENT.md`:

- **Deprecated npm packages.** A new dependency-free script, `scripts/check-npm-deprecated.mjs` (`npm run audit:deprecated`, part of `npm run audit`), reads `package-lock.json`. It fails on every deprecated version that has no valid exception in `audit-deprecated.json`, and always on a shipped direct dependency. Exceptions are keyed by package, version range and scope, and expire on their review date (research.md §3–§5).
- **Unmaintained crates.** The audit is unchanged: `unmaintained = "all"`, with exceptions in `deny.toml`. The rule's new part is direct against indirect: no direct dependency without a stated special circumstance, and an indirect one names its parent. It is checked in review and before a release (research.md §9).
- **Yanked crates.** `yanked = "deny"` in `src-tauri/deny.toml`, and there is no exception path: a Vitest guard fails if `[advisories] ignore` holds anything but advisory IDs (research.md §6).

There are also two related pieces of work:

- Lint gains `@typescript-eslint/no-deprecated` with type-aware parsing, and the three current hits are justified where they are (research.md §7).
- The one current npm deprecation, `glob@10.5.0`, is cleared by updating WebdriverIO to 10.1 (research.md §2).

## Technical Context

**Language/Version**: Node.js 24 (ES modules, `.mjs`) for the check; TypeScript 5.9 for its tests; TOML configuration for cargo-deny 0.20.2.

**Primary Dependencies**: none new. Uses `scripts/licenses/shipped.mjs` (`npmPackages`), cargo-deny, and typescript-eslint 8.65 (installed).

**Storage**: N/A. It reads `package-lock.json`, `audit-deprecated.json` and `src-tauri/deny.toml`.

**Testing**: Vitest (`npm test`): fixture lockfiles and exceptions inline, as in `scripts/licenses/shipped.test.ts`, plus config guards for `deny.toml` and ESLint. E2E (`npm run test:e2e`) only to show that the WebdriverIO update still runs the suite.

**Target Platform**: the dev container (Linux) and the `Dependency audit` workflow (`ubuntu-latest`). The check is platform-independent Node.

**Project Type**: developer tooling inside the desktop app's repository.

**Performance Goals**: `audit:deprecated` under 2 s (FR-013, SC-005). Lint goes from about 3.7 s to about 12.4 s, which the owner accepted (research.md §7).

**Constraints**: offline, no new dependency, never reads the real databases (it doesn't touch the app at all).

**Scale/Scope**: about 700 lockfile entries; one script, three test files, and config and doc edits.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle / gate | Status |
|---|---|
| I. Code quality: YAGNI, no needless dependency | ✅ One script, reusing `npmPackages()`. A local range matcher instead of `semver` (research.md §4). No tooling for direct against indirect unmaintained crates (research.md §9). |
| II. Testing (NON-NEGOTIABLE) | ✅ Every scenario of Stories 1–3 is a Vitest case written to fail first. Story 4 has a config guard plus `npm run lint`. Story 5 is shown by `npm run audit` passing. No test touches real data. |
| III. UX consistency | ✅ N/A: no UI change. |
| IV. Performance | ✅ N/A for the app. The tooling budget is in Technical Context. |
| V. Privacy | ✅ N/A: no collection data is involved. |
| Security & Data Handling: the dependency gate | ✅ Stricter than the gate, which names vulnerabilities, and unmaintained and unsound notices. This feature adds project rules under it in DEVELOPMENT.md, and changes no constitution text (owner, 2026-11-01). The gate's "scoped to the dependency path" rule governs vulnerability exceptions. npm deprecation exceptions are keyed by package and range instead (Clarifications, Q5), and advisory exceptions are unchanged. |
| Licensing | ✅ No new dependency. The WebdriverIO update is development-only, so `audit:licenses` is unaffected. Rerun anyway. |
| Workflow gates: lint, tests, audit, review | ✅ All run in the dev container before the PR. No UI, so no screenshots. No persistence change. |

**Amendment**: none in this PR. A proposal for a separate PR is with the owner.

## Test Levels

No story changes the app, so there is no E2E journey. The E2E suite runs once to show that the WebdriverIO update didn't break it (FR-017).

| Story | E2E journey | Other E2E | Scenarios tested in Rust or Vitest |
|-------|-------------|-----------|------------------------------------|
| US1 | none (tooling) | none | US1-1 … US1-7 → Vitest, `scripts/check-npm-deprecated.test.ts` |
| US2 | none | none | US2-1 … US2-5 → Vitest, same file |
| US3 | none | none | US3-1 → `npm run audit:cargo` with `yanked = "deny"`, and a Vitest guard that the setting is `deny`. US3-2 → Vitest guard on `ignore` entries, `scripts/deny-config.test.ts` |
| US4 | none | none | US4-1, US4-2 → Vitest, `scripts/eslint-config.test.ts`: the rule is on as an error for a `.ts` and a `.tsx` file. US4-3 → `npm run lint` |
| US5 | none | the full E2E suite, once, after the WebdriverIO update | US5-1 … US5-4 → reading the DEVELOPMENT.md diff in review. US5-5 → `npm run audit` passes |

## Project Structure

### Documentation (this feature)

```text
specs/012-deprecated-dependencies/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/audit-commands.md
├── checklists/requirements.md
└── tasks.md            # /speckit-tasks
```

### Source Code (repository root)

```text
scripts/check-npm-deprecated.mjs       # new: checkDeprecated() + CLI (012 FR-003–FR-013)
scripts/check-npm-deprecated.test.ts   # new: Stories 1–2 fixtures
scripts/deny-config.test.ts            # new: yanked = "deny", no crate specs in ignore (FR-014)
scripts/eslint-config.test.ts          # new: no-deprecated is on (FR-018)
audit-deprecated.json                  # new: { "//": …, "exceptions": [] }
package.json                           # audit:deprecated, audit chain; @wdio/* and webdriverio ^10.1.0
package-lock.json                      # WebdriverIO 10.1 (glob 10.5.0 gone)
flatpak/node-sources.json              # regenerated for the lockfile
src-tauri/deny.toml                    # yanked = "deny" (already on the branch) and a header note on the three rules
eslint.config.js                       # projectService, no-deprecated
src/components/Combobox.tsx            # justified disable (keyCode 229)
src/components/ConfirmDialog.tsx       # justified disable (keyCode 229)
src/components/lock.ts                 # justified disable (navigator.platform)
DEVELOPMENT.md                         # Dependency audit: the three rules, exceptions, the before-a-release list; Linting: no-deprecated, clippy -D warnings
```

**Structure Decision**: the check sits beside `check-npm-licenses.mjs` and `check-no-webdriver.mjs` in `scripts/`, with its tests beside it, as `scripts/licenses/` does. `docs/architecture.md` covers the app, not the audit tooling, so it doesn't change. CLAUDE.md's text stays correct: "the dependency and license audit" still describes the gate.

### Close-out (at pull request time)

Check each issue's state first (`gh issue view <n> --json state`). Don't comment on a closed issue. This lane drafts the comments in pr-notes.md for firstmate rather than posting them.

- **#43**: the pull request closes it ("Fixes #43"). Its body summarises each of the five decisions as clarified, and lists what is left: archived-repository and Node.js end-of-life checks stay by hand, and the advisory exceptions' comment dates still aren't expired.
- **#19** (release process): draft a comment. The pre-release checks gained a "Dependency audit: before a release" list (deprecation and unmaintained-crate exceptions, archived repositories and successors, Node.js end of life) for the written release process to take up.

## Complexity Tracking

No violations.

## Post-Design Constitution Check

Unchanged from above. The design adds no dependency, no app code path and no persistence, and every rule has a test or an audit run that shows it.
