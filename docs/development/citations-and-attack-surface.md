# Spec citations and the attack surface list

_Part of the [development docs](../../DEVELOPMENT.md)._

## Spec citations

Spec Kit numbers requirement, success-criterion, story, scenario and task IDs
within each spec, and each spec has its own `research.md`, `plan.md`,
`data-model.md`, `quickstart.md`, `tasks.md` and contracts. So outside a
spec's own folder a citation names its spec first (#121):

| Cites | Form |
|---|---|
| A requirement or success criterion | `003 FR-040`, `006 SC-002` |
| A user story | `001 US2` |
| An acceptance scenario (story 3, scenario 2) | `007 US3-2`, `003 US3-4a` |
| A task | `008 T050` |
| A document, or one of its sections | `007 research.md §5`, `006 contracts/ui-accessories.md §3`, `003 data-model.md` |

One spec number covers the list that follows it, joined by commas, "and",
"or", "to" or a dash: `(006 FR-001, FR-002 and SC-003)`,
`003 research.md §1, §11`, `008 T005-T008`. A citation of another spec within
the list takes its own number: `(006 FR-006; 001 FR-004)`. A path into a spec
folder (`specs/007-document-preview/research.md`) names its spec already.
"User Story 2" and "Scenario 3" are always written `001 US2` and `001 US1-3`.
Inside `specs/NNN-name/`, bare IDs mean that spec, as Spec Kit writes them.

`scripts/citations.mjs` checks this in `src/`, `src-tauri/`, `e2e/`,
`scripts/` and `tools/` (not `src-tauri/vendor/`), and that each cited
requirement, story, scenario, task, document and section exists in the spec
it names. It also checks the E2E `describe()` titles ([E2E files](e2e.md#e2e-files)). `npm test` runs it
(`scripts/citations.test.ts`); to check a few files while writing, run
`node scripts/citations.mjs <file> …`. Passing it means a citation names a
spec that has that ID, not that it is the right spec: FR-001 to FR-017 exist
in every one, so check what the cited text says.

## Attack surface list

`docs/attack-surface.md` is the one definition of the attack surface the
release security review, and any per-PR AI review, reads (constitution,
"Development Workflow & Quality Gates"). A pull request that adds, changes or
removes surface updates it in the same pull request: a new entry, or a new
**Code** path, **Ask** question or **Accepted** risk in one, and fixes for
paths it renamed or removed. Surface ids are permanent, since code scanning
alert ids start with them: rename an entry's title, never its id.

`scripts/attack-surface.mjs` checks the list, and `npm test` runs it
(`scripts/attack-surface.test.ts`); run `node scripts/attack-surface.mjs`
while editing. It fails when a **Code** path or glob matches no file, when a
backend, build, configuration, workflow or Flatpak file (its `PRODUCTION`
list) is claimed by no entry, or when a command in `UNSCOPED_COMMANDS` or a
Cargo feature isn't named in the list. Place a new file in the entry it
belongs to, or in `not-surface` if it is none. The check can't see new surface
inside a file an entry already claims, such as a new command argument or
parser, so look for that yourself.
