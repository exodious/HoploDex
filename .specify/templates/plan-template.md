# Implementation Plan: [FEATURE]

**Branch**: `[###-feature-name]` | **Date**: [DATE] | **Spec**: [link]

**Input**: Feature specification from `/specs/[###-feature-name]/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

[Extract from feature spec: primary requirement + technical approach from research]

## Technical Context

<!--
  ACTION REQUIRED: Replace the content in this section with the technical details
  for the project. The structure here is presented in advisory capacity to guide
  the iteration process.
-->

**Language/Version**: [e.g., Python 3.11, Swift 5.9, Rust 1.75 or NEEDS CLARIFICATION]

**Primary Dependencies**: [e.g., FastAPI, UIKit, LLVM or NEEDS CLARIFICATION]

**Storage**: [if applicable, e.g., PostgreSQL, CoreData, files or N/A]

**Testing**: [e.g., pytest, XCTest, cargo test or NEEDS CLARIFICATION]

**Target Platform**: [e.g., Linux server, iOS 15+, WASM or NEEDS CLARIFICATION]

**Project Type**: [e.g., library/cli/web-service/mobile-app/compiler/desktop-app or NEEDS CLARIFICATION]

**Performance Goals**: [domain-specific, e.g., 1000 req/s, 10k lines/sec, 60 fps or NEEDS CLARIFICATION]

**Constraints**: [domain-specific, e.g., <200ms p95, <100MB memory, offline-capable or NEEDS CLARIFICATION]

**Scale/Scope**: [domain-specific, e.g., 10k users, 1M LOC, 50 screens or NEEDS CLARIFICATION]

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

[Gates determined based on constitution file]

## Test Levels

*Every acceptance scenario needs a test (constitution II), but not an E2E one.
E2E is the slowest layer and runs on every OS, some of them small VMs, so place
each scenario at the cheapest level that can show it, and say why wherever the
plan departs from these defaults.*

- **E2E, one journey per user story**: the story's main path through the real
  app, keyboard-first where the story calls for it.
- **E2E, one test per error mechanism per form or dialog**, not one per rule:
  a field error (the backend's `fieldErrors` land on the right field), an
  overridable warning (the `confirmedWarnings` resend), and a refusal whose
  domain code the UI turns into its own message or dialog. A form that already
  has a mechanism's test needs no new one for a new rule, and a form whose
  every rule is also checked in the UI can't reach the backend's field errors,
  so it has none (Vitest shows how it would display them).
- **E2E, what only the real app shows**: IPC wiring, relaunch, locking, sleep
  and shutdown, native file drops, focus and real-input behaviour, the PDF
  surface, and the screenshot walk (a new screen joins
  `e2e/screenshots/screens.e2e.ts`).
- **Security**: test a security property where it is enforced (Rust
  integration tests for the backend's rules) and, when it depends on the IPC
  boundary, the web view, the file system or the OS (session scoping, browser
  controls, what is left on disk, the CSP and capabilities, confinement), also
  in the real app, by E2E or a dedicated check. A security test never moves to
  a cheaper level.
- **Everything else** (validation, duplicates, rounding, dates and other edge
  cases): Rust `ops` integration tests on a real database for persistence and
  rules, Vitest for UI behaviour. Cite the scenario (`US1 Scenario 9`) in the
  test's name or a comment, so each scenario's test can be found.

| Story | E2E journey | Other E2E (error mechanism, real app, security) | Scenarios tested in Rust or Vitest |
|-------|-------------|-------------------------------------------------|------------------------------------|
| [US1] | [path walked] | [each test and why it needs the real app] | [scenario IDs → level] |

## Project Structure

### Documentation (this feature)

```text
specs/[###-feature]/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)
<!--
  ACTION REQUIRED: Replace the placeholder tree below with the concrete layout
  for this feature. Delete unused options and expand the chosen structure with
  real paths (e.g., apps/admin, packages/something). The delivered plan must
  not include Option labels.
-->

```text
# [REMOVE IF UNUSED] Option 1: Single project (DEFAULT)
src/
├── models/
├── services/
├── cli/
└── lib/

tests/
├── contract/
├── integration/
└── unit/

# [REMOVE IF UNUSED] Option 2: Web application (when "frontend" + "backend" detected)
backend/
├── src/
│   ├── models/
│   ├── services/
│   └── api/
└── tests/

frontend/
├── src/
│   ├── components/
│   ├── pages/
│   └── services/
└── tests/

# [REMOVE IF UNUSED] Option 3: Mobile + API (when "iOS/Android" detected)
api/
└── [same as backend above]

ios/ or android/
└── [platform-specific structure: feature modules, UI flows, platform tests]
```

**Structure Decision**: [Document the selected structure and reference the real
directories captured above]

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| [e.g., 4th project] | [current need] | [why 3 projects insufficient] |
| [e.g., Repository pattern] | [specific problem] | [why direct DB access insufficient] |
