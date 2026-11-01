# Research: Deprecated Dependencies and Yanked Crates in the Dependency Audit

All findings below were checked on 2026-11-01, on the dev container (`scripts/dev-container.sh`), against the lockfiles at `8587ade`.

## §1 What the lockfiles hold today

- **Decision**: one npm deprecation needs clearing, and there are no yanked crates.
- **Findings**:
  - `package-lock.json` has one entry with a `deprecated` field: `node_modules/glob` at 10.5.0. It is `dev`, and its only dependent is `@wdio/config@10.0.1` (`glob ^10.2.2`).
  - The issue's other two, `mocha`'s nested `glob` and `inflight`, left with the WebdriverIO 10 move (#86). `mocha@12.0.3` now asks for `glob ^13.0.0`, which isn't deprecated.
  - With `yanked = "deny"`, `cargo deny check advisories` passes: no crate in `src-tauri/Cargo.lock` is yanked.
- **Rationale**: FR-014 and FR-017 depend on this.

## §2 Clearing `glob@10.5.0` (012 FR-017)

- **Decision**: update the WebdriverIO packages to 10.1.0 and raise their `package.json` minimums to `^10.1.0`, so a reinstall can't fall back to 10.0. The packages are `@wdio/cli`, `display-server`, `globals`, `local-runner`, `mocha-framework`, `spec-reporter` and `webdriverio`.
- **Findings**:
  - `@wdio/config@10.1.0` depends on `glob ^13.0.6`.
  - 10.1.0 was published 2026-10-18, so it is two weeks old.
  - The current ranges (`^10.0.x`) already allow it, so this is a minor update within the major the project is on.
- **Follow-ons**:
  - The lockfile changes, so `flatpak/node-sources.json` is regenerated with `scripts/dev-container.sh --flatpak npm run flatpak:sources` (DEVELOPMENT.md, "Flatpak").
  - E2E runs headless in the dev container on Linux. Windows and macOS aren't verified in this lane.
- **Alternatives considered**:
  - A dated exception: rejected, because a fix exists and the policy says to take it.
  - An `overrides` entry forcing `glob` 13 under `@wdio/config` 10.0: rejected. The upstream release already does that, and `glob` 13's API differs from 10's.

## §3 The npm check's shape

- **Decision**:
  - `scripts/check-npm-deprecated.mjs` exports a pure `checkDeprecated(lock, exceptions, today)` that returns the findings and failures.
  - It also runs as a CLI when it is the entry point. That is `scripts/citations.mjs`'s main guard: `process.argv[1]` resolved against `import.meta.url`.
  - The CLI reads `package-lock.json` and `audit-deprecated.json` from the repository root, prints a line per finding, and exits 1 on any failure.
  - It has no dependencies, like `check-npm-licenses.mjs`.
- **Scope**: `npmPackages()` from `scripts/licenses/shipped.mjs`, the license audit's definition (010 research.md §5, §6), so the two audits agree (012 FR-004).
- **Direct dependency**:
  - The lockfile's root entry (`packages[""]`) lists the app's `dependencies` and `devDependencies`.
  - A finding at `node_modules/<name>` whose name is in `dependencies` is a **shipped direct dependency** (FR-005). One whose name is in `devDependencies` is a development-only direct one.
  - Any other location is transitive, nested copies included.
  - `package.json` isn't read, because the lockfile's root entry mirrors it.
- **Today's date**: the UTC calendar date, so that CI and a laptop in another timezone agree on when an exception expires. The CLI computes it, and the tests pass a fixed date in.
- **Rationale**: a pure function makes the fixture tests cheap, with no files and no child process. The CLI stays a few lines.
- **Alternatives considered**:
  - A separate module with a thin CLI: rejected, because one file is enough.
  - Reading `node_modules`: rejected, because the lockfile records the flag and is what CI has before installing.

## §4 Version ranges without a dependency

- **Decision**: a small matcher in the script for the subset of npm range syntax an exception needs:
  - an exact version;
  - `>`, `>=`, `<` and `<=`;
  - `^` and `~`;
  - `*`, or an empty range;
  - comparators joined by spaces (AND) and `||` (OR).

  Anything else fails as an unreadable range (FR-011). A prerelease version is compared on its `major.minor.patch` only. That shortcut is commented in the code: deprecated prereleases are rare, and it is upgraded if one appears.
- **Rationale**: FR-013 rules out a dependency. The audit mustn't grow the tree it audits, which is `check-npm-licenses.mjs`'s rule, and importing `semver` from wherever it happens to sit in `node_modules` would break when a parent drops it.
- **Alternatives considered**:
  - Exact versions only: rejected. The issue asks for ranges, and its test list includes "an exception for a different version range doesn't apply".

## §5 The exceptions file

- **Decision**:
  - The file is `audit-deprecated.json` at the repository root, next to `audit-ci.jsonc`.
  - It is plain JSON (Node's `JSON.parse`), with a `"//"` key that points to DEVELOPMENT.md, and an `exceptions` array. Each entry has exactly the fields FR-007 lists (data-model.md).
  - An unknown key fails the audit, so a misspelt field can't silently do nothing.
- **Rationale**: every field can be read and checked, and there's no comment parsing. The policy prose lives in DEVELOPMENT.md (owner's answer, Clarifications 2026-11-01).
- **Alternatives considered**: JSONC like `audit-ci.jsonc`. Rejected: it would need comment stripping in a dependency-free script for no gain, since the reason is a field.

## §6 Yanked crates (012 FR-014)

- **Decision**:
  - `yanked = "deny"` in `src-tauri/deny.toml` `[advisories]`, with a comment saying that a yanked crate is never excepted.
  - A Vitest test reads `deny.toml` and fails if `yanked` isn't `"deny"`, or if `[advisories] ignore` holds anything but advisory IDs. cargo-deny takes a `crate = "name@version"` entry (or a bare crate spec) there to let a yanked crate through, so the test accepts only `{ id = "RUSTSEC-…" | "GHSA-…" | "CVE-…", reason = "…" }` tables and bare ID strings.
- **Findings**:
  - cargo-deny 0.20.2 (the Dockerfile's and CI's version) checks yanked crates against the registry index, with the advisory check.
  - Locally, it reads the cached index metadata; CI fetches it fresh.
  - The check covers every crate in the graph, build and proc-macro crates included, since `[graph] all-features = true` and no target filter apply. That makes it shipped or build-only, as the owner asked.
- **Rationale**: cargo-deny can't be told "no exceptions" for yanked crates, so the test is the guard. A Vitest test with a regular-expression read of one TOML array is the cheapest level that shows it. There is no TOML dependency in the npm tree to lean on, and a Rust test would need a `toml` dev-dependency.
- **Alternatives considered**: a review-only rule. Rejected, because the owner wants no yanked crate to be usable.

## §7 Deprecated APIs in the project's own code (012 FR-018)

- **Decision**:
  - Turn on `@typescript-eslint/no-deprecated` as an error for `**/*.{ts,tsx}`, with type-aware parsing (`parserOptions.projectService`, `tsconfigRootDir`).
  - Files outside every `tsconfig` are linted through `allowDefaultProject`: `vite.config.ts`, `tools/*/vite.config.ts`, `scripts/*.ts` and `scripts/*/*.ts`. The first two are in `tsconfig.node.json`, which no root config references. The `scripts` tests are in no config. The default project's file limit is raised to cover them, which is 19 files today.
- **Findings** (spike, 2026-11-01):
  - typescript-eslint 8.65.0 (already installed) has the rule.
  - Lint time in the dev container went from about 3.7 s to about 12.4 s, two runs each, 24 CPUs. That cost is accepted with the owner's choice (Clarifications, Q14).
  - The rule found three hits:
    - `src/components/Combobox.tsx:167` and `src/components/ConfirmDialog.tsx:109`: `event.keyCode === 229`. This is the One Enter rule's IME guard (#44): Safari reports `isComposing` false on the keydown that ends a composition, but sets keyCode 229. There is no standard replacement, so it stays, with a disable comment and that reason.
    - `src/components/lock.ts:5`: `navigator.platform`, used to show "⌘L" on a Mac. Its replacement, `navigator.userAgentData`, isn't in WebKit (WKWebView, WebKitGTK), so it stays, with a disable comment and that reason.
- **Test**: a Vitest test asks ESLint for the configuration it uses for a source file (`calculateConfigForFile`) and checks that the rule is on as an error. The rule's behavior is typescript-eslint's. What this project owns is that the rule is on, and `npm run lint` (with `--max-warnings 0`) passing shows the hits are handled.
- **Rust**: rustc's `deprecated` lint warns, and both clippy commands in DEVELOPMENT.md ("Linting & formatting") pass `-D warnings`, so a deprecated Rust API already fails lint. The policy records this, and nothing changes.
- **Alternatives considered**: `eslint-plugin-deprecation`. Rejected: it is superseded by this rule and would be a new dependency.

## §8 Hand checks and the release list

- **Decision**: the hand checks before a release join the "Dependency audit" section as a "Before a release" list, mirroring the license audit's hand checks:
  - every open npm deprecation exception in `audit-deprecated.json`, and every unmaintained-crate `ignore` entry in `deny.toml`, checking for each whether the crate is direct or indirect (012 FR-019);
  - direct dependencies with an archived repository, or one that names a successor;
  - the pinned Node.js version (`NODE_VERSION` in `Dockerfile`), and the npm it brings, against Node's release schedule.
- **Rationale**: there is no written release process yet (#19). The license audit already keeps its pre-release checks in its own section, and #19 collects them from there.

## §9 Unmaintained crates: direct or indirect (012 FR-019)

- **Decision**: a policy and review rule only, with no new tooling.
  - Each unmaintained-crate exception's comment in `deny.toml` says whether the crate is direct or indirect.
  - An indirect one names its parent.
  - A direct one may be excepted only in a special circumstance, which the comment states.
- **Findings**: the nine unmaintained entries at `8587ade` are all indirect, and each names its parent:
  - `proc-macro-error`, via gtk-rs 0.18;
  - five `unic-*` crates, via urlpattern in tauri-utils;
  - `zvariant`, `zbus_names` and `zvariant_derive`, via zbus 5, which secret-service and tauri-plugin-opener require.
  
  None needs its comment changed.

  _Amended 2026-11-01: that was wrong. Six of the nine comments had no review date, and none said "indirect" (the parent was only in the `reason`), which the constitution and FR-019 require. Each comment now says "Indirect, via <parent>." and has a "Review by 2027-01-31, or …" line; the entries are unchanged._
- **Why no tooling**: the `ignore` entries name advisory IDs, not crates. Telling direct from indirect would mean reading the RustSec database to map an ID to its crate, and then `src-tauri/Cargo.toml`. That is real machinery, and the owner asked for it only if it was cheap.
- **Alternatives considered**: a test that each reason names a parent (`via …`). Rejected: without the advisory database it can't tell an unmaintained entry from a vulnerability one, so it would police the wrong entries.

## §10 Not taken up

- **Archived-repository lookups**: they need GitHub's API and a token, and the issue keeps network checks out unless they are cheap. They are a hand check (§8).
- **Expiring the advisory exceptions** in `audit-ci.jsonc` and `deny.toml`: out of scope (owner's answer to Q8). Those dates are in comments.
- **A grace period**: none (Q12). The exception is the grace period, where one is allowed.
