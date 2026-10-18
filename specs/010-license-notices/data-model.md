# Data Model: License Notices in the App and the License Audit's Last Items

**Feature**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md) | **Date**: 2026-10-18

No database layout changes. No database, setting or `machine.json` field is
added. This feature's data is build output and committed files:

- **Generated, not committed** (git ignores `dist/`): `dist/notices.json` and
  `dist/NOTICES.txt`, one pair per build, for that build's target.
- **Committed records**: the files under `licenses/`, which the build reads to
  produce them. All four JSON files must exist; an empty list is written `[]`,
  so a deleted file fails instead of dropping its notices.
- **Constants compiled into the interface**: the program block.

contracts/notices.md gives the exact formats. This file describes the
entities, their fields and rules, and how they relate.

## Entities

### Program block (spec FR-002)

Compiled into the interface with Vite `define`, and repeated in
`notices.json`.

| Field | Source | Rule |
|---|---|---|
| `name` | constant | "HoploDex" |
| `version` | `package.json` `version` | Equal to `APP_VERSION` (`tests/app_version_test.rs`) |
| `copyrightYears` | 2026 and the year of the build's commit | "2026" or "2026–YYYY". With no git, the build's own year (research.md §13) |
| `copyrightHolder` | constant | "exodious and the HoploDex contributors" (spec Clarifications) |
| `sourceUrl` | `package.json` `repository.url` | Without the `git+` prefix and `.git` suffix: `https://github.com/exodious/HoploDex` |

The license statement and the no-warranty statement are the interface's own
fixed text (contracts/ui.md §2). The additional permission comes from
`licenses/additional-permission.txt`, through `notices.json`.

### Notices document (`dist/notices.json`; spec FR-004, FR-008, FR-011)

One per build.

| Field | Meaning |
|---|---|
| `format` | `1`. The dialog refuses any other value, so a mismatch shows as the load failure, not as a wrong screen |
| `program` | The program block |
| `target` | The target triple the notices were made for |
| `gpl` | The full text of `LICENSE` |
| `additionalPermission` | `licenses/additional-permission.txt` |
| `components` | Component notices, ordered by name in lower case, then version |
| `texts` | A map from text id to text: each distinct text stored once |
| `development` | `true` only when served by the development server (research.md §10) |

### Component notice (spec Key Entities)

| Field | Meaning | Rule |
|---|---|---|
| `name` | Package, crate or component name | Not empty |
| `version` | Its version, as shipped | For OpenSSL on Windows, read from the build (research.md §7) |
| `kind` | `npm`, `crate`, `compiled-in`, `linked`, `font` or `data` | `font` for an `@fontsource*` package; `compiled-in`, `linked` and `data` for a hand-recorded entry |
| `license` | The SPDX expression HoploDex uses it under | A choice is resolved to one allowed license (research.md §5): the first allowed alternative, in the shared preference order, that the package (or an exception's committed texts) has a text for; a slash reads as `OR`. It must satisfy the shared allow list (the audit's `ALLOWED`, or a recorded exception such as OFL-1.1 for fonts, or ODC-BY for the dictionary's data) |
| `declared` | The expression the package declares, when it differs from `license` | Present only for a choice |
| `texts` | Ids into `texts`, in display order | At least one. NOTICE and attribution texts follow the license text |
| `source` | Where the texts came from: `package`, `exception`, `clarified` or `hand-recorded` | |
| `carrier` | For a hand-recorded component, `{ kind, name, version }` of the shipped package that carries it | |

A component that two of the three OSes ship appears in each of those builds'
documents. Each document holds its own build's components only (spec Edge
Cases).

### Hand-recorded component (`licenses/hand-recorded.json`; spec FR-009, FR-016)

| Field | Rule |
|---|---|
| `name`, `version`, `license`, `kind` | As a component notice, with `kind` one of `compiled-in`, `linked` or `data`. `version` may be `"from-build"` (OpenSSL) |
| `os` | One or more of `linux`, `windows`, `macos` (never empty) |
| `carrier` | `{ "npm" \| "crate": name, version, feature? }`: exactly one of `npm` and `crate`, and a `version` |
| `texts` | At least one path: `licenses/texts/…` (committed), or `package:<path>` inside the carrier's folder |
| `reason` | Why the tools can't see it |

**Check** (research.md §7) fails, naming the entry, when:
- the carrier isn't shipped on a listed OS;
- the carrier's version differs from the recorded one;
- the feature isn't enabled;
- a text file is missing, or `texts` is empty;
- `os` is empty or names another operating system, or `kind` is another value;
- for OpenSSL on Windows, the version read from the build is below 3.

### Notice exception (`licenses/exceptions.json`; spec FR-010)

| Field | Rule |
|---|---|
| `component` | `crate:<name>` or `npm:<name>` |
| `checkedVersion` | The version the text was checked at |
| `declared` | The license expression declared at that version |
| `texts` | At least one committed file under `licenses/texts/` |
| `from` | URL of the file taken, at that version's tag or commit |
| `reason` | e.g. "the published crate leaves out its license file" |

**Check** fails when:
- the declared expression changes;
- the component now ships a usable license file of its own (the exception is
  stale; remove it);
- a text file is missing, or `texts` is empty.

When the component isn't shipped by any target, the check only warns: the
same exception covers several OSes, and one OS's build has nothing to say
about the others. The `--all-targets` audit fails on an exception that no
target uses.

### Clarification (`licenses/clarify.json`)

| Field | Rule |
|---|---|
| `component` | As above |
| `version` | The version it was written for |
| `files` | At least one `{ path, sha256 }` in the package, or committed `licenses/texts/` paths, or `{ text }`, used in place of what the matcher finds |
| `reason` | |

**Check** fails when a listed file's SHA-256 differs, a listed file is
missing, or `files` is empty.

### Native-code review (`licenses/native-code.json`; research.md §7)

| Field | Rule |
|---|---|
| `crate` | Crate name |
| `what` | `hand-recorded:<entry name>` (an entry in `hand-recorded.json`), `own-code`, `import-libraries` or `system-library` |
| `note` | One line |

**Check** fails, naming the crate, when a shipped crate:
- has an enabled `cc` or `cmake` build dependency, or
- carries a `*.lib`, `*.a`, `*.dll`, `*.dylib`, `*.so` or `*.o` file,

and isn't listed. It warns when a listed crate no longer ships on any target
(the `--all-targets` audit fails on it).

### Shipped set (spec FR-008, FR-012)

This isn't stored; it is computed by `scripts/licenses/shipped.mjs`, the one
definition that the audit and the notices share:

- **npm**: lockfile packages with neither `dev` nor `devOptional`.
- **crates**: `cargo tree --locked -e normal,no-proc-macro --target <triple>
  --features custom-protocol`, less path packages.

The build adds one more rule: every package in the main bundle and the
strength worker's bundle must be in the npm shipped set.

### Audit record (spec Key Entities, FR-013)

Prose in [research.md](./research.md) §1 to §4, one row per part, dated. It
isn't machine-read.

## The About dialog's states (spec US1)

```text
closed ──showAbout()──▶ open: loading ──fetch ok──▶ open: ready
                           │                          │
                           └──fetch fails──▶ open: failed (program block still shown)
any phase change (open, lock, close, switch) ──▶ closed
  (except the window's start, starting ──▶ chooser, which leaves it open)
```

- **Loading and failed** still show the program block, from the compiled-in
  constants, so FR-002's statement never waits on the fetch.
- **Ready**: the GPL disclosure, closed. The component list, with its filter
  empty and every row closed.
- **The filter**: a case-insensitive substring of `name`, trimmed. A filter
  with no match shows the no-match line.
- **A development build's document** (`development: true`): the program block
  ends with the muted line "Development build: these notices aren't checked."
  A build's document has no such field, and `false` shows nothing.
- **Ready**'s focus: on the license disclosure's button, once the notices
  have loaded (the button doesn't exist while loading).
