# Feature Specification: Deprecated Dependencies in the Dependency Audit

**Feature Branch**: `012-deprecated-dependencies`

**Created**: 2026-10-31

**Status**: Draft

**Input**: User description: GitHub issue #43, "Dependency audit: a rule and check for deprecated dependencies". The dependency audit catches known vulnerabilities, plus unmaintained and unsound notices for Rust crates, but has no rule for deprecated dependencies. This feature writes that rule down and makes `npm run audit` enforce as much of it as is practical. A deprecated dependency is then either replaced, or carries a recorded, dated exception the way an advisory exception does.

## User Scenarios & Testing *(mandatory)*

The "user" here is a HoploDex maintainer or contributor: someone who runs the dependency audit before a pull request, reviews a Dependabot update, or prepares a release. Collectors who use the app are affected only indirectly. The app they get carries no dependency that has stopped receiving fixes unless someone made and recorded that decision.

### User Story 1 - The Audit Stops a Deprecated npm Package That Has No Decision (Priority: P1)

A maintainer runs the dependency audit. Every npm package version in the lockfile whose maintainer has marked it deprecated is listed with its version, where it sits in the tree, whether it reaches the shipped app or is development tooling only, and the deprecation message. The audit fails on any such package that has no recorded exception, and the failure says what to do: replace it, or record an exception. A package with a valid exception is reported as excepted and doesn't fail the audit.

**Why this priority**: This is the gap the issue names. A deprecated npm package passes the audit today, and `npm install` only prints a warning that scrolls past. The lockfile already holds one, so the check has a real first case.

**Independent Test**: Run the check against fixture lockfiles: a shipped deprecated package with no exception, a development-only one with no exception, one with a valid exception, one whose exception has expired, and one whose exception covers a different version range. Confirm that each passes or fails as the acceptance scenarios say, and that the real lockfile passes once the current deprecation has been triaged (Story 4).

**Acceptance Scenarios**:

1. **Given** a lockfile where a package that ships in the app resolves to a deprecated version, **When** the audit runs, **Then** it fails and names the package, version, lockfile location and deprecation message, and says that it ships.
2. **Given** a lockfile where a development-only package resolves to a deprecated version and no exception names it, **When** the audit runs, **Then** it fails, names the package and says that it is development-only.
3. **Given** a deprecated package with an exception whose version range covers the resolved version, whose review date hasn't passed and whose recorded scope (shipped or development-only) matches, **When** the audit runs, **Then** it reports the package as excepted, with the review date, and doesn't fail on it.
4. **Given** an exception for a package whose version range doesn't cover the resolved version, **When** the audit runs, **Then** the exception doesn't apply and the package fails as in scenario 1 or 2.
5. **Given** a lockfile with no deprecated package and no exceptions, **When** the audit runs, **Then** the deprecation check passes and says so.
6. **Given** a lockfile where the same deprecated package appears at several locations (a nested copy under another package), **When** the audit runs, **Then** each location is reported, and each fails or is excepted on its own version.

---

### User Story 2 - Exceptions Come Back When Their Review Date Passes (Priority: P1)

An exception records why a deprecated package can't be replaced yet, whether it ships, what replaces it, the package that brings it in, and a date to review it. When that date passes, the audit fails on the exception, so the decision is made again instead of being forgotten. An exception that no longer matches anything in the lockfile also fails the audit, so the list doesn't fill up with stale entries.

**Why this priority**: Neither `audit-ci` nor `cargo-deny` expires its entries, so a review date written in a comment depends on someone noticing it. Without expiry, a deprecation exception would become a permanent pass.

**Independent Test**: With fixture exceptions, set a review date in the past and confirm the audit fails, naming the entry. Set the date to today and confirm it still passes. Point an exception at a package the lockfile doesn't have and confirm the audit fails and tells you to remove it. Leave out a required field, or give a malformed date or range, and confirm the audit fails, naming the entry and the field.

**Acceptance Scenarios**:

1. **Given** an exception whose review date is before today, **When** the audit runs, **Then** it fails, names the exception and its review date, and says to review it: replace the package, or record a new date with the reason.
2. **Given** an exception whose review date is today, **When** the audit runs, **Then** the exception still applies.
3. **Given** an exception that matches no deprecated package in the lockfile, **When** the audit runs, **Then** it fails and says to remove the exception.
4. **Given** an exception that lacks a required field, has a review date that isn't a valid date, or has a version range that can't be read, **When** the audit runs, **Then** it fails, naming the exception and what is wrong with it.
5. **Given** an exception recorded as development-only for a package that now ships in the app, **When** the audit runs, **Then** the exception doesn't apply, and the audit fails as for a shipped package.

---

### User Story 3 - The Audit Stops a Yanked Rust Crate (Priority: P2)

A maintainer runs the dependency audit, and a crate version in the Rust lockfile has been yanked from the registry. The audit fails and names the crate. A yanked version is the Rust registry's signal that a release should not be used, and today the audit only warns about it. Unmaintained notices keep failing the audit as they do now, with their exceptions recorded the same way.

**Why this priority**: It is a one-line policy change in the existing tool's configuration, but it closes the Rust half of the gap. It is P2 because a yanked version is rarer than an npm deprecation, and `Cargo.lock` keeps resolving a yanked version that is already in it, so it shows up only when someone looks.

**Independent Test**: Run the Rust advisory check and confirm that yanked crates are denied, not warned about, and that the current lockfile has none, or each one it has has been triaged (Story 4). Confirm that an unmaintained notice with no exception still fails the audit.

**Acceptance Scenarios**:

1. **Given** a Rust lockfile containing a crate version that has been yanked, **When** the audit runs, **Then** it fails and names the crate and version.
2. **Given** a yanked crate that can't be updated yet, **When** an exception is recorded next to the Rust audit's configuration with its reason, how it reaches HoploDex and a review date, **Then** the audit passes on it.
3. **Given** a RustSec unmaintained notice with no exception, **When** the audit runs, **Then** it fails, as it does today.

---

### User Story 4 - The Policy Is Written Down and the Current Deprecations Are Triaged (Priority: P2)

A contributor who meets a deprecation finding, or who wants to add a dependency, finds the policy in the "Dependency audit" section of the development guide. It says what counts as deprecated, what fails the audit, what an exception has to record and when it is reviewed, and what to check by hand that the tools can't see: archived upstream repositories, a direct dependency that names a successor, and the end of life of the pinned Node.js and npm versions. Before a release, every open deprecation exception is reviewed along with the advisory exceptions. The deprecations in the lockfile today have each been replaced or have a dated exception, so the audit passes on `main`.

**Why this priority**: The check enforces the rule, but without the written policy a contributor can't tell whether to replace a package or record an exception, or what goes in the exception. The triage is the policy's first use, and the audit can't pass on `main` without it.

**Independent Test**: Read the "Dependency audit" section and confirm that it answers each of the issue's five "Decisions to make" as clarified. Confirm that the list of release checks includes reviewing deprecation exceptions and making the hand checks. Run the full audit on the feature branch and confirm that it passes.

**Acceptance Scenarios**:

1. **Given** the development guide, **When** a contributor reads "Dependency audit", **Then** it states what counts as deprecated, the scope (shipped against development-only, direct against transitive), the rule for adding a dependency, the exception format and the expiry rule, and that a new finding on an unchanged branch is fixed in its own commit.
2. **Given** a release is being prepared, **When** the maintainer follows the release checks, **Then** they include reviewing every open deprecation exception and the hand checks the tools can't make.
3. **Given** the lockfiles at the time of this feature, **When** the full audit runs, **Then** it passes, and every deprecated package that is still present has an exception with a review date.

---

### Edge Cases

- **A deprecation on an old major version HoploDex doesn't use.** npm records the deprecation flag on each version, and the lockfile only shows the flag of the version that resolved. A deprecated old major that nothing resolves to never shows up, and a package whose current version isn't deprecated passes.
- **A package that is deprecated and later un-deprecated.** If the lockfile entry has no deprecation message, or has an empty one, nothing is reported. A matching exception then fails as stale (Story 2, scenario 3) and should be removed.
- **A deprecated optional or platform-specific package** (one that installs only on some operating systems). It is reported like any other package, and its scope is decided by the license audit's definition of development-only, so the two audits agree on what ships.
- **A deprecation that arrives without any dependency change.** A maintainer can deprecate a version that is already in the lockfile, but the lockfile records the flag only when it is regenerated. So a finding appears when the lockfile is next refreshed (an install, an update, a Dependabot pull request), and is fixed in its own commit like a new advisory.
- **The same package at several versions**: each lockfile location is judged on its own version. One exception with a version range can cover several of them.
- **A dependency added in a pull request that is already deprecated**: see FR-012.
- **No network**: the npm check reads only the lockfile and its exceptions, so it runs offline. The Rust yanked check needs the crate registry's index, which the advisory check already fetches.

## Requirements *(mandatory)*

### Functional Requirements

**What counts as deprecated**

- **FR-001**: For this policy, a dependency is **deprecated** when one of these holds:
  - (a) the npm package version it resolves to carries a non-empty deprecation message in the lockfile;
  - (b) the crate version it resolves to has been yanked from the crate registry;
  - (c) RustSec has published an unmaintained notice for it, which the audit already enforces;
  - (d) it is a direct dependency whose upstream repository is archived, or that names a successor.

  The audit enforces (a) to (c). (d) needs network lookups, so it is a hand check before each release (FR-016).
- **FR-002**: A deprecation counts only when it covers the version HoploDex resolves to. A deprecation of a version or major release that the lockfile doesn't resolve to isn't a finding.

**The npm check**

- **FR-003**: `npm run audit` MUST run a deprecation check (`npm run audit:deprecated`) that reads `package-lock.json` and reports every package location whose resolved version is deprecated, with its name, version, lockfile location, scope (shipped or development-only) and deprecation message.
- **FR-004**: A package's scope MUST be decided by the same definition of development-only that the license audit uses (010 research.md §5, §6), so the two audits never disagree about what ships.
- **FR-005**: A deprecated package that ships in the app MUST fail the audit unless it has a valid exception. [NEEDS CLARIFICATION: Can any shipped deprecated package carry an exception, or only a transitive one that HoploDex can't change? The issue proposes that shipped code fails, and that a transitive deprecation we can't change is an exception.]
- **FR-006**: A deprecated development-only package MUST fail the audit unless it has a valid exception.
- **FR-007**: An exception MUST record:
  - the package name and the version range it covers;
  - the scope it was recorded for (shipped or development-only);
  - why the package can't be replaced yet;
  - the package that brings it in, for a transitive dependency;
  - the successor, or that none is known;
  - a review date.

  It is kept next to the audit's configuration, in a form the check can read.
- **FR-008**: An exception MUST apply only to deprecated packages that have its name, whose resolved version is within its range, and whose scope matches the scope it records. A development-only exception never covers a package that ships.
- **FR-009**: An exception whose review date is before the day the audit runs MUST fail the audit, naming the exception. On the review date itself, the exception still applies.
- **FR-010**: An exception that matches no deprecated package in the lockfile MUST fail the audit and say to remove it.
- **FR-011**: An exception that lacks a required field, or that has an unreadable version range or review date, MUST fail the audit, naming the exception and the problem.
- **FR-012**: A pull request MUST NOT add a direct dependency, or move one to a version, that is already deprecated. [NEEDS CLARIFICATION: Is this refused outright, with no exception path, and does the check enforce it (for example by never accepting an exception for a direct dependency), or does review enforce it?]
- **FR-013**: The check MUST NOT add a dependency of its own, MUST run offline, and MUST finish in under 2 seconds on the current lockfile.

**Rust**

- **FR-014**: The Rust audit MUST fail on a yanked crate version in `src-tauri/Cargo.lock`. An exception is recorded the way the Rust audit already records them: an `ignore` entry with a `reason`, and above it a comment with why the crate can't be updated, how it reaches HoploDex and a review date. Unmaintained and unsound notices keep failing as they do now.

**Policy, release checks and first use**

- **FR-015**: The "Dependency audit" section of `DEVELOPMENT.md` MUST state the policy:
  - what counts as deprecated (FR-001, FR-002);
  - what fails the audit (FR-005, FR-006, FR-014);
  - the rule for adding a dependency (FR-012);
  - the exception format and its expiry (FR-007 to FR-011);
  - that a finding that appears on a branch which changed no dependencies is fixed in its own commit.
- **FR-016**: The checks made by hand before a release MUST include:
  - reviewing every open deprecation exception, npm and Rust, whatever its review date;
  - checking each direct dependency for an archived repository or a named successor;
  - checking that the pinned Node.js version is still supported.
- **FR-017**: Every deprecation in the lockfiles when this feature lands MUST be replaced, or recorded as an exception under FR-007, so that the audit passes. Today that is `glob@10.5.0`, development-only, brought in by `@wdio/config`. The issue's other two, `mocha`'s nested `glob` and `inflight`, have already left the lockfile.
- **FR-018**: Deprecation warnings for the project's own use of a dependency's API are handled as follows:
  - Rust: the compiler already warns about these, and the lint gate already turns warnings into errors. The policy records this.
  - TypeScript: [NEEDS CLARIFICATION: Does this feature gate uses of deprecated TypeScript or JavaScript APIs (React, Radix, Tauri's JavaScript packages) in lint, or is that a follow-up issue? The lint has no type-aware rule today.]

### Key Entities

- **Deprecation finding**: one lockfile location whose resolved version is deprecated, with its name, version, location, scope (shipped or development-only), message, and outcome (fails, or excepted by a named exception).
- **Deprecation exception**: a recorded decision to keep a deprecated package for now. It holds the package name, version range, recorded scope, reason, the package that brings it in, the successor and the review date. It is valid only while its date hasn't passed and it still matches something.
- **Yanked-crate exception**: an entry in the Rust audit's configuration, in the same form as its existing advisory exceptions.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Every deprecated package in a lockfile is reported by a single audit run, with none missed, across the fixture cases in Stories 1 and 2. That covers the five cases the issue lists, plus a stale exception, a malformed exception and a nested location.
- **SC-002**: A contributor who meets a deprecation finding can tell from the failure message and the development guide alone whether to replace the package or record an exception, and what to write in it, without asking anyone.
- **SC-003**: No deprecation exception stays in force past its review date. On the first audit run after the date, the exception fails.
- **SC-004**: The full dependency audit passes on the feature branch, and every deprecated package that remains is covered by an exception with a review date.
- **SC-005**: The npm deprecation check adds under 2 seconds to the audit and needs no network access.

## Assumptions

- **Shipped vs development-only** follows the license audit's existing definition (`scripts/licenses/shipped.mjs`), so nothing new decides what ships.
- **Exceptions are keyed by package name and version range, not by dependency path.** Unlike a vulnerability, the problem with a deprecated version (no more fixes) doesn't depend on how it is reached. The recorded scope stops a development-only exception from covering a shipped package, and the package that brings it in is recorded so the reviewer knows what to watch.
- **The review date is in the exception's own data**, so the check can expire it. The advisory exceptions in `audit-ci.jsonc` and `src-tauri/deny.toml`, whose dates are in comments, are unchanged by this feature.
- **No grace period**: a finding fails the audit as soon as it appears, the same as a new advisory. A finding that appears on a branch that changed no dependencies is fixed in its own commit.
- **The audit workflow** (`.github/workflows/audit.yml`) already runs `npm run audit`, so the new check runs on every pull request and daily on `main` with no change to the workflow.
- **The pinned Node.js version** (the dev container's `NODE_VERSION`) is checked by hand before a release (FR-016), since an automated end-of-life check would need network data or a hand-kept table.
- **Archived upstream repositories** are a hand check before a release, not an automated one, because the issue keeps network-dependent checks out unless they are cheap.
- **The constitution's dependency gate** names vulnerabilities and licenses. Whether that wording is amended to name deprecations is a decision for the owner, recorded with the clarifications.

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
