# Implementation Plan: Session Isolation, Safe Import, Browser Controls and Pending Changes from Another Version

**Branch**: `008-hardening-batch` | **Date**: 2026-10-11 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/008-hardening-batch/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

Four issues, each a story that can be built and tested on its own:

- **#69, session isolation (P1)**: every open of a database gets a
  session id.
  - **Backend**: every request about the collection carries the id in an IPC
    header. The backend checks it under the session's lock and refuses an
    ended session's request, reads included (`DATABASE_CLOSED`).
  - **Frontend**: a `SessionScope` per open owns the session's thumbnail
    cache, resumed draft, staging and pending work. Its `invoke` stamps the
    header and drops late responses. The toasts move inside the tree keyed
    by the open, so they go with it.
  - **Resumed pending changes** stay in the database until their form has
    actually opened; a form that can't open brings the dialog straight
    back.
- **#66, safe import (P2)**: files are read in a confined helper process,
  the HoploDex executable started again with `--import-helper`. It shares
  007's TIFF helper core (confinement, job object, framing), with a 512 MiB
  memory cap.
  - Limits on file size, unpacked size, sheets, rows, columns, cell text,
    total text and shared strings are enforced as the data arrives.
  - The reading holds no session lock, shows progress, and stops within 1 s
    on **Stop reading** or a lock.
  - Free-text fields get maximum lengths (200 and 4,000 characters), so
    the largest collection, and the limits sized against it, are defined.
- **#40, browser controls (P3)**, in release and E2E builds:
  - a page-level layer cancels the context menu outside editable text and
    selections, and cancels the reload, navigation, print, save, find,
    view-source and developer-tools keys and the mouse's back and forward
    buttons;
  - a native filter on each OS trims the menus that remain to editing,
    copying, spelling and emoji;
  - WebView2's browser accelerator keys and swipe navigation are off.

  Development builds keep the web view's menu and reload.
- **#78, pending changes from another version (P4)**:
  - `app_state.last_saved_version` records the version that last saved the
    database, atomically with pending changes when they are written.
  - The pending changes dialog names that version.
  - The dialog offers **Close the database** in every case, which leaves
    the changes for the next open.

**Decisions taken at planning** (research.md):
- **The owner's** (2026-10-11): free-text field maximums (§10), and the
  import read in a confined helper (§7, §8). Both are recorded in spec.md's
  clarifications.
- **Mine, worth a look**: listed under "Findings confirmed with the user"
  below.

## Technical Context

**Language/Version**: Rust 1.97+ (edition 2024, `src-tauri`), TypeScript 5.x /
React 18 (`src`), unchanged

**Primary Dependencies**: no new crate or npm package (research.md §21).
- `calamine` 0.36, `csv` 1.4, `zip` 8.6 and `quick-xml` 0.41 stay at their
  locked versions and now run inside the import helper.
- `webkit2gtk` (Linux), `objc2`/`objc2-app-kit`/`objc2-web-kit` (macOS) and
  `webview2-com` (Windows) are already direct dependencies from 007; they
  gain uses on the main web view.
- Tauri 2.11's `ipc::CommandArg` and `InvokeOptions.headers` carry the
  session id.

**Storage**: the existing encrypted SQLCipher database. `0001_initial.sql` is
edited in place to add `app_state.last_saved_version`, so existing
development databases must be recreated (CLAUDE.md "Schema changes"). No
other schema change; the field maximums are validation, not `CHECK`s.
`machine.json` is unchanged.

**Testing**:
- **cargo**: integration tests against real temporary databases and the
  real helper binary.
  - New: `session_scope_test.rs`, `pending_resume_test.rs`,
    `last_saved_version_test.rs`, `import_reader_test.rs` with a generated
    hostile corpus, `text_limits_test.rs`, `app_version_test.rs`, and a
    release-only `import_largest_test.rs`.
  - Amended: `lock_test.rs`, `tiff_helper_test.rs` (the moved helper core)
    and every test that calls a session accessor (ids added).
- **Vitest**: the scope, `SessionProvider`, thumbnails, batches, toasts, the
  resume lifecycle, `PendingChangesDialog`, `browserControls` and the
  `TextArea` counter.
- **WebdriverIO**:
  - new `us14-session-isolation` and `us15-browser-controls` specs, with
    real input on Linux;
  - `us9` and `us5` extended;
  - the screenshot walk.
- **The window controls check** (new): one per OS, like the PDF surface
  check. It covers SC-005 where the E2E specs have no real input.
- **Manual**: 3 numbered procedures, best effort before a release
  (quickstart.md).

**Target Platform**: Desktop: Windows 10+, macOS 26+, Linux (WebKitGTK 2.40+),
unchanged.

**Project Type**: Desktop application (Tauri: React frontend + Rust backend in
one repo)

**Performance Goals**:
- The session check adds an integer comparison per command, under a mutex
  the command already takes. Search ≤ 500 ms and actions ≤ 1 s at 10,000
  items still hold (`performance_test.rs` unchanged apart from ids).
- Crafted files are refused within 5 s (SC-002).
- Cancel and lock take effect within 1 s while reading (SC-004).
- Progress shows from the import's first byte.

**Constraints**:
- **Memory** while reading any one file: the HoploDex process grows by at
  most 512 MB. The helper is capped at 512 MiB on Linux and Windows.
- **No crash from any file**: a panic or a failed allocation in the reader
  ends only the helper.
- **No session data across sessions**, in the frontend or through a late
  request.
- **Development builds** keep their tools.
- **No cipher or key change.**

**Scale/Scope**: four user stories (P1 to P4), 17 functional requirements, 6
success criteria.
- **Interface**: 1 new command (`cancel_import`); a session header on the
  64 scoped commands (14 stay unscoped); 4 commands amended; 3 events amended; 1 new
  error code (`IMPORT_LIMIT_EXCEEDED`); 1 new process mode.
- **Schema**: 1 column.
- **Validation**: 24 fields gain a maximum.
- **UI**: the pending dialog and import dialog amended; a `TextArea` counter;
  no new screen (screenshot variants only).
- **Native**: one main-window filter per OS.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Requirement | How this plan satisfies it |
|---|---|---|
| I. Code Quality | Lint/static analysis, review, small single-purpose modules, no speculative abstraction | **One rule, one place**: the session check lives in `Session`'s accessors (under the mutex) and is reached through one argument type, `ScopedSession`. `UNSCOPED_COMMANDS` is the one exception list, and a test keeps it honest. The limits are one module (`import_reader::limits`), the text maximums one module (`models/rules.rs`, mirrored in `src/lib/textLimits.ts`), the browser controls one module per side. **Reuse, not a second copy**: the import helper shares 007's helper core, which moves to `services/helper/`, rather than duplicating confinement. **Refactor, not workaround**: `Session::read` and friends take the id and every caller is updated; `generation` becomes `session_id` and `with_generation` is folded into the accessors; services take the scope as a parameter and every caller and mock is updated; `import_collection_stoppable` takes tables read rather than paths. No `_with_x` siblings. `clippy`, `rustfmt`, `eslint` and `prettier` run locally. Full CI is #25, out of scope |
| II. Testing (NON-NEGOTIABLE) | Tests first, real persistence, one test per acceptance scenario, test isolation | Every scenario and criterion maps to a named test (quickstart.md). The two-database scenarios use real databases seeded through `ops` with colliding ids. The crafted files are generated at test time and run through the real helper binary. Regression tests for each #69 path (thumbnail, draft, batch, toast) and each #66 path (`uniqueCount`, dense grid, CSV field, the lock waiting) fail before the fix. E2E, screenshots and the window controls check run in the sandbox; the new seed option writes only into `--dir`. Three manual procedures, numbered and naming "Main collection", "Shared collection" and `human testing passphrase` |
| III. UX Consistency | One component set and pattern, WCAG 2.1 AA | The pending dialog keeps its pattern and gains a secondary button, as other dialogs order danger, secondary, primary. The import dialog's reading phase uses the existing `ProgressBar` and error banner. The text maximums follow the entry fields' existing message and immediate check, carried to every form with free-text fields (FirearmForm, AccessoryForm, DisposeDialog, InsurancePolicyForm). The counter is a `TextArea` prop, not a one-off. Right-click behaves as in a native app. Accessibility: the counter is announced politely, and the dialog's focus defaults never land on Discard |
| IV. Performance | 100 ms feedback / 1 s completion, 500 ms search, no UI-thread blocking | The session check is constant time. The import's reading moves off the session lock and shows progress from the start; parsing is in another process. Cancel and lock act within 1 s. The largest export's import time is recorded (`import_largest_test.rs`). The context-menu and key listeners are O(1) per event. **Import is performance-sensitive**: the PR notes the measured reading time and memory for the largest export and for the crafted corpus |
| V. User Privacy | Local only, real deletion, no hidden copies | Session data is forgotten at the session's end (FR-001), extending 003 FR-033 to the frontend. The helper receives the file over a pipe and writes nothing; its core dumps are off, and it has no file or network access. Selected text can no longer be sent to Look Up, Translate, web search, Share or Services from the main window. The last-saved version is a build number, not personal data |
| Security & Data Handling | Encryption at rest, vetted dependencies, no cipher change | No cipher, key or passphrase change. The session check is enforced on the trusted side (research.md §2). Untrusted spreadsheets are confined (§8). The PR's data-handling notes come from research.md §20. No new dependency to vet |
| Licensing | GPLv3-compatible dependencies and bundled assets with recorded source | Nothing new is bundled or depended on (research.md §21) |

**Result**: PASS, with the entries in Complexity Tracking.

## Project Structure

### Documentation (this feature)

```text
specs/008-hardening-batch/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   ├── tauri-commands.md      # session header and unscoped list; DatabaseStatus.sessionId;
│   │                          #  PendingSummary versions; resolve_pending_changes actions;
│   │                          #  import errors and progress; cancel_import; events; --import-helper
│   ├── ui.md                  # pending dialog (amends 003 §13); import reading; text maximums;
│   │                          #  right-click and keys; screenshot walk
│   └── spreadsheet-format.md  # import limits; free-text maximums by column
├── checklists/
│   └── requirements.md
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

The contracts are **deltas**. As in earlier features, the final phase adds
a one-line `_Amended by [spec 008](...)_` note at each amended anchor:
- **003**:
  - FR-033 (extended to what the interface holds) and FR-039 (resume kept
    until the form opens; Close the database);
  - `contracts/ui-databases.md` §13;
  - `contracts/tauri-commands.md`: `DatabaseStatus`, `PendingSummary`,
    `resolve_pending_changes`, `close_database`, the events, and "Changes to
    every existing command" (the session header);
  - `data-model.md`'s `app_state` and its pending changes states;
  - `research.md` §16, the pending changes' lifecycle.
- **001** and **006**: `import_collection` (limits, the reading phase,
  cancel) in their command contracts; "Import behavior" and "Recognising a
  table on import" in the spreadsheet contracts; FR-019, FR-020 and 006
  FR-022 in the specs. The field maximums go beside each form's field list.
- **007**: `research.md` §11, the helper core moved to `services/helper/`.
  Paths there and in 007's plan and tasks are renamed in place, as a
  mechanical rename.

### Source Code (repository root)

This is the existing Tauri layout. Only files that change or are added are
listed.

```text
src-tauri/
├── capabilities/default.json        # + allow-cancel_import
├── src/
│   ├── main.rs                      # --import-helper checked first; window_controls applied to "main";
│   │                                #  cancel_import registered
│   ├── lib.rs                       # APP_VERSION
│   ├── commands_list.rs             # + cancel_import; UNSCOPED_COMMANDS
│   ├── db/
│   │   ├── migrations/0001_initial.sql  # app_state.last_saved_version
│   │   └── mod.rs                   # record_saved_version(); set at creation
│   ├── session/
│   │   ├── mod.rs                   # SessionId; current id; accessors take the id; PendingState;
│   │   │                            #  write_checked records the version when rows changed
│   │   ├── scoped.rs                # NEW: ScopedSession (CommandArg from the HoploDex-Session header)
│   │   ├── pending.rs               # resume/opened/notOpened/discard; write_pending records the version
│   │   └── lifecycle.rs             # internal callers pass current_id(); events carry sessionId
│   ├── commands/
│   │   ├── *.rs                     # State<Session> → ScopedSession in every scoped command
│   │   ├── databases.rs             # PendingSummary versions; resolve actions
│   │   └── import_export.rs         # reading through import_reader outside the lock; cancel_import;
│   │                                #  import_collection_stoppable takes read tables
│   ├── models/
│   │   ├── rules.rs                 # MAX_SHORT_TEXT_CHARS, MAX_LONG_TEXT_CHARS, check_text_length
│   │   ├── firearm.rs, accessory.rs, insurance_policy.rs  # call it (and commands/firearms.rs's
│   │   │                            #  DisposeInput, for the recipient)
│   │   └── database.rs              # PendingSummary, PendingAction, DatabaseStatus.session_id
│   ├── services/
│   │   ├── helper/                  # NEW home of 007's helper core: confine.rs (memory cap a parameter),
│   │   │                            #  job.rs, frames.rs, handle.rs, mod.rs
│   │   ├── preview/                 # helper*.rs and confine.rs moved out; TIFF mode keeps its requests
│   │   ├── import_reader/           # NEW: limits.rs, reader.rs (helper side: zip walk, counted inflate,
│   │   │                            #  sst pre-read, streaming cells, CSV), client.rs (parent side),
│   │   │                            #  frames.rs (import frames)
│   │   └── spreadsheet.rs           # read_csv/read_xlsx removed; read_table/recognise kept for the parent
│   └── window_controls/             # NEW: mod.rs, linux.rs (context-menu allowlist), macos.rs
│                                    #  (willOpenMenu: hook), windows.rs (ContextMenuRequested allowlist,
│                                    #  browser accelerator keys and swipe off); release and e2e only
├── examples/
│   ├── human_seed.rs                # --pending-from <version>; last_saved_version on the seeded pending;
│   │                                #  an over-limit sample in import-samples (20 sheets)
│   ├── window_controls_check.rs     # NEW: the main window with window_controls + real input + menu log
│   └── pdf_surface_input.{py,swift,ps1}  # renamed real_input.*, shared by both checks
└── tests/
    ├── session_scope_test.rs        # NEW
    ├── pending_resume_test.rs       # NEW
    ├── last_saved_version_test.rs   # NEW
    ├── import_reader_test.rs        # NEW (with support/hostile_spreadsheets.rs)
    ├── import_largest_test.rs       # NEW, release + --ignored
    ├── text_limits_test.rs          # NEW
    ├── app_version_test.rs          # NEW
    ├── tiff_helper_test.rs, tiff_preview_test.rs, support/preview_support.rs  # helper core's new paths
    ├── lock_test.rs                 # + reading during lock, screen lock, sleep
    └── (every test using Session's accessors)  # ids added

src/
├── main.tsx                         # installs browserControls in production
├── App.tsx                          # ToastProvider moved into SessionProvider
├── services/tauriClient.ts          # invoke(command, args, { session })
├── lib/textLimits.ts                # NEW
├── components/TextArea.tsx          # limit prop and counter
├── features/
│   ├── app/browserControls.ts       # NEW (+ test)
│   ├── app/AppShell.tsx             # resumed route from the scope
│   ├── session/
│   │   ├── sessionScope.ts          # NEW (+ test)
│   │   ├── SessionProvider.tsx      # scope per open, ended on events; ToastProvider; Opening… state;
│   │   │                            #  Close the database from the dialog
│   │   ├── PendingChangesDialog.tsx # versions in the wording; Close; Opening…; failure line
│   │   ├── usePendingDraft.ts       # resumed draft and staging through the scope
│   │   └── sessionService.ts        # scope parameter; resolve actions
│   ├── browse/FirearmThumbnail.tsx  # cache in the scope
│   ├── media/PhotoGallery.tsx, DocumentList.tsx  # batches stop at the scope's end
│   ├── firearms/, accessories/, insurance/   # record pages report notOpened; forms' text maximums
│   ├── import-export/ImportDialog.tsx        # reading phase, Stop reading, limit messages
│   └── */*Service.ts                # scope as the first parameter

e2e/
├── specs/us14-session-isolation.e2e.ts  # NEW
├── specs/us15-browser-controls.e2e.ts   # NEW (real input on Linux)
├── specs/us9-locking.e2e.ts             # + pending from another version
├── specs/us5-export-import.e2e.ts       # + refused file, Stop reading
└── screenshots/screens.e2e.ts           # contracts/ui.md §5

scripts/
├── human-testing.sh                     # --release (the E2E-profile build in the sandbox), --pending-from
├── window-controls-check.sh             # NEW (Linux, Xvfb + XTest)
├── macos/window-controls-check.sh       # NEW (the macOS 26 VM)
└── windows/window-controls-check.ps1    # NEW

DEVELOPMENT.md                           # the window controls check; human-testing --release, --pending-from
CLAUDE.md                                # session scope, ScopedSession, UNSCOPED_COMMANDS; services/helper/,
                                         #  import_reader/; window_controls/; field maximums
```

**Structure Decision**: no new project or layer.
- **Rust** owns every enforcement: the session check, the pending state
  machine, the version, the limits, and the helper and its confinement. It
  also owns the native menu filters.
- **React** owns the scope's lifetime, what it forgets, the dialogs, and
  the page-level browser controls.
- **E2E specs** continue the `us` series (`us14`, `us15`).

**First tasks**, because later ones depend on them holding:
1. **The header reaches commands** on Linux, macOS and Windows, through the
   custom-protocol IPC and the `postMessage` fallback, with a
   `ScopedSession` on one command and the full E2E suite passing (research.md
   §2).
2. **macOS's `willOpenMenu:withEvent:` hook** on WryWebView in macOS 26:
   check which items it can remove from a text field's and a selection's
   menu. Writing Tools and Services are the likely ones it can't. If any
   item of FR-011's excluded list can't be removed, stop and ask the owner
   to choose among keeping that item, HoploDex's own Radix menu, or no menu
   on macOS (research.md §13; spec Assumptions).
3. **The helper core moved** to `services/helper/` with the TIFF tests
   (`tiff_helper_test.rs`, the confinement self-check) passing on all
   three OS, before the import mode is added.

### Close-out (at pull request time)

The final phase of tasks.md, done as part of opening the pull request:

- **#69**: link the pull request (it closes the issue on merge). Reply to
  each finding:
  - the thumbnail cache now lives in the session's scope;
  - the resumed draft is kept in the database until its form opens, and
    matched only within its session;
  - attachment batches stop at the session's end, and the backend refuses
    them;
  - toasts are keyed by the open;
  - every request carries the session id and the backend checks it under
    the lock, reads included.

  Name the two-database regression tests.
- **#66**: link the pull request (it closes the issue). Reply:
  - the reader runs in the confined `--import-helper`, with the limits of
    research.md §9;
  - calamine's `uniqueCount` and dense-grid paths are refused, or end only
    the helper;
  - the reading is outside the session lock and cancellable.

  Name the hostile corpus tests, and give the measured times and memory.
- **#40**: link the pull request (it closes the issue). Comment with what
  each OS does (research.md §13, §14), the window controls check's results
  on all three, and which manual checks were run.
- **#78**: link the pull request (it closes the issue). Comment that the
  version is recorded in `app_state.last_saved_version` independent of the
  pending changes, as the owner asked; that the dialog names it; and that
  Close the database is offered in every case.
- **#24** (versioning policy): comment that `app_state.last_saved_version`
  now exists, and that showing it elsewhere or warning about a newer saver
  is left to #24 (spec Assumptions).
- **#73** (web view navigation): comment that reload and history keys,
  buttons and menu items are now blocked in the main window, but its
  navigation handling itself is unchanged, so #73 stays open.
- **#21** (release security review): comment that the review must cover:
  - `ScopedSession` and `UNSCOPED_COMMANDS`;
  - the frontend scope;
  - the import helper, its confinement, limits and frames;
  - `window_controls` on each OS.

  List the residual risks of research.md §22.
- **A new issue**: "Workbook export fails for a cell over 32,767
  characters", for `photo_filenames` with more than about 270 photos on
  one record (research.md §9).
- **The PR body** carries `/security-review`'s result (the repository's
  hook requires it) and the data-handling notes of research.md §20.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|---|---|---|
| A second helper process mode (`--import-helper`), and the helper core moved to a shared module | FR-007's "however crafted, including parts read by third-party components" in a process that aborts on panic; SC-002's memory bound; SC-004's 1 s cancel. Owner's choice, 2026-10-11 | In-process hardening leaves unknown calamine panics able to end the app; a vendored calamine has the same gap plus a fork to maintain (research.md §7) |
| Per-OS native code on the main web view (a context-menu filter on each OS; a runtime-added AppKit method on macOS) | FR-011 asks for the system's menu, trimmed; a page can only cancel a menu outright, not remove Look Up or Reload from it | Our own Radix menu loses the system's spelling suggestions and emoji, which #40 and the spec keep; it stays as macOS's fallback if the hook can't remove an item (research.md §13) |
| A parameter on every collection service function and a header on every scoped command | FR-004 requires every request to name the session it was begun in; only a value captured when the work began can (research.md §3) | A module-level "current session" names the session open when the request is sent, which is #69's attachment bug |
| Free-text maximums across four forms and the import (a rule the spec didn't state) | SC-003 needs a "longest" to size the limits against; a note over 32,767 characters already breaks the workbook export. Owner's choice, 2026-10-11 | Unbounded fields leave SC-003 without a size and the limits arbitrary (research.md §10) |

## Post-Design Constitution Check

*Re-evaluated after Phase 1 (research.md, data-model.md, contracts/,
quickstart.md).*

- **Code Quality**: each rule has one home.
  - The session check: `Session`'s accessors, reached only through
    `ScopedSession` or `current_id()`.
  - The exceptions: `UNSCOPED_COMMANDS`, with its test.
  - The limits and the text maximums: one module each.
  - The menu allowlists: one file per OS.
  - The helper core: shared by TIFF and import.

  No sibling variants: accessors, services and the import entry point
  change shape, and their callers follow. Still PASS.
- **Testing**: every scenario and criterion has a named test (quickstart.md).
  The hostile corpus is generated, so it is reproducible. What only real
  input shows is automated per OS by the window controls check. Three
  manual procedures remain, none a merge gate. Still PASS.
- **UX Consistency**: no new screen. The dialog, import and form changes
  extend existing patterns, and the text maximums are carried to every
  form. Right-click and keys now behave as in a native application. Still
  PASS.
- **Performance**: the session check is constant time; the import's
  reading is off the lock, in another process, with progress; cancel and
  lock are bounded at 1 s. The PR reports the measured times and memory.
  Still PASS.
- **User Privacy**: frontend data is now bounded by its session, nothing
  new is stored beyond a version number, the helper writes nothing, and the
  menus no longer offer to send text elsewhere. Still PASS.

**Result**: PASS. The design adds no violations beyond those justified
above.

## Findings confirmed with the user

Confirmed by the owner on 2026-10-11 at planning (spec.md clarifications):
- **Free-text maximums**: 200 characters for short fields, 4,000 for notes
  and a firearm's accessories, checked on save and on import (research.md
  §10).
- **The import is read in a confined helper process** (research.md §7,
  §8).

Design choices made at planning that the spec left open (worth a look):
- **The session id is the existing per-open counter**, carried in an IPC
  header and checked under the session's lock. A late request is refused
  with the existing `DATABASE_CLOSED` (research.md §1, §2).
- **The unscoped commands** are the chooser, `get_database_status`, quit,
  idle reporting, `skip_backup`, `cancel_import`, the per-computer
  document-opening setting, and `restore_backup` from the chooser.
  `lock_database`, `close_database` and the pending changes commands name
  their session (research.md §2).
- **Services take the scope as their first parameter**, rather than reading
  a current session (research.md §3).
- **The pending dialog stays up, "Opening…", until the resumed form opens**,
  so the collection is blocked as FR-003 asks, and nothing else can be
  opened in front of the form (research.md §6).
- **Stop reading cancels only the reading.** Once rows are being saved the
  button is disabled as today, and only a sleep stops that phase, keeping
  the rows already imported. Allowing a stop there too would change row
  behaviour, which FR-009 keeps (research.md §11).
- **The idle lock can't fall due while a file is read**, because a running
  operation pauses the idle clock (003, unchanged). US2-5's idle case
  therefore can't arise; a user lock, screen lock, sleep or quit goes ahead
  (research.md §11).
- **The limits' values** (research.md §9): 256 MiB per file, 1 GiB
  unpacked, 16 sheets, 100,000 rows, 256 columns, 32,767 characters per cell
  (Excel's own), 256 MiB of text, and 2,000,000 shared strings.
- **The version is recorded at creation**, inside `write_pending`'s
  transaction, and after any scoped write that changed rows. Collection
  writes record it just after their commit, not atomically (research.md
  §16).
- **A test asserts that `package.json`, `tauri.conf.json` and `Cargo.toml`
  give the same version** (research.md §16).
- **`human-testing.sh` gains `--release`** (the E2E-profile build in the
  sandbox, so a person can try the browser controls) **and
  `--pending-from <version>`** (quickstart.md M3).
