# Feature Specification: Deprecated Dependencies and Yanked Crates in the Dependency Audit

**Feature Branch**: `012-deprecated-dependencies`

**Created**: 2026-10-31

**Status**: Draft

**Input**: User description: GitHub issue #43, "Dependency audit: a rule and check for deprecated dependencies". The dependency audit catches known vulnerabilities, plus unmaintained and unsound notices for Rust crates, but has no rule for deprecated dependencies. This feature writes that rule down and makes `npm run audit` enforce as much of it as is practical. A deprecated dependency is then either replaced, or carries a recorded, dated exception the way an advisory exception does. A yanked crate version is a separate, harder rule: it always fails the audit.

## Clarifications

### Session 2026-11-01

The owner answered the batched decisions: "Q14A, others recommended, wait a moment on the Q17 constitution. This probably applies to the other questions here - I don't like the confluence between a deprecated dependency and a yanked crate. If a cargo crate has been yanked, that is an exceptional circumstance and it means something is very wrong. We should not be using yanked crates."

- Q: Which signals count as deprecated? → A: npm's deprecation flag on a resolved version is enforced by the audit. An archived upstream repository, or a direct dependency that names a successor, is a hand check before each release. **A yanked crate is not a kind of deprecation**: it is its own failure (FR-001, FR-014). The recommended answer also counted a RustSec unmaintained notice. The owner's later answer below makes that its own rule too (FR-019).
- Q: Does a deprecation of an old major version HoploDex doesn't resolve to count? → A: No. Only the resolved version counts (FR-002).
- Q: Can a deprecated npm package that ships in the app carry an exception? → A: Only a transitive one. A shipped direct dependency that is deprecated must be replaced (FR-005).
- Q: What does a deprecated development-only npm package need? → A: A dated exception, whether it is direct or transitive. Without one, it fails the audit (FR-006).
- Q: Are exceptions keyed by package and version range, or by dependency path? → A: By package name and version range, plus the scope they were recorded for. A development-only exception never covers a copy that ships. The package that brings it in is recorded for the reviewer, but isn't matched on (FR-008).
- Q: Is adding a deprecated dependency refused outright? → A: Yes for a direct dependency that ships, and the check enforces it, because it never accepts an exception for one. A development-only direct dependency, and a transitive one arriving with a new dependency, follow the exception rule (FR-012).
- Q: Where do the npm deprecation exceptions live? → A: In a small data file next to the audit configuration, with the fields FR-007 lists. The check reads it.
- Q: Should the check fail on an expired review date? → A: Yes, for the npm deprecation exceptions it reads. A date in the past fails, and the date itself still passes. The existing advisory exceptions in `audit-ci.jsonc` and `deny.toml` keep their manual review (FR-009).
- Q: Does a stale exception fail? → A: Yes, with "remove the exception" (FR-010).
- Q: Is there a cap on how far ahead a review date may be? → A: No. Review sets a sensible date, and the pre-release review looks at every exception (Assumptions).
- Q: How is an exception for a yanked crate recorded? → A: It isn't. There is no exception path for a yanked crate (FR-014).
- Q: Does a new finding fail at once, or is there a grace period? → A: It fails at once. A finding on a branch that changed no dependencies is fixed in its own commit (FR-015).
- Q: Is `yanked = "deny"` turned on in `src-tauri/deny.toml`? → A: Yes. A yanked crate version always fails, whether it ships or is only used at build time. If one is in today's `Cargo.lock`, it is updated in this feature. _None is: checked 2026-11-01._
- Q: Does lint gate the project's own use of deprecated TypeScript and JavaScript APIs? → A: Yes. A type-aware deprecation rule is turned on, and each existing hit is fixed or justified where it is (FR-018).
- Q: How are the pinned Node.js and npm versions checked for end of life? → A: By hand, before each release (FR-016).
- Q: What is done about `glob@10.5.0` (development-only, via `@wdio/config@10.0.1`)? → A: It is replaced, by updating the WebdriverIO packages to 10.1, whose `@wdio/config` uses a current `glob` (FR-017).
- Q: How are crates with a RustSec unmaintained notice handled? → A: The owner said: "I want to clarify that we should not be directly using crates that have unmaintained notices, but we can except those under special circumstances, or if they come in via indirect dependencies (that's another kind of exception)". So an unmaintained crate is its own rule. It isn't folded into the npm deprecation rule and doesn't take the npm exception's shape. Its exceptions stay in `src-tauri/deny.toml` as today. A direct dependency must not be an unmaintained crate. An exception for a direct one is allowed only in special circumstances, and its comment must say that it is direct and why. An indirect one may be excepted as today, with its comment naming the parent that brings it in. The nine existing entries are all indirect, and stay (FR-019).
- Q: Is the constitution's dependency gate amended to name deprecations? → A: Not in this feature. The owner said: "I want to see a new proposal for the constitution update when it's done. Also I think per the constitution or per speckit the constitution change is its own standalone PR". This feature changes no part of the constitution. Its rules are stricter project rules under the existing dependency-audit gate, recorded in `DEVELOPMENT.md`. An amendment comes in its own pull request. The owner approved it on 2026-11-01, as three separate rules: npm deprecated packages, unmaintained crates direct against indirect, and yanked crates never. A separate task opens it after this feature lands.

## User Scenarios & Testing *(mandatory)*

The "user" here is a HoploDex maintainer or contributor: someone who runs the dependency audit before a pull request, reviews a Dependabot update, or prepares a release. Collectors who use the app are affected only indirectly. The app they get carries no dependency that has stopped receiving fixes unless someone made and recorded that decision, and it never carries a crate version its publisher withdrew.

### User Story 1 - The Audit Stops a Deprecated npm Package That Has No Decision (Priority: P1)

A maintainer runs the dependency audit. Every npm package version in the lockfile whose maintainer has marked it deprecated is listed with its version, where it sits in the tree, whether it reaches the shipped app or is development tooling only, and the deprecation message. The audit fails on any such package that has no recorded exception, and the failure says what to do: replace it, or record an exception where one is allowed. A package with a valid exception is reported as excepted and doesn't fail the audit.

**Why this priority**: This is the gap the issue names. A deprecated npm package passes the audit today, and `npm install` only prints a warning that scrolls past.

**Independent Test**: Run the check against fixture lockfiles: a shipped deprecated package with no exception, a development-only one with no exception, one with a valid exception, one whose exception has expired, one whose exception covers a different version range, and a shipped direct dependency with an exception. Confirm that each passes or fails as the acceptance scenarios say, and that the real lockfile passes (Story 5).

**Acceptance Scenarios**:

1. **Given** a lockfile where a package that ships in the app resolves to a deprecated version, **When** the audit runs, **Then** it fails and names the package, version, lockfile location and deprecation message, and says that it ships.
2. **Given** a lockfile where a development-only package resolves to a deprecated version and no exception names it, **When** the audit runs, **Then** it fails, names the package and says that it is development-only.
3. **Given** a deprecated package with an exception whose version range covers the resolved version, whose review date hasn't passed and whose recorded scope matches, and which isn't a direct dependency that ships, **When** the audit runs, **Then** it reports the package as excepted, with the review date, and doesn't fail on it.
4. **Given** an exception for a package whose version range doesn't cover the resolved version, **When** the audit runs, **Then** the exception doesn't apply and the package fails as in scenario 1 or 2.
5. **Given** a direct dependency of the app that ships, resolves to a deprecated version and has an exception, **When** the audit runs, **Then** it fails and says that a shipped direct dependency must be replaced and can't be excepted.
6. **Given** a lockfile where the same deprecated package appears at several locations (a nested copy under another package), **When** the audit runs, **Then** each location is reported, and each fails or is excepted on its own version.
7. **Given** a lockfile with no deprecated package and no exceptions, **When** the audit runs, **Then** the deprecation check passes and says so.

---

### User Story 2 - Exceptions Come Back When Their Review Date Passes (Priority: P1)

An npm deprecation exception records why the package can't be replaced yet, whether it ships, what replaces it, the package that brings it in, and a date to review it. When that date passes, the audit fails on the exception, so the decision is made again instead of being forgotten. An exception that no longer matches anything in the lockfile also fails the audit, so the list doesn't fill up with stale entries.

**Why this priority**: Neither `audit-ci` nor `cargo-deny` expires its entries, so a review date written in a comment depends on someone noticing it. Without expiry, a deprecation exception would become a permanent pass.

**Independent Test**: With fixture exceptions, set a review date in the past and confirm the audit fails, naming the entry. Set the date to today and confirm it still passes. Point an exception at a package the lockfile doesn't have and confirm the audit fails and tells you to remove it. Leave out a required field, or give a malformed date, scope or range, and confirm the audit fails, naming the entry and the field.

**Acceptance Scenarios**:

1. **Given** an exception whose review date is before today, **When** the audit runs, **Then** it fails, names the exception and its review date, and says to review it: replace the package, or record a new date with the reason.
2. **Given** an exception whose review date is today, **When** the audit runs, **Then** the exception still applies.
3. **Given** an exception that matches no deprecated package in the lockfile, **When** the audit runs, **Then** it fails and says to remove the exception.
4. **Given** an exception that lacks a required field, or has a review date, scope or version range that can't be read, **When** the audit runs, **Then** it fails, naming the exception and what is wrong with it.
5. **Given** an exception recorded as development-only for a package that now ships in the app, **When** the audit runs, **Then** the exception doesn't apply, and the audit fails as for a shipped package.

---

### User Story 3 - A Yanked Rust Crate Always Fails the Audit (Priority: P2)

A maintainer runs the dependency audit, and a crate version in the Rust lockfile has been yanked from the registry. The audit fails and names the crate. Nothing can make it pass except moving off that version. A yanked release is its publisher saying the release must not be used, which means something is very wrong with it. This is not the same as a deprecation, and it is never kept with an exception, whether the crate ships in the program or is only used at build time.

**Why this priority**: The change itself is small, one setting in the existing tool's configuration plus a guard against excepting a yanked crate. It is P2 because a yanked version is rare, and `Cargo.lock` keeps resolving a yanked version that is already in it, so it shows up only when the audit looks.

**Independent Test**: Run the Rust advisory check and confirm that yanked crates are denied, not warned about, and that the current lockfile has none. Confirm that a check fails if the Rust audit's configuration gains an entry that would let a yanked crate pass.

**Acceptance Scenarios**:

1. **Given** a Rust lockfile containing a crate version that has been yanked, whether it ships or is build-only, **When** the audit runs, **Then** it fails and names the crate and version.
2. **Given** the Rust audit's configuration, **When** someone adds an entry that would let a yanked crate version pass, **Then** the project's tests fail and say that a yanked crate is updated, never excepted.

---

### User Story 4 - Lint Stops New Uses of Deprecated APIs (Priority: P3)

A contributor writes TypeScript that calls a function, component or option that a dependency has marked deprecated (React, Radix, Tauri's JavaScript packages, or any other). Lint fails, naming the use, the way the Rust lint already fails on a deprecated Rust API. A use that has to stay for now carries a disable comment at that line with its reason.

**Why this priority**: A deprecated API in the project's own code is the same early signal as a deprecated package, and the issue's "Frameworks and runtimes" point asks for it. It is P3 because it changes the lint gate rather than the audit, and it doesn't affect which dependency versions the project takes.

**Independent Test**: Lint a fixture that uses a deprecated API and confirm that it fails. Confirm that the whole project lints clean once each existing hit has been fixed or justified, and that the Rust lint's treatment of deprecation warnings is recorded in the policy.

**Acceptance Scenarios**:

1. **Given** TypeScript code that uses an API marked deprecated in its type declarations, **When** lint runs, **Then** it fails, naming the file, line and API.
2. **Given** a use that has to stay, with a disable comment and a reason at that line, **When** lint runs, **Then** it passes.
3. **Given** the project at the time of this feature, **When** lint runs, **Then** it passes, and each remaining disable for this rule has its reason.

---

### User Story 5 - The Policy Is Written Down and the Current Findings Are Cleared (Priority: P2)

A contributor who meets a deprecation finding, an unmaintained crate or a yanked crate, or who wants to add a dependency, finds the policy in the "Dependency audit" section of the development guide. The deprecation policy says what counts as deprecated, what fails the audit, what an exception has to record and when it is reviewed, and what to check by hand that the tools can't see: archived upstream repositories, a direct dependency that names a successor, and the end of life of the pinned Node.js and npm versions. The unmaintained-crate rule is stated separately: no direct dependency on one without a special circumstance recorded, and an indirect one is excepted with its parent named. The yanked-crate rule is stated separately too, as a rule with no exceptions. Before a release, every open deprecation exception is reviewed along with the advisory exceptions. The deprecations in the lockfile today have been replaced, so the audit passes on `main`.

**Why this priority**: The checks enforce the rules, but without the written policy a contributor can't tell whether to replace a package or record an exception, or what goes in the exception. Clearing the current findings is the policy's first use, and the audit can't pass without it.

**Independent Test**: Read the "Dependency audit" section and confirm that it answers each of the issue's five "Decisions to make" as clarified, and that it keeps yanked crates out of the deprecation policy. Confirm that the release checks include reviewing deprecation exceptions and making the hand checks. Run the full audit on the feature branch and confirm that it passes.

**Acceptance Scenarios**:

1. **Given** the development guide, **When** a contributor reads "Dependency audit", **Then** it states what counts as deprecated, the scope (shipped against development-only, direct against transitive), the rule for adding a dependency, the exception format and the expiry rule, and that a new finding on an unchanged branch is fixed in its own commit.
2. **Given** the development guide, **When** a contributor looks up a yanked crate, **Then** they find a separate rule: it always fails, and it is updated, never excepted.
3. **Given** the development guide, **When** a contributor meets a RustSec unmaintained notice, **Then** they find its own rule: a direct dependency must not be unmaintained except in special circumstances stated in its exception, and an indirect one is excepted with the parent named.
4. **Given** a release is being prepared, **When** the maintainer follows the release checks, **Then** they include reviewing every open deprecation exception and the hand checks the tools can't make.
5. **Given** the lockfiles at the time of this feature, **When** the full audit runs, **Then** it passes with no npm deprecation exceptions.

---

### Edge Cases

- **A deprecation on an old major version HoploDex doesn't use.** npm records the deprecation flag on each version, and the lockfile only shows the flag of the version that resolved. A deprecated old major that nothing resolves to never shows up.
- **A package that is deprecated and later un-deprecated.** If the lockfile entry has no deprecation message, or has an empty one, nothing is reported. A matching exception then fails as stale (Story 2, scenario 3).
- **A deprecated optional or platform-specific package** (one that installs only on some operating systems). It is reported like any other package, and its scope is decided by the license audit's definition of development-only, so the two audits agree on what ships.
- **A deprecation that arrives without any dependency change.** The lockfile records the flag only when it is regenerated. So a finding appears when the lockfile is next refreshed (an install, an update, a Dependabot pull request), and is fixed in its own commit like a new advisory.
- **The same package at several versions**: each lockfile location is judged on its own version. One exception with a version range can cover several of them.
- **A package that is both a direct dependency and nested elsewhere**: only the top-level copy that resolves for the app's own `dependencies` counts as the shipped direct dependency (FR-005). A nested copy under another package is transitive.
- **A crate yanked after it entered `Cargo.lock`**: the audit fails on the next run, even on a branch that changed nothing, and the crate is updated in its own commit.
- **No network**: the npm deprecation check reads only the lockfile and its exceptions, so it runs offline. The Rust yanked check needs the crate registry's index, which the Rust advisory check already fetches.

## Requirements *(mandatory)*

### Functional Requirements

**What counts as deprecated**

- **FR-001**: For this policy, a dependency is **deprecated** when one of these holds:
  - (a) the npm package version it resolves to carries a non-empty deprecation message in the lockfile;
  - (b) it is a direct dependency whose upstream repository is archived, or that names a successor.

  The audit enforces (a). (b) needs network lookups, so it is a hand check before each release (FR-016). A crate with a RustSec unmaintained notice has its own rule (FR-019), and a yanked crate version has another (FR-014). Neither is grouped with this policy.
- **FR-002**: A deprecation counts only when it covers the version HoploDex resolves to. A deprecation of a version or major release that the lockfile doesn't resolve to isn't a finding.

**The npm deprecation check**

- **FR-003**: `npm run audit` MUST run a deprecation check (`npm run audit:deprecated`) that reads `package-lock.json` and reports every package location whose resolved version is deprecated, with its name, version, lockfile location, scope (shipped or development-only) and deprecation message.
- **FR-004**: A package's scope MUST be decided by the same definition of development-only that the license audit uses (010 research.md §5, §6), so the two audits never disagree about what ships.
- **FR-005**: A deprecated package that ships in the app MUST fail the audit unless it is transitive and has a valid exception. A shipped direct dependency (one named in the app's `dependencies`, at its top-level lockfile location) that is deprecated MUST fail, whatever exception exists, and the failure MUST say that it has to be replaced.
- **FR-006**: A deprecated development-only package, direct or transitive, MUST fail the audit unless it has a valid exception.
- **FR-007**: An exception MUST record:
  - the package name and the version range it covers;
  - the scope it was recorded for (shipped or development-only);
  - why the package can't be replaced yet;
  - the package that brings it in, for a transitive dependency;
  - the successor, or that none is known;
  - a review date.

  It is kept in a data file next to the audit's configuration, in a form the check can read.
- **FR-008**: An exception MUST apply only to deprecated packages that have its name, whose resolved version is within its range, and whose scope matches the scope it records. A development-only exception never covers a package that ships.
- **FR-009**: An exception whose review date is before the day the audit runs MUST fail the audit, naming the exception. On the review date itself, the exception still applies.
- **FR-010**: An exception that matches no deprecated package in the lockfile MUST fail the audit and say to remove it.
- **FR-011**: An exception that lacks a required field, or that has an unreadable version range, scope or review date, MUST fail the audit, naming the exception and the problem.
- **FR-012**: A pull request MUST NOT add a shipped direct dependency, or move one to a version, that is already deprecated. The check enforces this through FR-005. A development-only direct dependency, and a transitive one arriving with a new dependency, follow FR-006 to FR-011.
- **FR-013**: The check MUST NOT add a dependency of its own, MUST run offline, and MUST finish in under 2 seconds on the current lockfile.

**Yanked crates (not a deprecation)**

- **FR-014**: The Rust audit MUST fail on any yanked crate version in `src-tauri/Cargo.lock`, whether the crate ships in the program or is only used at build time. A yanked crate MUST NOT be excepted. The project's tests MUST fail if the Rust audit's configuration gains an entry that would let a yanked crate version pass. Any yanked crate in the lockfile when this feature lands MUST be updated in it. _None was, as of 2026-11-01._

**Policy, release checks and first use**

- **FR-015**: The "Dependency audit" section of `DEVELOPMENT.md` MUST state:
  - the deprecation policy: what counts as deprecated (FR-001, FR-002), what fails the audit (FR-005, FR-006), the rule for adding a dependency (FR-012), the exception format and its expiry (FR-007 to FR-011), and that a finding that appears on a branch which changed no dependencies is fixed in its own commit;
  - separately, the unmaintained-crate rule (FR-019);
  - separately, the yanked-crate rule (FR-014).
- **FR-016**: The checks made by hand before a release MUST include:
  - reviewing every open npm deprecation exception and every unmaintained-crate exception, whatever its review date;
  - checking each direct dependency for an archived repository or a named successor;
  - checking that the pinned Node.js version, and the npm it brings, are still supported.
- **FR-017**: Every npm deprecation in the lockfile when this feature lands MUST be replaced where a replacement exists, or else recorded as an exception under FR-007, so that the audit passes. Today that is `glob@10.5.0`, development-only, brought in by `@wdio/config@10.0.1`, and it is replaced by updating the WebdriverIO packages to 10.1. The issue's other two, `mocha`'s nested `glob` and `inflight`, have already left the lockfile.

**Unmaintained crates (their own rule)**

- **FR-019**: A RustSec unmaintained notice MUST keep failing the Rust audit (`unmaintained = "all"`), and its exceptions MUST stay `ignore` entries in `src-tauri/deny.toml`, with a `reason` and, above them, a comment with the analysis and a review date. Every such comment MUST also say whether the crate is a direct dependency of HoploDex or an indirect one.
  - **Indirect**: the comment names the parent that brings it in. The exception is allowed as today.
  - **Direct**: HoploDex MUST NOT depend directly on an unmaintained crate. An exception is allowed only in special circumstances, and its comment MUST say that the crate is direct and why there is no alternative yet.

  No tooling tells direct from indirect, because that would need the advisory database mapped to `Cargo.toml`. It is checked in review, and before each release (FR-016). The nine entries in `deny.toml` at the time of this feature are all indirect and already name their parent.

**Deprecated APIs in the project's own code**

- **FR-018**: Lint MUST fail on a use, in the project's TypeScript (`**/*.{ts,tsx}`), of an API that its type declarations mark deprecated. Plain-JavaScript scripts (`.mjs`) are not covered by this rule. A use that has to stay MUST carry a disable comment for that rule at that line, with its reason. Every existing hit MUST be fixed or justified this way. Rust's deprecation warnings are already errors under the Rust lint gate, and the policy MUST record this.

### Key Entities

- **Deprecation finding**: one lockfile location whose resolved version is deprecated, with its name, version, location, scope (shipped or development-only), whether it is a shipped direct dependency, its message, and its outcome (fails, or excepted by a named exception).
- **Deprecation exception**: a recorded decision to keep a deprecated npm package for now. It holds the package name, version range, recorded scope, reason, the package that brings it in, the successor and the review date. It is valid only while its date hasn't passed and it still matches something, and never for a shipped direct dependency.
- **Unmaintained-crate exception**: an `ignore` entry in `src-tauri/deny.toml` for a RustSec unmaintained notice. It records whether the crate is direct or indirect, the parent (if indirect) or the special circumstance (if direct), and a review date in its comment.
- **Yanked crate**: a crate version in `Cargo.lock` that its publisher withdrew from the registry. It is always a failure, and there is no exception entity for it.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Every deprecated package in a lockfile is reported by a single audit run, with none missed, across the fixture cases in Stories 1 and 2. That covers the five cases the issue lists, plus a shipped direct dependency, a stale exception, a malformed exception and a nested location.
- **SC-002**: A contributor who meets a deprecation finding or a yanked crate can tell from the failure message and the development guide alone what to do (replace it, update it, or record an exception where one is allowed), without asking anyone.
- **SC-003**: No deprecation exception stays in force past its review date. On the first audit run after the date, the exception fails.
- **SC-004**: The full dependency audit and lint pass on the feature branch, with no npm deprecation exceptions and no yanked crate.
- **SC-005**: The npm deprecation check adds under 2 seconds to the audit and needs no network access.

## Assumptions

- **Shipped vs development-only** follows the license audit's existing definition (`scripts/licenses/shipped.mjs`), so nothing new decides what ships.
- **Exceptions are keyed by package name and version range, not by dependency path.** Unlike a vulnerability, the problem with a deprecated version (no more fixes) doesn't depend on how it is reached. The recorded scope stops a development-only exception from covering a shipped package.
- **No cap on review dates**: the reviewer of the pull request that adds an exception judges its date, and the pre-release review looks at every exception whatever its date.
- **The advisory exceptions** in `audit-ci.jsonc` and `src-tauri/deny.toml`, whose review dates are in comments, are unchanged by this feature.
- **No grace period**: a finding fails the audit as soon as it appears, the same as a new advisory. The exception path is the grace period, where one is allowed.
- **The audit workflow** (`.github/workflows/audit.yml`) already runs `npm run audit`, so the new check runs on every pull request and daily on `main` with no change to the workflow.
- **The pinned Node.js version** (the dev container's `NODE_VERSION`) and **archived upstream repositories** are hand checks before a release (FR-016), since automating them would need network data or a hand-kept table.
- **Type-aware lint is slower**: FR-018's rule reads the TypeScript types, so lint takes longer. The plan measures by how much.
- **The constitution is unchanged.** Its dependency gate names vulnerabilities and licenses, and this feature's rules sit under that gate as stricter project rules (`DEVELOPMENT.md`). An amendment that names deprecations, with yanked crates as a separate rule, would be its own pull request (Clarifications, 2026-11-01).

## Relationship to Earlier Features

- **010** (`specs/010-license-notices/`): the npm check reuses the license audit's definition of what ships (010 research.md §5, §6) and follows `scripts/check-npm-licenses.mjs`'s shape: it reads only the lockfile and has no dependencies of its own. Nothing in 010's behaviour changes.

## Source Request

GitHub issue #43, "Dependency audit: a rule and check for deprecated dependencies" (labels: needs spec, release, security), opened 2026-09-30. It is reproduced verbatim below and has no comments.

> ## Problem
>
> The dependency audit (`npm run audit`, see "Dependency audit" in `DEVELOPMENT.md`) catches known vulnerabilities, and on the Rust side unmaintained and unsound RustSec notices. It has no rule for **deprecated** dependencies, so they can enter or stay in the tree unnoticed:
>
> - **npm**: a package the maintainer marked deprecated (`npm deprecate`, recorded as `"deprecated"` in `package-lock.json`) passes `audit-ci`, because that only looks at advisories. `package-lock.json` has three today (`glob`, `mocha`'s nested `glob`, `inflight`), and `npm install` only prints a warning that scrolls past.
> - **Rust**: yanked crate versions, and crates whose repository is archived or whose README says "deprecated" without a RustSec notice, aren't covered. `cargo-deny`'s `yanked` setting and the `unmaintained` policy only catch some cases.
> - **Frameworks and runtimes**: deprecated APIs, features or pinned versions of our own direct dependencies (Tauri 2 plugins, Radix, React APIs, the Node/npm versions) have no check either. Compiler and lint deprecation warnings aren't gated.
>
> Deprecated is not the same as vulnerable, but it is the early signal: no more fixes, so the next advisory has no patch. Deciding this when a dependency is added, or when the audit first flags it, is cheaper than deciding it during a release.
>
> ## Goal
>
> Agree a written policy, then enforce as much of it as is practical in `npm run audit`, so a deprecated dependency is either replaced or carries a recorded, dated exception, the same way an advisory exception does.
>
> ## Decisions to make
>
> 1. **What counts as deprecated.** Proposal: an npm `deprecated` flag on any resolved version; a yanked crate version; a RustSec `unmaintained` notice (already enforced); a direct dependency whose repository is archived or that names a successor. Decide what to do about a deprecation that only concerns an old major version we aren't on.
> 2. **Scope.** Shipped dependencies vs dev-only tooling (test runners, build tools). Proposal: shipped code fails the audit; dev-only findings are reported and need a decision, but a dated exception is enough. Direct vs transitive: a transitive deprecation we can't change is an exception with a "review when parent X updates" date.
> 3. **Adding a dependency.** Should adding a new dependency (or a version) that is deprecated be refused outright, with no exception path?
> 4. **Exception format.** Reuse the advisory exception rule: an entry next to the tool's config, with what's deprecated, why it can't be replaced yet, whether it reaches the shipped app, the successor, and a date to review. Neither tool expires entries, so decide whether a small script should fail on expired review dates.
> 5. **Severity and timing.** Fail the audit immediately, or warn for a grace period? The audit can already start failing on a branch that changed no dependencies (new advisories); deprecation flags appear the same way, so the "fix it in its own commit" rule should apply.
>
> ## Suggested implementation
>
> - `scripts/check-npm-deprecated.mjs`, modelled on `scripts/check-npm-licenses.mjs`: read `package-lock.json`, list every entry with a `deprecated` field, split shipped from dev-only, fail on shipped ones and on dev-only ones without an exception. Exceptions in a small config file (or in `audit-ci.jsonc`-style comments plus a data list) keyed by package and version range, each with a review date.
> - Rust: turn on `yanked = "deny"` in `src-tauri/deny.toml` `[advisories]` and confirm it is what we want; keep `unmaintained = "all"`. Consider a check for archived upstream repositories only if it can be done offline or cheaply, otherwise leave it to the periodic review.
> - Wire the npm check into `npm run audit` as `audit:deprecated`, and extend the "Dependency audit" section of `DEVELOPMENT.md` with the policy and the exception format.
> - Add a step to the pre-release checklist (and the pre-release security review, per the constitution) to review every open deprecation exception.
> - Triage the three current npm deprecations as the first use: replace, or record an exception with a review date.
>
> ## Tests
>
> The check script needs tests against fixture lockfiles (constitution II): a shipped deprecated package fails, a dev-only one without an exception fails, one with a valid exception passes, an expired exception fails, and an exception for a different version range doesn't apply.
>
> ## Notes
>
> - Likely needs `/speckit-specify` (touches the constitution's dependency gate), or a constitution amendment if the gate wording changes.
> - Related: the existing advisory exception rules in `DEVELOPMENT.md` and `src-tauri/deny.toml`, and the license audit (`scripts/check-npm-licenses.mjs`).
