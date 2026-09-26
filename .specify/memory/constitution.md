<!--
Sync Impact Report
Version change: 1.0.0 → 1.1.0
Bump rationale: MINOR. Two materially expanded sections (a dependency
vulnerability policy and two new quality gates); the backup clarification on
its own would be a PATCH.
Modified principles: none
Modified sections:
  - Security & Data Handling Constraints: the opt-in rule for "any network sync
    or backup feature" clarified to cover only data that leaves the device, so
    local backups may be on by default (needed by feature 003, FR-024); a
    dependency vulnerability policy added (critical, high, unscored and
    unmaintained/unsound advisories; scoped, dated exceptions; dev-only
    dependencies held to the same bar)
  - Development Workflow & Quality Gates: "the dependency audit passes" added to
    the pull request gates; a whole-codebase AI-assisted security review added
    as a release gate, which may use Anthropic or another AI vendor and sends
    source code, never collection data, to it
Added sections: none
Removed sections: none
Templates requiring updates:
  - .specify/templates/plan-template.md: ✅ no change needed (Constitution Check
    is generic and reads from this file)
  - .specify/templates/spec-template.md: ✅ no change needed
  - .specify/templates/tasks-template.md: ✅ no change needed
  - .specify/templates/checklist-template.md: ✅ no change needed
  - DEVELOPMENT.md "Dependency audit": ✅ already describes the enforced policy
  - CLAUDE.md "Spec Kit workflow" gate list: ⚠ pending, add the dependency audit
    and the release security review
Follow-up TODOs:
  - No written release process exists yet for the release security review to
    live in; add one (a release checklist) with the release engineering work.
  - The release security review has no tooling yet (a project skill that
    reviews the whole codebase and writes the dated report).
  - Feature 003's spec, plan and research describe the backup rule as needing
    clarification; update them to cite constitution 1.1.0 once this merges.
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

**Rationale**: This app is the system of record for a user's firearm
collection, often tied to legal, insurance, and safety obligations; undetected
regressions have real-world consequences beyond typical inconvenience.

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
- A vulnerability advisory with no severity score counts as high. Notices that
  a Rust crate is unmaintained or unsound also block, and are cleared the same
  way as a high advisory.
- Every exception MUST be scoped to the dependency path that was analysed, so
  the same advisory arriving by another path fails again, and MUST carry a
  date for review. It MUST be removed once a fix can be taken.

## Development Workflow & Quality Gates

Every pull request MUST pass automated linting, the full test suite, the
dependency audit, and at least one peer review before merge. Pull requests
that touch UI MUST include before/after evidence (screenshot or recording)
demonstrating adherence to the shared design system. Pull requests that touch data-handling or persistence
code MUST call out, in the description, how the change satisfies the Security
& Data Handling Constraints above. Performance-sensitive changes (queries,
list rendering, import/export) MUST include a note on expected impact against
the budgets in Principle IV.

Before every release, an AI-assisted security review of the whole codebase,
not only a diff, MUST be run against the application's attack surface: the
commands exposed to the frontend, the Tauri capabilities and content security
policy, filesystem path handling, spreadsheet import parsing and export
(including formula injection), handling of the database key and passphrase,
secure deletion, and decrypted document copies. Its dated report MUST be
committed to the repository, and every critical or high finding MUST be fixed,
or justified in the report, before the release. The review MAY use Anthropic
or another AI vendor. It sends source code, never collection data, to that
vendor, which is consistent with Principle V. A diff-scoped security review of
individual pull requests is optional.

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

**Version**: 1.1.0 | **Ratified**: 2026-07-20 | **Last Amended**: 2026-09-26
