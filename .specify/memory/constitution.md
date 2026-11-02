<!--
Sync Impact Report
Version change: 1.4.0 → 1.5.0
Bump rationale: MINOR. Materially changed guidance in Development Workflow &
Quality Gates (#21): the release review's attack surface moves from an inline
list to one maintained definition that a pull request changing the surface
updates; the per-PR review's upload rule is narrowed to code the pull request
adds or changes; and the re-review a release commit needs after fixes is
stated. No principle is removed or redefined.
Modified principles: none
Modified sections:
  - Development Workflow & Quality Gates:
    - The release review's attack surface is defined once, in
      docs/attack-surface.md (code to review, questions, accepted residual
      risks), which MUST describe the code at the release commit; the seven
      areas listed inline since 1.1.0 are entries there (commands and
      capabilities → ipc-commands; capabilities and CSP → webview-config;
      paths → paths; import and export → import-export; key and passphrase →
      key-passphrase; secure deletion → plaintext-lifetime; decrypted copies
      → document-copies)
    - A pull request that adds, changes or removes attack surface updates the
      list in the same pull request; a per-PR AI review, when run, says
      whether the surface changed and proposes the update
    - A surface is removed only when its code is gone, with the reason in the
      pull request
    - The list records the design, never findings; findings stay in code
      scanning (and private advisories after a release)
    - A per-PR AI review uploads to code scanning only its findings about
      code the pull request adds or changes; a vulnerability it notices in
      code a release already contains is handled as a private advisory
    - After fixes made in response to a release review, the final release
      commit needs carry-forward re-verification of that review's findings
      plus a review of the diff from the reviewed commit to the final one,
      not a second whole-codebase review
Added sections: none
Removed sections: none
Templates requiring updates:
  - .specify/templates/plan-template.md: ✅ the Constitution Check names the
    attack-surface entries the feature adds or changes; the close-out no
    longer comments on #21
  - .specify/templates/tasks-template.md: ✅ a final-phase task to update
    docs/attack-surface.md
  - CLAUDE.md "Spec Kit workflow" gate list: ✅ names the list update;
    "Architecture" points to docs/attack-surface.md
Follow-up TODOs:
  - ✅ docs/attack-surface.md and scripts/attack-surface.mjs with its test
    (in npm test), in the amending pull request
  - Issue 21: the review skill, the per-PR verdict and upload, and the
    issue's description (the committed report is gone since 1.3.0) are a
    separate change
-->

# HoploDex Constitution
<!-- HoploDex: Firearm Collection Inventory App -->

## Core Principles

### I. Code Quality

All code MUST pass automated linting and static analysis before merge; no quality
gate MAY be bypassed without documented justification recorded in the pull
request. Every change MUST go through peer review — self-merges are not
permitted. Functions and modules MUST be kept small and single-purpose;
duplication MUST be factored out only when a real third occurrence appears, not
speculatively. Complexity (new abstractions, dependencies, or architectural
layers) MUST be justified by a concrete, current requirement — YAGNI applies by
default.

**Rationale**: Firearm inventory data (serial numbers, values, provenance) must
remain trustworthy; inconsistent or poorly reviewed code is the primary source
of silent data corruption and security regressions.

### II. Testing Standards (NON-NEGOTIABLE)

Every feature MUST ship with automated tests covering its acceptance scenarios
before it is considered done. Tests MUST be written to fail first against the
unimplemented behavior, then pass once implemented (red-green). Inventory data
operations (create, update, delete, import, export) MUST have integration tests
that exercise real persistence, not mocks. Any bug fix MUST include a
regression test that fails without the fix. The full automated test suite MUST
pass before any merge to the main branch.

Tests, and any other tooling that runs the application (end-to-end runs,
screenshots, human-testing data), MUST NOT read, write, or delete the user's
real database, the key or passphrase stored for it, or any other real
application data. They MUST use throwaway locations created for the run and
test keys or passphrases, and tooling that seeds data MUST refuse to target
the real data directory.

**Rationale**: This app is the system of record for a user's firearm
collection, often tied to legal, insurance, and safety obligations; undetected
regressions have real-world consequences beyond typical inconvenience. The
developers use the app for their own collections, so a test that touches real
data can destroy the very records the app exists to protect.

### III. User Experience Consistency

The application MUST use a single, documented set of UI components, terminology,
and interaction patterns across every screen — no screen may invent its own
navigation, confirmation, or error-handling pattern. Destructive actions
(deleting a firearm record, bulk edits, data wipes) MUST use a consistent
confirmation pattern throughout the app. The interface MUST meet WCAG 2.1 AA
accessibility as a baseline. Any deviation from established patterns MUST be
reflected back into the shared design system, not left as a one-off.

**Rationale**: Collection management is a long-lived, trust-sensitive
workflow; inconsistent UX increases the risk of user error on irreversible
actions (e.g., deleting a record) and erodes confidence in the data.

### IV. Performance Requirements

Interactive actions (opening a record, saving an edit, navigating between
screens) MUST provide visible feedback within 100ms and complete within 1s
under expected collection sizes (up to 10,000 items). Search and filter
operations across the full collection MUST return results within 500ms. No
user-facing operation may block the UI thread; long-running work (import,
export, bulk operations) MUST run asynchronously with progress indication.
Performance budgets MUST be re-validated whenever a feature changes data
volume assumptions or query patterns.

**Rationale**: A slow or unresponsive inventory tool discourages the exact
behavior it exists to enable — regular, accurate record-keeping.

### V. User Privacy

Firearm ownership data is highly sensitive: it MUST be treated as
confidential-by-default. Collection data MUST be stored locally or encrypted
at rest and in transit; no record (including serial numbers, valuations, or
storage locations) may be transmitted to a third party or cloud service
without explicit, informed, per-feature user consent. No analytics, crash
reporting, or telemetry may collect collection contents or personally
identifying details tied to specific firearms. Data export/backup features
MUST make it clear to the user what leaves the device and where it goes.
Deleting a record or account MUST actually remove the underlying data, not
just hide it from the UI.

**Rationale**: Beyond ordinary privacy expectations, exposure of this data can
carry safety, legal, and insurance implications for the user; privacy failures
here are not merely inconvenient but potentially harmful.

## Security & Data Handling Constraints

Sensitive fields (serial numbers, storage locations, valuations) MUST be
encrypted at rest using platform-standard encryption; encryption keys MUST
never be logged or transmitted in plaintext. Any feature that syncs or backs
up data over a network or to a cloud service MUST be opt-in, off by default,
and clearly disclosed before first use. This rule covers data that leaves the
device: a backup the application writes only to local storage, in a location
the user can see and choose, is not a network backup and MAY be on by default.
A local folder that another program syncs elsewhere is the user's choice, not
a network feature of the application. Third-party SDKs and dependencies MUST
be reviewed for data-collection behavior before adoption — a dependency that
phones home collection data by default is disqualifying. Access to the local
data store MUST NOT be exposed to other applications without explicit
user-granted permission.

Dependencies, development-only ones included, MUST be kept free of known
vulnerabilities, as checked by the dependency audit:

- A dependency with a known critical advisory MUST NOT be used unless no
  patched release exists anywhere and an analysis of whether and how the
  vulnerability affects HoploDex is recorded with the exception.
- A dependency with a known high advisory MUST NOT be used without a recorded
  mitigation or justification.
- A vulnerability advisory with no severity score counts as high.
- **Unsound** notices block, and are cleared the same way as a high advisory.
- **Unmaintained crates.** HoploDex MUST NOT depend directly on a crate with an
  unmaintained notice, except in a special circumstance recorded with the
  exception. A crate with an unmaintained notice that comes in through another
  dependency MAY be excepted, and the exception names the parent that brings it
  in. Either way, the exception carries a review date.
- **Deprecated npm packages.** A package version its maintainer marked
  deprecated MUST be replaced, or carry a recorded exception stating why it
  can't be replaced yet, whether it ships, its successor and a review date. A
  deprecated package that ships in the app as a direct dependency MUST be
  replaced. An exception past its review date fails the audit.
- **Yanked crates.** A yanked crate version MUST NOT be used, whether it ships
  or is build-only. It is never excepted.
- Every advisory exception MUST be scoped to the dependency path that was analysed, so
  the same advisory arriving by another path fails again, and MUST carry a
  date for review. It MUST be removed once a fix can be taken.

## Licensing

HoploDex is released under GPL-3.0-only. Every dependency, library, and asset
shipped with the application MUST be under a GPLv3-compatible license, as
checked by the license audit. This covers what the audit tools cannot see:
C code compiled inside a dependency, the crypto library the database links or
ships with, and bundled artwork, fonts, and data files, whose source and
license MUST be recorded before they are added. Development-only dependencies
that are not distributed are exempt. A license acceptable only for particular
packages (for example, a font license for fonts shipped as separate files)
MUST be recorded as an exception scoped to those packages, with the reason.
Every release MUST carry the copyright and license notices that its
dependencies' licenses require.

## Development Workflow & Quality Gates

Every pull request MUST pass automated linting, the full test suite, the
dependency audit (vulnerabilities, unmaintained and yanked crates, deprecated
packages, and licenses), and at least one peer review
before merge. Pull requests that touch UI MUST include before/after evidence
(screenshot or recording) demonstrating adherence to the shared design system.
Pull requests that touch data-handling or persistence code MUST call out, in
the description, how the change satisfies the Security & Data Handling
Constraints above. Performance-sensitive changes (queries, list rendering,
import/export) MUST include a note on expected impact against the budgets in
Principle IV.

Pull requests opened by Dependabot or other automation are held to the same
gates. A pull request that changes only Markdown documentation (`*.md` files
that neither the application nor its build includes) is exempt from the
automated gates (linting, the test suite, the dependency audit, and CodeQL)
and from the evidence and notes above, but not from peer review.

Security findings are recorded in the repository's GitHub security features
(code scanning, Dependabot and secret scanning alerts, and repository security
advisories), not in documents committed to the repository. Every pull request
to the main branch MUST be scanned by CodeQL and MUST NOT introduce a code
scanning alert at error level or of high severity or above; the repository
ruleset enforces this. An AI-assisted security review of a pull request's diff
is optional. When one is run, its findings about code the pull request adds or
changes MUST be uploaded to code scanning against that pull request, and a
vulnerability it notices in code a release already contains is handled as a
private advisory, as below.

Before every release, an AI-assisted security review of the whole codebase at
the release commit, not only a diff, MUST be run against the application's
attack surface as defined in `docs/attack-surface.md`, which is the one
definition of it: for each surface, the code to review, the questions the
review must answer, and the residual risks the project has accepted, which
the review reassesses. The list MUST describe the code at the release commit.
A pull request that adds, changes, or removes attack surface MUST update the
list in the same pull request, and an AI-assisted review of a pull request's
diff, when one is run, MUST say whether the attack surface changed and
propose the update when it did. A surface MAY be removed from the list only
when the code it describes is gone, with the reason stated in the pull
request. The list records the design, never findings: a finding goes to code
scanning, and a risk is listed as accepted only once the project has accepted
it where its decision is recorded. The review's findings MUST be uploaded to
code scanning. When fixes made in response to the review produce a new release
commit, that commit MUST get a carry-forward re-verification of the review's
findings and a review of the diff from the reviewed commit to it; a second
whole-codebase review is not required. A release MUST NOT proceed while any
critical or high code scanning, Dependabot, or secret scanning alert is open.
Each such alert MUST be fixed, or dismissed with its reason stated; a dismissed
dependency alert MUST match an exception recorded under the Security & Data
Handling Constraints above. The review MAY use Anthropic or another AI vendor.
It sends source code, never collection data, to that vendor, which is
consistent with Principle V.

Before the first release, security findings MAY also be tracked as ordinary
issues. After a release, a vulnerability that affects a released version MUST
be handled through a private repository security advisory that records the
affected and patched versions, and MUST NOT be described in a public issue or
pull request before a fixed release is available. The repository's
SECURITY.md MUST direct reporters to private vulnerability reporting.

Before every release, the license checks the tools cannot make MUST also be
done by hand, and the release MUST include its third-party license notices.

## Governance

This constitution supersedes all other development practices, style guides,
and informal conventions for this project. Any conflict between this document
and other guidance MUST be resolved in favor of this constitution until the
constitution itself is amended.

Amendments are proposed via pull request against this file, must include a
completed Sync Impact Report (as an HTML comment at the top of the file), and
require review approval before merge — the same bar as any other code change.
Versioning follows semantic versioning: MAJOR for backward-incompatible
principle removals or redefinitions, MINOR for new principles or materially
expanded guidance, PATCH for clarifications and wording fixes. Every pull
request MUST be checked against this constitution during review; unjustified
complexity or violations MUST be resolved or explicitly documented before
merge.

**Version**: 1.5.0 | **Ratified**: 2026-07-20 | **Last Amended**: 2026-11-02
