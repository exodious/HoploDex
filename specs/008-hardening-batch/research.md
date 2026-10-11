# Research: Session Isolation, Safe Import, Browser Controls and Pending Changes from Another Version

**Feature**: [spec.md](./spec.md) · **Plan**: [plan.md](./plan.md) · **Date**: 2026-10-11

Each section records a decision, why it was made, and what else was
considered. Section numbers are cited from the code (`research.md §n`) and
from the other artifacts.

Two decisions were the owner's, asked at planning on 2026-10-11:
- **Field maximums** (§10): free-text fields get maximum lengths, so that
  "every text field at its longest" (SC-003) has a size and the import limits
  can be set against it.
- **The import is read in a confined helper process** (§7, §8), the same
  executable started again with a hidden argument, as 007's TIFF helper is.

---

## §1 What a session is, in the backend

**Decision**: The database session of FR-001 is the existing
`OpenDatabase::generation` (`src-tauri/src/session/mod.rs`), renamed
`session_id` and given its own type, `SessionId(u64)`. It comes from a
counter that only goes up within a run, so a session's id is never handed out
twice. `SessionInner` also keeps the current id in an `AtomicU64` (0 when
nothing is open), set by `install` and cleared by `take`/`forget_open`, so
that work running outside the session's mutex can ask "is my session still
the open one?" without waiting on a close (§11).

**Rationale**:
- 007 already uses the generation as a session id: `with_generation` runs
  `open_document`'s write only in the session that asked (007 research.md
  §18).
- A new `OpenDatabase` is built for every open, unlock and restore, so every
  event FR-001 lists as ending a session gives a new id. A take-over drops
  the `OpenDatabase`. A switch is a close followed by an open.
- A passphrase change re-keys the same `OpenDatabase` and keeps its id,
  which FR-001 allows, since it doesn't list a passphrase change.

**Alternatives considered**:
- *A random 128-bit token per session*: it gives nothing a counter doesn't.
  The id isn't a secret. `get_database_status` returns it to the only web
  view that can call commands (007's app ACL manifest). What the id has to do
  is never repeat within a run, and a counter guarantees that. A reload of
  the web view finds the session through `get_database_status`, and a restart
  of the process ends every session anyway.
- *The database's own id (`app_state.database_id`)*: it names the file, not
  the session, so it can't tell two sessions of one database apart. The spec
  requires that (Edge Cases: "The same database closed and reopened").

## §2 Every request names its session, and the backend checks it (FR-004)

**Decision**:
1. **The id travels in an IPC header.** The frontend sends every
   collection command with a `HoploDex-Session: <id>` header (§3). Tauri 2
   carries `InvokeOptions.headers` on both IPC paths: the `ipc://` custom
   protocol, and the `postMessage` fallback, which forwards `options.headers`
   into `InvokeMessage::headers()`. The custom protocol's preflight already
   allows the custom headers Tauri itself sends (`Tauri-Callback`,
   `Tauri-Invoke-Key`).
2. **A command argument type reads it.** `session::scoped::ScopedSession`
   implements `tauri::ipc::CommandArg`. It takes the `Session` state (as
   `State<Session>` does) and parses the header. It replaces
   `session: State<'_, Session>` in every command that reaches the open
   database, under the same parameter name, so command bodies keep their
   `session.read(...)` and `session.write(...)` calls. A missing or malformed
   header is refused with `DATABASE_CLOSED`: a request that names no session
   can't be served in one.
3. **`Session`'s accessors take the id.** `read`, `read_stamped`, `write`,
   `write_open`, `write_housekeeping`, `inspect`, `inspect_mut`, `hold` and
   `with_generation` (which becomes `read`/`inspect_mut` with the id, so it
   is folded in) each take a `SessionId`. Each compares it with the open
   database's id **under the session's mutex**, before the fingerprint check
   and before anything runs. A mismatch, or nothing open, is
   `DATABASE_CLOSED`, with nothing changed and nothing returned. The check
   happens under the same lock as the work, so a close or switch can't come
   between them. `ScopedSession` delegates to these with its id.

   The close paths take the id as well. `close_normal` and `lock` (in
   `session/lifecycle.rs`) end a session through `Session::take`, which
   checks no id today, and `lock` with no draft reaches no other accessor.
   So `take` becomes `take(id)`, comparing under the mutex and returning
   nothing (→ `DATABASE_CLOSED`) for another session, and `close_normal` and
   `lock` take the `SessionId` they pass to it. The `ops` in `databases.rs`
   and `backups.rs` that reach the open database take `&Session` and the
   request's `SessionId`; `ScopedSession` doesn't `Deref` to `Session`, so a
   command can't reach the open database without its id. The immediate
   close (`hold_for_close`, at sleep or shutdown) acts on whatever is open,
   as point 4 says.
4. **The backend's own callers** (the idle lock, screen lock, sleep,
   shutdown, quit, take-over, `lock_with_staged`) act on whatever is open. They
   read `session.current_id()` and pass that, through the same accessors.
   There is no unscoped variant of the accessors (no `_with_x` siblings).
5. **Unscoped commands**: a short list in `commands_list.rs`,
   `UNSCOPED_COMMANDS`. These are the commands that have no open session to
   name, or that act on something other than the open collection:
   - `get_chooser_state`, `create_database`, `open_database`,
     `remove_recent_database`, `locate_database`: the chooser.
   - `get_database_status`: how the frontend learns the id.
   - `quit_application`, `note_activity`, `set_idle_paused`.
   - `skip_backup` and `cancel_import` (§11): they act on the running
     operation, whose session may already have ended.
   - `get_document_opening`, `set_document_opening`: per-computer.
   - `restore_backup`: its `database_path` form restores a database that
     isn't open, from the chooser. When the open database is restored, the
     command checks the header itself and refuses a missing or ended one.

   Everything else names its session, including `lock_database`,
   `close_database`, `stage_pending_changes`, `resolve_pending_changes`,
   `dismiss_note`, the settings, saved-passphrase and backup commands, and the
   preview commands. Without the check:
   - a late `lock_database` or `stage_pending_changes` could write one
     session's draft as another database's pending changes;
   - a late `close_database` could close the database opened after it.
6. **A test keeps the lists honest**: `tests/session_scope_test.rs` (beside
   `acl_manifest_test.rs`) reads `src/commands/*.rs` and fails when a
   `#[tauri::command]` takes `State<'_, Session>` but isn't in
   `UNSCOPED_COMMANDS`, or is in the list but takes `ScopedSession`.

**Rationale**:
- One extractor and one check under the mutex can't be forgotten by a new
  command (the test) and can't race a close (the lock).
- A header leaves every command's arguments and every frontend wrapper's
  argument object as they are. The contracts keep their shapes, and so do
  the E2E specs that call commands through WebDriver.
- `DATABASE_CLOSED` is already the code for "the session you are in has
  ended" (`with_generation`, research.md §18 of 007), and every caller
  already handles it.

**Alternatives considered**:
- *A `session` argument on every command*: the same check, but every
  command signature, wrapper and contract changes.
- *Wrapping the generated invoke handler to check the header before
  dispatch*: the check would run before the command takes the session's
  lock, so a close between the two would let the command run in the next
  session. That's the race FR-004 exists to close.
- *A thread-local or task-local id set by the handler*: async commands run
  on other threads, and an implicit id is easy to lose.

**Verified on Linux (T006, 2026-10-11)**: the dev container, WebKitGTK, with
`ScopedSession` on `list_firearms` only. The probe spec
`e2e/specs/zz-session-header-probe.e2e.ts` calls `list_firearms` from the
page through `__TAURI_INTERNALS__.invoke` (its header comment says how to
force the fallback by hand) and passed all 8 checks on both IPC paths:
- **`ipc://` custom protocol**: with `HoploDex-Session: <current id>` the
  call is served; with no header, and with another session's id, it fails
  with `DATABASE_CLOSED`.
- **`postMessage` fallback**: forced by replacing `window.fetch` with one that
  rejects every request to the IPC protocol. Tauri's `ipc-protocol.js` then
  logs "IPC custom protocol failed, Tauri will now use the postMessage
  interface instead", sets `customProtocolIpcFailed` and re-sends through
  `window.ipc.postMessage` for the rest of the page's life. The probe counted
  exactly one rejected `fetch` (the one that tripped the fallback) and saw the
  warning, so every later call went through `postMessage`. The same three
  cases give the same three results: served with the header, `DATABASE_CLOSED`
  without it and with another id. The header survives
  `options.headers` -> `InvokeMessage::headers()`.

Not yet run on macOS (T007) or Windows (T008), where the custom protocol is
`ipc://localhost` and `http://ipc.localhost`; the probe's `fetch` filter
covers both. The probe stays in `e2e/specs/` until those are recorded.

## §3 The frontend's session scope

**Decision**:
- **`SessionScope`** (`src/features/session/sessionScope.ts`) is created by
  `SessionProvider` for each open, from `DatabaseStatus.sessionId`. It holds:
  - `id`, and `ended`, with `end()`, which is synchronous and runs the
    scope's clean-ups (§4);
  - `invoke(command, args)`, which sends the `HoploDex-Session` header. It
    rejects with `CommandFailure { code: "DATABASE_CLOSED" }`, without
    sending, once the scope has ended. When a response arrives after the
    scope ended, it rejects the same way and the response is dropped
    (FR-002);
  - what the session keeps for reuse: the thumbnail cache, the resumed
    draft (§4, §6).
- **The scope is provided** through `SessionScopeContext` to everything
  inside the collection's tree, which is keyed by the open, and is read with
  `useSessionScope()`.
- **Collection services take the scope** as their first parameter:
  `mediaService.listPhotos(scope, owner)`, `firearmsService.getFirearm(scope,
  id)`, and so on for every wrapper in the `*Service.ts` files of `firearms`,
  `accessories`, `mounts`, `insurance`, `media`, `browse`, `import-export`
  and the entry commands. The open session's own commands in
  `sessionService.ts` and `databasesService.ts` take it too, except those on
  the unscoped list (§2). A component captures the scope when it renders, so
  a closure created in session A carries A's scope even if it runs after B
  has opened. That is what a module-level "current session" can't do.
- **`tauriClient.invoke`** takes an optional `{ session?: number }` and
  turns it into the header. Only `SessionScope.invoke` passes it, so a call
  can't name a session without holding its scope.
- **When a scope ends** (synchronously, before anything else of the next
  session can run):
  - on `session:closing` and `session:closed`, whose payloads gain
    `sessionId`, so a late event of an old session can't end the new one;
  - right after "Lock now" sends `lock_database` (the request already carries
    the id);
  - when `opened()` installs a new status (a restore, a switch), which ends
    the previous scope first;
  - on `toChooser`.

**Rationale**:
- A late request has to name the session it was begun in, not the one
  that's open when it's sent. Only an object captured when the work began
  can give it that, and React's render closures give that capture for free.
- Passing the scope explicitly also makes "this call belongs to a session"
  visible in the types: an unscoped collection call doesn't compile.

**Alternatives considered**:
- *A module-level current scope read at call time*: a loop begun in A that
  sends its next request after B has opened would read B's id. That is the
  attachment case of #69.
- *Services as methods of a per-session client* (`useCollectionApi().media`):
  equivalent. It was rejected because it changes every import and mock shape
  more than an added first parameter does.

## §4 What the scope owns: caches, staged drafts, notifications

**Decision**:
- **Thumbnails**: `FirearmThumbnail`'s module-level `photoThumbnailCache`
  moves into the scope (`scope.thumbnails`, keyed by photo id within the
  session). `end()` clears it. Because the scope's `invoke` rejects late
  responses, an image that arrives after the end is never cached or shown
  (US1-1, US1-2).
- **Staging** (`usePendingDraft.ts`): each registration keeps the scope of
  the form that registered, and `flush` stages through that scope. A
  debounced flush that fires after its session ended is refused on the
  frontend, and in the backend if it got that far. `staged` (what the
  backend holds) resets when a scope ends, so a new session's first draft is
  never thought to be staged already.
- **The resumed draft**: the module-level `resumed` moves into the scope
  (§6).
- **Notifications** (FR-005): `ToastProvider` moves from `App.tsx` into
  `SessionProvider`. It wraps the collection's `FaultBoundary`, so it is
  keyed by the open and unmounts with it:
  - every toast of the session goes at once when the collection is replaced
    (by the closing screen, the chooser or the next open);
  - a `notify` from the ended session's closures calls a provider that no
    longer exists, which React ignores, so nothing late is ever shown.

  The session layer and the chooser use no toasts. Their notices (lock,
  close, backup, take-over) are chooser notices kept in `machine.json` (003),
  so they are unaffected. A Vitest test asserts that `DatabaseChooser`,
  `ClosingScreen`, `PendingChangesDialog` and `SessionProvider` render no
  `useToast` consumer outside the keyed tree.
- **Module state left alone**: `documentOpening.ts` (a per-computer
  setting), `lib/busy.ts` (a counter), `placeFocus.ts`, `StrengthHint.tsx`'s
  lazily loaded estimator. None holds collection data. `research.md` lists
  them so the review can check the list is complete. A grep of `src/` for
  module-level `let` and `new Map/Set` found exactly these (2026-10-11).

**Rationale**: FR-001 and FR-002 ask that everything kept outside what is on
screen dies with its session. Putting each such thing in the scope, or in the
tree keyed by the open, makes the session's end the one moment they all go.

**Alternatives considered**:
- *Keying the thumbnail cache by database id and photo id*: it keeps another
  session's images in memory, which 003 FR-033 forbids for the collection's
  data, and it doesn't separate two sessions of one database.
- *Clearing toasts on `session:closed`*: the user-visible effect is the same,
  but a late `notify` would still show. The tree keyed by the open covers
  both.

## §5 Work that makes several changes, and late results (FR-002, FR-004)

**Decision**:
- **Batches**: `PhotoGallery.addAll` and `DocumentList`'s batch check
  `scope.ended` before each file and stop when it has ended. Each `add` goes
  through the scope, so a request already on its way is refused by the
  backend if the session ended meanwhile. Files added before the end stay in
  A (they were written in A's session). Nothing is added to B even when a
  record of B has the same id (US1-5).
- **Loads**: every `load()` and `useEffect` fetch goes through the scope, so
  a response that arrives after the end is rejected before it reaches state.
  It is also thrown away by React, because the component has unmounted.
- **Drops**: `listenForFileDrops` handlers live in components inside the
  tree, so they unsubscribe with it. A drop event delivered after the end
  reaches no handler.

**Rationale**: refusing on both sides means neither a stale frontend
closure nor a request already sent can write into the wrong database. The
backend refusal is the one that holds even if a frontend check is missed.

## §6 Resumed pending changes until their form opens (FR-003)

**Decision**: the backend keeps pending changes until the form has opened
with them, and the dialog stays up until then.

- **`OpenDatabase.pending_unresolved: bool` becomes `pending: PendingState`**
  with three states:
  - `None`;
  - `Unresolved`: a row exists and blocks collection commands
    (`PENDING_CHANGES_UNRESOLVED`, as today);
  - `Resuming`: a row exists, the user chose Resume, and collection commands
    are served, so the record can load and its form open.
- **`resolve_pending_changes` takes four actions** (contracts/tauri-commands.md):

  | Action | Allowed in | What it does |
  |---|---|---|
  | `resume` | `Unresolved` | Returns the draft and moves to `Resuming`. The row stays. |
  | `opened` | `Resuming` | The form has opened with the draft: deletes the row and makes the draft the session's staged draft. → `None` |
  | `notOpened` | `Resuming` | The form couldn't open: back to `Unresolved`. Returns the summary for the dialog. |
  | `discard` | `Unresolved` | Deletes the row. → `None`. Unchanged. |

  An action in the wrong state is a `VALIDATION_ERROR`, with nothing changed.
- **`opened` stages the draft.** The form's own staging is debounced, so
  between the form opening and its first staging the backend would hold
  neither the row nor a staged draft, and a lock, sleep or shutdown then
  would lose the changes. `opened` therefore sets
  `OpenDatabase.staged_draft` to the draft under the same hold of the
  session's mutex that deletes the row, and the frontend's `resumeOpened()`
  marks the draft as staged. The form's later staging replaces it as
  usual.
- **A session that ends while `Resuming`** leaves the row in place: the
  lock's `write_pending` writes only a staged draft, and nothing is staged
  before the form opens. The next open of that database finds the row and
  offers it (FR-003's last sentence).
- **The frontend's resume**: the scope holds `resumed: { draft, state:
  "opening" }`.
  - The `PendingChangesDialog` stays open while the collection mounts
    underneath, showing **Resume editing** as pending ("Opening…"), with
    **Close the database** still available (FR-017). Nothing else in the
    collection can be used meanwhile, so no other form can be opened and
    staged in front of it.
  - When the form's `useResumedDraftTaken` effect runs, it calls
    `scope.resumeOpened()` → `resolve_pending_changes("opened")`, and the
    dialog closes.
  - The places that can tell the form won't open call
    `scope.resumeFailed(reason)` → `resolve_pending_changes("notOpened")`:
    - `FirearmRecordPage` and `AccessoryRecordPage`, when their record fails
      to load;
    - `PolicyEditors`, when the policy isn't in the collection;
    - the collection's `FaultBoundary`, when a render fails.

    The dialog then shows the problem ("The form for <label> couldn't be
    opened. Your changes are still kept.") above its three buttons, and the
    collection is blocked again (US1-4).
  - A resumed draft is matched to a form only through the scope that
    resumed it. Another session's form can't take it (US1-3).

**Rationale**:
- The spec asks that resumed changes are never lost before their form
  opens, and that a failure brings the dialog straight back.
- Keeping the row, rather than deleting it and writing it back on failure,
  means a crash, a lock or a sleep in between loses nothing.
- Keeping the dialog up while the form opens blocks the collection as
  FR-003 asks. It also closes the gap #69 describes, where a draft whose
  form never opened lingered unseen.

**Alternatives considered**:
- *Delete the row on resume and rewrite it on failure*: a session that ends
  between the two loses the changes.
- *A timeout for the form to open*: there is no right value. The failure
  points are known, and each reports.

## §7 Protecting the import's reader: the threat, and the owner's choice

**What can go wrong today** (#66, verified against the locked sources on
2026-10-11):
- **calamine 0.36.0 trusts declared counts.** `read_shared_strings` (in
  `src/xlsx/mod.rs`) calls `self.strings.reserve(n)` with the `uniqueCount`
  attribute, parsed as `usize`. A huge value is a capacity-overflow panic or
  an allocation failure, and the release profile aborts on panic.
- **`worksheet_range` builds a dense grid.** `Range::from_sparse` allocates
  `rows × columns` cells between the first and last cell present. Two cells
  at A1 and XFD1048576 ask for about 17 billion cells.
- **Every sheet's text is held at once**, and **CSV collects every record**
  before any check. Neither has a byte, row or cell budget.
- **The reading happens inside `session.write_open`**, so it holds the
  session's mutex: a lock, sleep or close waits for it.
- **There is no way to cancel** before the row loop begins.

**Decision (owner, 2026-10-11)**: read both formats in a **confined helper
process** (§8), and enforce the limits inside it too (§9). Anything the
helper does wrong, a panic or an allocation that fails, ends only the
helper, and the import is refused as unreadable with the file named. This
gives FR-007 its "however crafted" for the parts of the reading done by
third-party code, not just for the paths found so far.

**Alternatives offered**:
- *In-process, hardened*: pre-check the zip (entry count, unpacked sizes
  measured by inflating, shared-string counts), then use calamine's
  streaming cell reader under the limits. Less code, but a calamine panic
  not yet found would still end the app.
- *Vendor a patched calamine*: the same residual risk, plus a fork to keep
  current.

## §8 The import helper

**Decision**:
- **One helper core, two modes.** The parts of 007's TIFF helper that don't
  depend on TIFF move from `services/preview/` to a new `services/helper/`:
  - `confine.rs`: Landlock and rlimits on Linux, `sandbox_init` and the
    kqueue parent watch on macOS, the parent-owned job object on Windows;
  - `job.rs` (was `helper_job.rs`): spawning into the job object;
  - `frames.rs` (was the framing half of `helper_protocol.rs`);
  - `handle.rs` (was the generic half of `helper_handle.rs`): start, time
    limits, kill on drop.

  `services/preview/` keeps the TIFF requests and responses (`tiff.rs`,
  `helper.rs` as the TIFF mode's loop). The new `services/import_reader/`
  holds the import mode: `reader.rs` (the helper's side, which parses under
  the limits) and `client.rs` (the parent's side).

  The executable is started as `hoplodex --render-helper` (TIFF, unchanged)
  or `hoplodex --import-helper`. `main.rs` checks both before anything else.
  These moves are mechanical renames, so 007's documents get their paths
  updated in place (CLAUDE.md "Spec Kit workflow").
- **Confinement**: as the TIFF helper's, with the memory cap a parameter of
  `confine()`: 2 GiB for TIFF (unchanged) and **512 MiB for import**
  (`RLIMIT_DATA` on Linux, the job's `JobMemoryLimit` on Windows). macOS has
  no enforced data limit, as for TIFF; the limits of §9 bound what the
  helper allocates there.
  - It reads no file: the parent sends the bytes (Landlock and the sandbox
    profile deny file access).
  - It has no network, no core dumps, and dies with the parent (the
    children-die-with-the-app rule: `PR_SET_PDEATHSIG`, the kqueue watch,
    `KILL_ON_JOB_CLOSE`).
- **Protocol** (data-model.md "Import helper frames"):
  1. The parent sends `Begin { format, file_size }`, then the file in
     `Chunk` frames of 1 MiB as it reads it, then `End`. It never holds the
     whole file, so the parent's memory doesn't grow with the file.
  2. The helper replies with:
     - `Progress { done, total }`;
     - `Sheet { name }`, then `Rows { cells }` in batches of 500 rows;
     - one of `Finished`, `Refused { limit, sheet }` and `Unreadable`.

  CSV is parsed as its chunks arrive (`csv::Reader` over the incoming
  bytes). XLSX needs the archive whole, so the helper keeps the bytes it has
  been sent (at most the 256 MB file limit) and opens them with
  `calamine::Xlsx::new(Cursor)`.
- **XLSX reading inside the helper**, in this order:
  1. Walk the zip's central directory: refuse more than 1,000 entries.
  2. Inflate each part the reading needs (workbook, relationships, shared
     strings, styles, the sheets) through a counting reader that stops at
     the unpacked limit. This checks the true sizes, not the declared ones.
  3. Pre-read the shared strings' `count` and `uniqueCount` with quick-xml,
     and refuse a count over the limit before calamine's `reserve` sees it.
     Should another declared size slip through, it is the helper that dies.
  4. Read each sheet with `worksheet_cells_reader` (streaming, no dense
     grid), applying the row, column, cell and total-text limits as cells
     arrive.
- **The parent's side** (`client.rs`):
  - start the helper, stream the files to it, collect the rows, and map
    each sheet's header with the existing `read_table`/`recognise` (which
    stay in `services/spreadsheet.rs`);
  - check `is_cancelled()` and `ScopedSession::is_current()` every 100 ms
    while it waits, and kill the helper at once on either (§11);
  - treat the helper's death, its time limit (120 s with no frame) or a
    malformed frame as `Unreadable`.

**Rationale**:
- It is the same mechanism 007 built for the same reason (an untrusted
  file and a panicking parser in a process that aborts on panic), already
  confined and tested on all three OS. The import reuses it rather than
  adding a second one.
- Streaming both ways keeps each process's memory bounded by the limits,
  not by the file.

**Alternatives considered**:
- *A third executable*: the owner rejects separately built programs, and
  the second-mode pattern already exists.
- *Sending the whole file in one frame*, as `Load` does for TIFF: the
  parent would hold up to 256 MB it has no other use for, and progress
  couldn't be shown while sending.

## §9 Import limits (FR-006, SC-002, SC-003)

**Decision**: fixed constants in `services/import_reader/limits.rs`,
checked inside the helper. File size and row count are also checked by the
parent, as a second line.

| Limit | Value | Checked | Largest the app's own export writes (§10) |
|---|---|---|---|
| File size | 256 MiB (268,435,456 bytes) | file metadata before reading; bytes counted while streaming (the file may grow) | ≈ 170 MB as CSV; ≈ 100 MB as a workbook |
| Size once unpacked (workbook) | 1 GiB, all parts together, measured by inflating | while inflating | ≈ 210 MB |
| Parts in a workbook | 1,000 zip entries | central directory | ≈ 15 |
| Sheets | 16 | workbook part | 2 |
| Rows in a table | 100,000 | while reading | 10,000 |
| Columns in a row | 256 | while reading | 42 |
| Cells in all, empty ones included | 5,000,000 per import | while reading | ≈ 630,000 |
| Text in a cell | 32,767 characters (Excel's own cell limit) | while reading | 4,000 (a note; §10) |
| Text in all, once read | 256 MiB (UTF-8 bytes) | while reading | ≈ 166 MB of ASCII text |
| Shared strings declared or present | 2,000,000 | pre-read, then while reading | ≈ 630,000 |

**How the export's largest was worked out**: 10,000 firearms and 10,000
accessories, every text field at its §10 maximum, with ASCII text.
- **A firearm row** is about 11.2 KB:
  - the six entry fields at 100 characters;
  - ten short fields at 200;
  - notes and accessories at 4,000 each;
  - the policy name at 200;
  - two record ids;
  - about 250 characters of dates, amounts and choices.
- **An accessory row** is about 5.4 KB.
- **Both tables**: about 166 MB of text. CSV adds roughly 1% of delimiters.
- **A workbook**: the shared strings plus the sheets' XML (about 30 bytes a
  cell) unpack to about 210 MB. Random letters compress to roughly half.

The 256 MB total-text limit is a 1.5× margin over the ASCII largest. A
collection whose every field is at its maximum and written entirely in 3- or
4-byte characters would pass it. That collection isn't realistic, and it is
recorded as an accepted edge.

**The refusal**:
- `IMPORT_LIMIT_EXCEEDED`, with `details { file, sheet?, limit }` and a
  message naming the file, the sheet of a workbook, and the limit, for
  example: `inventory.xlsx, sheet "Sheet3": more than 100,000 rows. HoploDex
  imports at most 100,000 rows per table.` (contracts/tauri-commands.md).
- Unreadable files keep today's `VALIDATION_ERROR` "could not read the
  file", now also for a helper that died, timed out or sent a malformed
  frame.
- Nothing is imported in either case (FR-009). The two-table edge is covered
  because both files are read, and pass, before any row is saved, as today.

**Memory** (SC-002, measured as the HoploDex process's resident memory; the
helper is a separate process capped at 512 MiB):
- The parent holds at most the rows the helper streams back. The text is
  bounded by the 256 MiB total-text limit, but each cell also costs about
  32 bytes (a `String` and its allocation) whether it holds text or not.
  Without a bound on cells, a 25 MB CSV of 100,000 lines of 255 commas
  (25.6 million empty cells, within every other limit) would cost the
  parent over 600 MB, and two such files over 1.2 GB. `MAX_CELLS` (5,000,000
  per import, every cell position the helper sends counted, a sparse row
  padded up to its last cell) bounds that at about 160 MB, so the parent
  stays near 420 MB at worst.
- The parent turns each `Rows` batch into import rows as it arrives (the
  sheet's header first), so the cells aren't held twice.
- The helper holds the workbook's bytes, its shared strings and the batch
  being sent.

**Photo names**: `photo_filenames` is written on export and ignored on
import. A record with more than about 270 photos gives a cell over 32,767
characters, which the workbook export already can't write (an existing
limitation, not this feature's). A follow-up issue records it (plan.md
"Close-out").

## §10 Field maximums (owner's decision, 2026-10-11)

**Decision**: every free-text field gets a maximum length, counted in
characters (Unicode scalar values, as `check_entry_text` counts), after
trimming:

| Fields | Maximum |
|---|---|
| Entry fields: make, model, cartridge, caliber, registration form, registered to | 100 (unchanged, `MAX_ENTRY_CHARS`) |
| Short text: a firearm's nickname, serial number, finish, acquisition source, disposition recipient, country of manufacture, importer name, original make, original model and original serial number; an accessory's serial number, acquisition source and disposition recipient; a policy's name, policy number, insurance company, company contact, agent name and agent contact | **200** |
| Long text: a firearm's notes and accessories; an accessory's notes; a policy's notes | **4,000** |

- **One rule, one place**: `models/rules.rs` gains `MAX_SHORT_TEXT_CHARS`,
  `MAX_LONG_TEXT_CHARS` and `check_text_length(label, value, max)`.
  - The message is `"<Label> can be at most <n> characters."`, as the entry
    fields' is.
  - `validate_firearm_input`, `validate_accessory_input`, the dispose and
    policy validations call it.
  - The import's rows go through the same validation, so an over-long cell
    within the 32,767-character limit is a row error, not a refusal of the
    file.
- **Frontend**: `src/lib/textLimits.ts` mirrors the two numbers and the
  check, for an immediate field error as `entryText.ts` gives. No
  `maxLength` attribute: it cuts pasted text off without saying so.
  - The long text areas show "<n> of 4,000" once the text passes 3,600
    characters (`TextArea` gains an optional `limit`).
  - The pattern is carried to every form that has these fields
    (constitution III): FirearmForm, AccessoryForm, DisposeDialog,
    InsurancePolicyForm.
- **Existing data**: the app is unreleased, and 008's schema change already
  means recreating databases (§16). A record that is longer than its
  maximum can't be saved until it is shortened. That is recorded in spec.md's
  clarifications of 2026-10-11.

**Rationale**:
- SC-003 needs a largest collection, and none exists while fields are
  unbounded.
- 4,000 characters is about a page and a half of notes. It keeps the
  largest export (≈ 170 MB) far enough below the 512 MB of SC-002 that both
  can hold.

## §11 Reading outside the session lock: cancel, lock and progress (FR-008)

**Decision**:
- **`import_collection` in two phases**:
  1. *Reading*: register the `Import` operation, as today, so a sleep stops
     it and the idle clock pauses. Then stream the files through the helper
     **without taking the session's mutex**.
  2. *Rows*: `session.write_open(...)`, as today, with the request's
     `SessionId`. A session that ended during the reading is refused there
     with `DATABASE_CLOSED`, and nothing is imported.

  `import_collection_stoppable` takes the files' tables already read, not
  the paths. Integration tests read them with `import_reader::read_files`
  through the real helper binary, as `tiff_preview_test.rs` does.
- **Cancel**: a new unscoped command, `cancel_import`. It stops the running
  operation if it is an `Import` in its reading phase, and does nothing
  otherwise. The reading loop sees `is_cancelled()` within 100 ms, kills the
  helper, and returns `OPERATION_STOPPED` with `importedCount: 0`. The
  dialog says "The import was cancelled. Nothing was imported." (US2-4,
  SC-004).
  - The row phase keeps today's rule: it isn't cancellable from the dialog,
    and a sleep stops it, keeping the rows already imported.
  - The dialog's **Cancel** button is enabled while the file is read and
    disabled once rows are being imported. It is labelled "Stop reading"
    while reading.
- **A lock while reading** (user, Ctrl/⌘+L, screen lock, quit): the lock
  takes the session's mutex at once, because the reading doesn't hold it.
  The session ends, `current_id` changes, and the reading loop, seeing
  `!is_current()`, kills the helper within 100 ms and returns
  `DATABASE_CLOSED`.
  - A sleep or shutdown stops the operation through the registry, as
    today, and begins an immediate close. The loop returns
    `DATABASE_CLOSED` for it too: `OPERATION_STOPPED` is only for a stop
    made by `cancel_import`, which marks the operation as cancelled by the
    user, in a session that is still current. Whichever the loop notices
    first, the result is the same for each trigger, and nothing is
    imported. The dialog has gone with the session, so nothing is shown.
  - The idle lock can't fall due while a file is read, because a running
    operation pauses the idle clock (003 research.md §15, unchanged). The
    idle case of US2-5 therefore can't arise. If it ever did, it would take
    the same path as a lock.
- **Progress**: `import_collection:progress` gains a `phase`:
  - `"reading"`: `processed` and `total` are bytes of the file or files the
    helper has parsed. For a workbook, the unpacked bytes of the parts it
    reads, against their declared sizes, clamped; declared sizes are used
    only for the bar, never trusted.
  - `"importing"`: rows, as today.

  `ProgressBar` shows "Reading <file>…" with a percentage, then rows.

**Rationale**:
- Holding the mutex only for the rows means a lock waits at most for the
  row phase, which is the existing, budgeted behaviour.
- Killing the helper is immediate whatever it is doing, which gives SC-004's
  1 second.

## §12 Browser controls: the frontend layer (FR-010, FR-012, FR-013)

**Decision**: `src/features/app/browserControls.ts`, installed by
`main.tsx` before React renders, only when `import.meta.env.PROD` (FR-014,
§15).
- **`contextmenu`** (capture phase on `window`): `preventDefault()` unless:
  - the target is inside an editable element: `input` (of a text-like type),
    `textarea`, `[contenteditable]` other than `"false"`, or `select`; or
  - the document has a non-empty selection and the click is inside it.

  A read-only or disabled field with selected text gets the copy menu, and
  without a selection nothing (spec Edge Cases).
- **`keydown`** (capture phase), `preventDefault()` and
  `stopPropagation()` for:
  - F5, Shift+F5, Ctrl+F5, Ctrl/⌘+R and Ctrl/⌘+Shift+R;
  - Alt+←/→ on Windows and Linux only. On macOS, Alt is Option, and
    Option+←/→ moves the caret by word in text (FR-013), so it is left
    alone there; WKWebView binds no navigation to it;
  - on macOS, ⌘[ and ⌘], and ⌘← and ⌘→ when focus isn't in editable text,
    where ⌘←/→ move the caret (FR-013);
  - on macOS, ⌃⌘D (Look Up, FR-011);
  - `BrowserBack`, `BrowserForward`, `BrowserRefresh`, `BrowserStop`,
    `BrowserHome`;
  - Ctrl/⌘+P (print), Ctrl/⌘+S (save), Ctrl/⌘+U (view source), Ctrl/⌘+F, F3
    and Ctrl/⌘+G (find), F7 (caret browsing), F12, and Ctrl/⌘+Shift+I, J
    and C (developer tools).

  The app's own keys ("/", Ctrl/⌘+L, Escape) and the editing keys (Ctrl/⌘+C,
  X, V, Z, Y, Shift+Z, A, and the arrow, Home and End keys with Shift) are
  not in the list, so they work as before. Ctrl/⌘+L is handled by
  `SessionProvider`'s own capture listener, which runs after this one
  (this one is installed before React renders) and isn't stopped by it,
  since Ctrl/⌘+L isn't in the table.
- **The platform** is read once from `navigator.userAgent` at install; the
  key table is a function of it, so the tests check both.
- **Force click** (macOS): `webkitmouseforcewillbegin` is prevented, which
  stops WebKit's default force-click action, Look Up (FR-011).
- **Mouse buttons**: `mousedown`, `mouseup` and `auxclick` with `button` 3
  or 4 (back, forward) are prevented, in the capture phase.
- **One list, unit-tested**: the key table is exported, and
  `browserControls.test.ts` checks each key it blocks, each it must let
  through, and the context-menu decision for each kind of target.

**Rationale**:
- #40's proposal, extended to the shortcuts FR-012 names.
- The page sees these events before WebKitGTK, WKWebView or WebView2 acts
  on them, where they act on them at all (§14). The native layer (§13, §14)
  covers what a page can't cancel, and trims the menus the page lets
  through.

## §13 Browser controls: the native menus (FR-010, FR-011)

The page decides whether a menu appears. Each OS's own filter decides what
is in it, on the main web view only, built in `setup()`. A new
`src-tauri/src/window_controls/` holds `mod.rs`, `linux.rs`, `macos.rs` and
`windows.rs`, applied in release builds (`not(debug_assertions)`) and in
E2E builds. The filters are allowlists: an item is kept only if it is known
to be editing or copying, so an item a future web view adds is removed until
someone looks at it.

- **Linux (WebKitGTK)**: `connect_context_menu`, as the PDF surface does
  (`services/preview/surface/linux.rs`). It keeps the stock actions:
  - `Cut`, `Copy`, `Paste`, `Delete`, `SelectAll`, `UnicodeInsertEmoji`;
  - the spelling actions (`SpellingGuess`, `NoGuessesFound`,
    `IgnoreSpelling`, `LearnSpelling`, `IgnoreGrammar`) and `Custom`
    spelling items.

  It removes everything else, including `GoBack`, `GoForward`, `Stop`,
  `Reload`, `InspectElement`, link, image and media items, and Paste as
  Plain Text and the Input Methods submenu, which aren't on FR-011's list
  (the input method is still chosen from the desktop's own controls). A menu left empty isn't shown
  (return `true`).
- **Windows (WebView2)**: `ICoreWebView2_11::add_ContextMenuRequested` on
  the main web view, as the surface does. It keeps items by `Name`:
  - `cut`, `copy`, `paste`, `selectAll`, `undo`, `redo`, `emoji`;
  - the spelling items. WebView2 offers suggestions with the kind
    `COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_COMMAND` and the name
    `spellCheck`. The exact names are confirmed by the window controls
    check (§18), which logs every item offered.

  It removes `back`, `forward`, `reload`, `saveAs`, `print`, `share`,
  `webCapture`, `inspectElement`, `copyLinkToHighlight`, `openLinkInNewWindow`
  and every other name. An empty menu sets `Handled` with no menu.
- **macOS (WKWebView)**: Tauri's web view is wry's `WryWebView`, a
  `WKWebView` subclass. HoploDex adds `willOpenMenu:withEvent:` to that class
  at startup (objc2, `class_addMethod`, calling `super` first), a public
  `NSView` hook AppKit calls before showing a context menu. The hook removes
  every item except:
  - items whose `identifier` is `WKMenuItemIdentifierCut`, `Copy`, `Paste`,
    `SpellingMenu` (and its submenu's Show Spelling and Grammar and Check
    Spelling While Typing), `SpellingGuess`, `IgnoreSpelling` or
    `LearnSpelling`;
  - AppKit's Emoji & Symbols, identified by its action
    `orderFrontCharacterPalette:`.

  It removes Look Up, Translate, Search With Google, Share, Services,
  Speech, Font, Substitutions, Transformations, Writing Direction, Writing
  Tools, AutoFill, Inspect Element and Reload, then tidies separators.

  The same class also gains `quickLookWithEvent:` (NSResponder's public
  method for the three-finger-tap Look Up), doing nothing. With the page's
  `webkitmouseforcewillbegin` and ⌃⌘D handling (§12), that covers FR-011's
  look-ups without a menu. Each is confirmed in the macOS 26 VM in the same
  first task; one that can't be stopped goes to the owner with the menu
  items.
  **This hook is unproven** on macOS 26. It is the first task of Story 3
  (plan.md "First tasks"). If AppKit adds an item after the hook runs (Writing
  Tools and Services are the likely ones), the plan's fallback is the
  owner's decision before the macOS build ships it (spec Assumptions). The
  options are:
  - keep that item;
  - draw HoploDex's own menu with Radix `ContextMenu`, which loses the
    system's spelling and emoji items;
  - offer no menu on macOS.

**Rationale**:
- A page can only cancel a menu outright. It can't remove Look Up or Reload
  from a text field's menu. So the trimming FR-011 asks for has to be done
  by each OS's filter.
- Linux and Windows reuse 007's pattern. macOS is new, and is therefore
  checked first.

**Alternatives considered**:
- *Turn the native menu off and draw our own* (Radix `ContextMenu` with Cut,
  Copy, Paste and Select All): the same on every OS, but it loses the
  system's spelling suggestions and emoji, which #40 and the spec keep.
  It is held as macOS's fallback.
- *WebView2's `AreDefaultContextMenusEnabled = false`*: it removes the text
  field's menu too.

## §14 Browser controls: reload and navigation per OS (FR-012)

**Decision**, by OS. The frontend layer (§12) runs everywhere. The native
measures cover what a page can't cancel.

- **Windows**:
  - `ICoreWebView2Settings3::put_AreBrowserAcceleratorKeysEnabled(false)` on
    the main web view: F5, Ctrl+R, Ctrl+P, Ctrl+S, Ctrl+U, Ctrl+F, F3, F7,
    F12, Ctrl+Shift+I and the Browser* keys stop acting in WebView2 itself.
    The editing shortcuts are not browser accelerators, so they keep
    working. The surface already does this.
  - `ICoreWebView2Settings6::put_IsSwipeNavigationEnabled(false)`: no
    history swipe on a touchpad.
  - The mouse's XButton1 and XButton2 are cancelled in the page (§12). The
    window controls check confirms that WebView2 doesn't navigate on them
    anyway.
- **Linux**: WebKitGTK binds none of the reload keys itself (an
  application such as Epiphany does), and GTK3 has no swipe navigation.
  Whether it acts on mouse buttons 8 and 9 is confirmed by the check. The
  page's handlers (§12) are the measure, and the check confirms the window
  doesn't reload.
- **macOS**: WKWebView binds no reload key unless a menu item does, and
  HoploDex's menu has none. `allowsBackForwardNavigationGestures` stays
  `NO` (WKWebView's default). The page's handlers cover ⌘R, ⌘[ and ⌘].
- **Every OS**: the main window has one history entry (the app is a single
  page with no routing), so Back has nowhere to go. It is blocked anyway, as
  #40 asks.

**Rationale**: each OS gets what it needs and nothing more. Only Windows
has browser accelerator keys built in.

## §15 Development builds (FR-014)

**Decision**:
- The frontend layer is installed only when `import.meta.env.PROD`.
- The native filters are compiled for `not(debug_assertions)`, which
  covers the release and E2E profiles (`profile.e2e` inherits release).

So `npm run tauri dev` keeps Inspect Element and reload, and every E2E run,
screenshot walk and release build has the controls. A test in
`window_controls` asserts that the E2E build installs the filters.

## §16 The last-saved version (FR-015)

**Decision**:
- **Storage**: `app_state.last_saved_version TEXT`, NULL when none is
  recorded, added to `0001_initial.sql` in place (CLAUDE.md "Schema
  changes"; existing databases must be recreated).
- **The value**: `crate::APP_VERSION` = `env!("CARGO_PKG_VERSION")`.
  `tests/app_version_test.rs` checks that `package.json`, `tauri.conf.json`
  and `Cargo.toml` give the same version, so the number shown is the
  number released.
- **When it is written**, by one helper, `db::record_saved_version(conn)`:
  `UPDATE app_state SET last_saved_version = ?1 WHERE last_saved_version IS
  NOT ?1`.
  1. **At creation**, in `db::create_database`'s initialization. A new
     database was written by this version.
  2. **Inside `pending::write_pending`'s transaction, when a draft is
     written.** Both the lock path and the immediate close at sleep or
     shutdown go through it, so the version that kept pending changes is
     recorded atomically with them, and FR-016 always reads the right one.
  3. **By `Session::write_checked`**, for `write`, `write_open` and
     `write_housekeeping`, after a change that succeeded **and changed
     rows**: `conn.total_changes()` before and after, which counts trigger
     changes too. That covers the collection, the database's settings,
     notes dismissed, and resolving pending changes.

  Opening (the open marker), closing (marker, backup record), backups,
  restore's own bookkeeping and the passphrase change don't go through
  `write_checked`, so they leave it unchanged. So does a write that
  changed nothing.
- **Atomicity**: case 3 runs after the change has committed, because the
  changes open their own transactions. A crash between the two leaves the
  previous version recorded. Only the pending changes' message reads it,
  and case 2, the one that message depends on, is atomic.

**Rationale**:
- The owner's note on #78 asks for the version to be recorded in the
  database, independent of the pending changes.
- Recording it only on a save keeps it naming the version that kept the
  pending changes, because nothing else can be saved while they wait.

**Alternatives considered**:
- *Triggers calling an app-registered SQL function*
  (`hoplodex_version()`): atomic, but the schema would then depend on a
  function only HoploDex registers. Any other SQLCipher client, or an older
  HoploDex, would fail on its first write.
- *Recording it at open*: FR-015 forbids it, since another version opening
  and closing without saving would hide who kept the changes.

## §17 The pending changes dialog (FR-016, FR-017)

**Decision**:
- **`PendingSummary`** gains `savedByVersion: string | null` (the database's
  `last_saved_version`) and `appVersion: string` (this build's), so the
  frontend words the message without guessing which version is newer.
- **The wording** when the form's version differs (`canResume` is
  unchanged, so whether the changes resume is still decided by the form's
  version):
  - a recorded version that differs from this one: "They were kept by
    HoploDex <savedByVersion>. Open the database with that version to
    resume them, or discard them here.";
  - the same number as this build: "They were kept by a different build of
    HoploDex <appVersion>. Open the database with that build to resume
    them, or discard them here.";
  - none recorded: "They were kept by another version of HoploDex. Open the
    database with that version to resume them, or discard them here."
- **The buttons**, in every case:
  - **Discard changes** (danger, asks first, unchanged);
  - **Close the database** (secondary);
  - **Resume editing** (primary, when it can resume).

  Close calls `closeDatabase("closed")`. `close_normal` doesn't check
  pending changes, so the normal close, its backup when due, and the
  chooser follow. A backup never carries `pending_changes` (003), and the
  row is untouched (US4-2, US4-5). The record-gone case gets Close too
  (US4-5).
- **Contract**: 003's `contracts/ui-databases.md` §13 is amended with a
  pointer to this feature's UI contract.

## §18 Testing

Every acceptance scenario and success criterion maps to a named test in
quickstart.md. In outline:

- **Rust (cargo), real SQLCipher databases**:
  - `session_scope_test.rs`:
    - every accessor refuses an ended id and a never-issued id;
    - a late request after a switch to a second database whose ids collide
      returns nothing and changes nothing (SC-001's backend half);
    - the command lists (§2).
  - `pending_resume_test.rs`: the four actions in each state, and the row
    kept across a lock while resuming.
  - `last_saved_version_test.rs`: FR-015's writes and non-writes.
  - `import_reader_test.rs` with `support/hostile_spreadsheets.rs`, the
    SC-002 corpus generated at test time:
    - the impossible `uniqueCount`;
    - the A1/XFD1048576 grid;
    - a 1000× zip bomb;
    - over-limit rows, columns, sheets, cells and total text;
    - a CSV with one 300 MB field;
    - truncated and bit-flipped workbooks.

    Each must be refused within 5 s, the parent's resident memory must stay
    within 512 MB, and the helper must be dead afterwards.
  - Cancel and lock-while-reading within 1 s (SC-004).
  - `text_limits_test.rs` for §10.
  - A release-only, `#[ignore]`d `import_largest_test.rs` for SC-003, which
    generates the §9 largest export as a workbook and as two CSVs and
    imports them.
- **Vitest**:
  - `sessionScope.test.ts`;
  - `SessionProvider` ending scopes on each event;
  - `FirearmThumbnail` with deferred responses and two scopes whose photo
    ids collide;
  - `PhotoGallery` and `DocumentList` batches stopped part-way;
  - toasts gone at the session's end, and a late `notify` showing nothing;
  - the resume lifecycle (opened, failed, session ended);
  - `PendingChangesDialog`'s three wordings and its Close button;
  - `browserControls.test.ts`;
  - `TextArea`'s counter.
- **E2E (WebdriverIO)**:
  - `us14-session-isolation.e2e.ts`: two databases seeded with colliding
    ids, where the thumbnail shown after lock-and-switch is B's, not A's.
  - `us15-browser-controls.e2e.ts`:
    - `contextmenu` events dispatched at each kind of target, asserting
      `defaultPrevented`;
    - real F5, Ctrl+R, Alt+← and mouse back on Linux (XTest) with typed
      input surviving;
    - every OS: the app's own and editing shortcuts still work.
  - `us9-locking.e2e.ts` gains pending changes from another version
    (seeded by `human_seed --pending-from 1.2.0`): open, close from the
    dialog, reopen.
  - `us5-export-import.e2e.ts` gains a refused file and a cancelled read.
- **The window controls check** (new, per OS, like the PDF surface check):
  `examples/window_controls_check.rs` builds the main window with the app's
  own `window_controls` code and a test page. With real input (XTest, and
  the existing `pdf_surface_input.swift` and `.ps1`, renamed `real_input.*`),
  it:
  - right-clicks a blank area, a row, a button, an image, a text field and
    a selection, and logs every item each native menu offered (through the
    filters' own hooks);
  - presses every FR-012 key and the mouse's back and forward buttons;
  - fails if a menu offered an item outside FR-011's list, a menu appeared
    where none should, or the page reloaded or navigated (a marker in
    `sessionStorage` and a typed field).

  It covers SC-005 on macOS and Windows, where real input isn't available
  to the E2E specs (DEVELOPMENT.md "Real keyboard and mouse input").
- **Manual** (best effort before a release, numbered in quickstart.md):
  the menus and look-ups on a real Mac's trackpad (M1), workbooks saved by
  other programs (M2), and pending changes from another version by hand
  (M3). The check covers mice and keyboards.

## §19 Performance

- **The session check**: an integer comparison under a mutex the command
  takes anyway, and one header per IPC call. Nothing measurable against the
  500 ms and 1 s budgets. `performance_test.rs` runs unchanged with ids
  added.
- **Import reading**:
  - spawning the helper adds about 20 ms (TIFF helper, 007 research.md
    §22);
  - streaming the file costs one copy through a pipe;
  - the row phase is unchanged.

  `import_largest_test.rs` records the time for the largest export
  (informational; constitution IV sets no budget for an import's total
  time, only that it runs with progress, which it now does from its first
  byte).
- **Field maximums**: one character count per field per save.
- **Browser controls**: one capture listener each for `contextmenu`,
  `keydown`, and the mouse button events.

## §20 Security and data handling (for the pull request)

- **#69**: the backend refuses every request of an ended session, reads
  included, under the session's lock (§2). The frontend forgets the session's
  images, drafts, toasts and pending work when it ends (§3–§6). Nothing new
  is stored. The session id isn't a secret and authorizes nothing by itself.
- **#66**:
  - untrusted spreadsheets are parsed only in a confined, memory-capped
    helper with no file or network access, which dies with the app;
  - limits are enforced as the data arrives, not from declared sizes;
  - the parent never holds the session's lock while a file is read (§7–§11).
- **#40**:
  - the web view's navigation, print, save, view-source and developer-tools
    paths are closed in release builds;
  - nothing that sends selected text to another program or the internet
    (Look Up, Translate, Search With Google, Share, Services, Writing Tools)
    stays in the menu (§12–§14).
- **#78**: the version is a public build number, and pending changes keep
  their existing protections. Closing from the dialog is a normal close,
  whose backup never carries pending changes (§16, §17).
- **No cipher, key or passphrase handling changes.**

## §21 Dependencies and licensing

No new crate and no new npm package.
- `calamine`, `csv`, `zip` and `quick-xml` stay at their locked versions.
  They now run in the helper process.
- `webkit2gtk`, `objc2-web-kit`/`objc2-app-kit`/`objc2` and `webview2-com`
  are already direct dependencies (007), with new uses only.
- `windows-sys` may need the `ICoreWebView2Settings6` interface from
  `webview2-com`, which is already in its bindings.

Nothing is bundled or fetched.

## §22 Residual risks

- **macOS's menu hook** may not see items AppKit adds after it (Writing
  Tools, Services). It is the first task, with the owner deciding the
  fallback (§13).
- **The macOS helper has no enforced memory cap** (as for TIFF). The import
  limits bound what it allocates, and a runaway allocation there ends only
  the helper.
- **The last-saved version is recorded after the commit** for collection
  writes (§16). A crash in between names the previous saver. The pending
  changes case is atomic.
- **A collection at every field's maximum in 3- or 4-byte characters**
  passes the 256 MB total-text limit (§9).
- **macOS's look-ups without a menu** (force click, three-finger tap,
  ⌃⌘D) depend on WebKit honouring `webkitmouseforcewillbegin` and on the
  responder chain reaching `quickLookWithEvent:`. The first task of Story 3
  confirms them; one that can't be stopped goes to the owner (§13).
- **The parent's memory bound** rests on `MAX_CELLS` and the total-text
  limit together (§9); a change to either needs the arithmetic redone.
- **Notifications raised by the session layer** (none today) would not be
  covered by the keyed `ToastProvider`. The Vitest guard of §4 catches a new
  one.
