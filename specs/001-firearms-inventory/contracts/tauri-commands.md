# Contract: Tauri IPC Commands

HoploDex has no network-facing API; the frontend/backend boundary is
Tauri's IPC command layer (`invoke("command_name", args)` from React,
`#[tauri::command] async fn command_name(...)` in Rust). This document is
the contract between `src` and `src-tauri` — the frontend MUST only reach
the database, filesystem, or keyring through these commands.

_Amended by [spec 003](../../003-database-protection-management/contracts/tauri-commands.md): every collection command below fails with `DATABASE_CLOSED` while no database is open, and 003 adds the commands that create, open, close, lock and back up databases._

All commands are `async` and run on Tauri's async runtime, never on the
UI thread (constitution Principle IV). Long-running commands (`import_*`,
`export_*`) additionally emit progress events over a Tauri event channel
named `"{command}:progress"` with a `{ processed: number, total: number }`
payload, so the frontend can render a shared progress-bar component.

Every monetary number in a command's input or output (`estimatedValue`,
`acquisitionPrice`, `dispositionPrice`, `price`, `scheduledCoverageAmount`,
`scheduledAmount`, `blanketCoverageLimit`, `limit`, `total`, `collectionTotal`)
is a non-negative integer number of whole U.S. dollars (FR-037). The backend
rejects a negative value with `VALIDATION_ERROR` and a `fieldErrors` entry, and
a fractional number never decodes into the integer argument at all (Tauri
refuses the call before the command runs); it never rounds. Thousands separators are a display
concern of the frontend only and never cross this boundary.

All error returns use a common shape:

```ts
type CommandError = {
  code: string;        // e.g. "VALIDATION_ERROR", "NOT_FOUND", "POLICY_HAS_FIREARMS"
  message: string;      // human-readable, safe to show the user
  fieldErrors?: Record<string, string>; // for per-field validation failures
};
```

_Amended by [spec 003](../../003-database-protection-management/contracts/tauri-commands.md): the shape gains an optional `details` object for the codes that carry data, such as `DATABASE_OPEN_ELSEWHERE`'s `machineName` and `since`._

## Firearm records (User Story 1)

### `create_firearm`

- **Input**: `FirearmInput` — all `Firearm` fields from data-model.md
  except `id`, `created_at`, `updated_at`, `thumbnail_photo_id`. This includes
  the FR-039 physical details, which cross the boundary as the stored scaled
  integers `barrelLengthHundredths`, `overallLengthHundredths` (inches × 100),
  `weightTenthsOz` (ounces × 10), `capacity`, `finish` and `condition`
  (`"new_in_box" | "like_new" | "excellent" | "good" | "fair" | "poor"`), all
  nullable; the frontend converts to and from decimals for display.
  `FirearmSummary` (list results) does not carry them.
- **Output**: `Firearm` (full record as persisted).
- **Errors**: `VALIDATION_ERROR` (e.g. missing serial number without
  attestation, or a serial number together with the attestation —
  FR-029; disposition fields inconsistent with status;
  duplicate nickname among active firearms — FR-031; duplicate
  make/model/serial among active firearms — FR-032; the message names
  the conflicting record; acquisition or disposition date in the future, or
  disposition before acquisition — FR-003/FR-004; a physical detail out of
  range — a length or weight `<= 0`, a capacity `< 1`, or an unknown
  `condition` — FR-039, with a `fieldErrors` entry naming the field).

_Amended by [spec 002](../../002-firearm-identification/contracts/tauri-commands.md):
`FirearmInput` gains the seven identification fields; a
`confirmedWarnings?: boolean` argument and the `ORIGINAL_MARKS_MATCH` error
code are added; the make/model/serial clash is accepted, not blocked, when
both records have a year of manufacture and the years differ._

_Amended by [spec 004](../../004-cartridges-action-types/contracts/tauri-commands.md): `FirearmInput` gains `cartridge` and
`actionTypeId`, checked against the entry rules and the type's allowed
actions._

### `update_firearm`

- **Input**: `id: number`, `FirearmInput` (partial or full; validation
  rules from data-model.md apply to the resulting record).
- **Output**: `Firearm` (as `create_firearm`).
- **Errors**: `VALIDATION_ERROR` (same uniqueness rules, excluding the
  record itself), `NOT_FOUND`.

_Amended by [spec 002](../../002-firearm-identification/contracts/tauri-commands.md):
as `create_firearm`._

_Amended by [spec 004](../../004-cartridges-action-types/contracts/tauri-commands.md): as `create_firearm`._

### `dispose_firearm`

- **Input**: `id: number`, `{ dispositionType, recipient, date, price }`.
- **Output**: `Firearm` (status now `disposed`).
- **Errors**: `VALIDATION_ERROR` (including a future `date`, or a `date`
  earlier than the firearm's acquisition date — FR-004), `NOT_FOUND`.

### `reverse_disposition`

- **Input**: `id: number`, `{ history: "keep" | "discard", nickname?: string | null }`.
  `history` is required: the frontend asks the user (via the shared
  `ConfirmDialog` pattern, constitution III) and never defaults it
  silently. The optional `nickname` lets the user resolve a FR-031 nickname
  clash in the same step by renaming.
- **Output**: `Firearm` (status now `active`, disposition fields null).
- **Errors**: `VALIDATION_ERROR` (record is not disposed; `history`
  missing; nickname or make/model/serial clash with a currently active
  firearm per FR-031/FR-032 — message names the conflicting record;
  nothing is changed), `NOT_FOUND`.
- **Side effects**: with `keep`, inserts a `DispositionHistory` row;
  status change, history insert, and column clear happen in one
  transaction. Frontend re-invokes `get_value_summary` afterwards (the
  firearm re-enters the value summary, FR-015/FR-025).

_Amended by [spec 002](../../002-firearm-identification/contracts/tauri-commands.md):
input gains `confirmedWarnings?: boolean`; errors also include the
FR-007/FR-008 year exception and `ORIGINAL_MARKS_MATCH`, both inside the
same transaction as the status change and history insert._

### `delete_firearm`

- **Input**: `id: number`, `confirmed: true` (frontend enforces the
  confirmation dialog before ever sending `confirmed: true`).
- **Output**: `{ deleted: true }`.
- **Errors**: `NOT_FOUND`.
- **Side effects**: cascades to delete associated Photos and
  DocumentAttachments (spec Assumption).

### `get_firearm`

- **Input**: `id: number`.
- **Output**: `FirearmDetail`: the `Firearm` plus `dispositionHistory`, its
  retained `DispositionHistory` rows (newest first, FR-033). Photos and
  documents come from `list_photos` / `list_documents`; the policy and the
  computed insurance-status flags come from `list_insurance_policies` and
  `list_firearms`, so nothing is fetched twice.
- **Errors**: `NOT_FOUND`.

## Browse, search, group (User Story 2)

### `list_firearms`

- **Input**:
  ```ts
  {
    query?: string;                 // free-text, run against firearms_fts
    groupBy?: "type" | "caliber" | "make";
    includeDisposed?: boolean;      // default false, FR-025
    view?: "list" | "tile";         // informational; both return the same data shape
  }
  ```
- **Output**: `{ groups: { key: string; firearms: FirearmSummary[] }[] }`
  where `FirearmSummary` includes id, make, model, nickname (FR-031),
  serial number (so firearms sharing a make and model stay
  distinguishable in lists),
  caliber, type, status, thumbnail reference (`thumbnailPhotoId` or
  generic type key), estimated value, scheduled coverage
  (`insurancePolicyId`, `scheduledCoverageAmount`; both null when the
  firearm is unscheduled), and computed insurance-warning
  flags (for SC-004's "always visibly flagged" requirement).
  _Amended by [spec 005](../../005-regulated-item-types/spec.md): the summary also carries `registeredAs`; see 005's command contract._
- **Search semantics**: `query` matches as a phrase whose last word may be
  partial (`"cracked han"` finds "cracked handle"), so results can update
  as the user types.
- **Performance contract**: MUST return within 500ms at 10,000-record
  scale (Principle IV) — implemented via the FTS5 index and indexed
  columns on `firearm_type_id`/`caliber`/`make`.

_Amended by [spec 002](../../002-firearm-identification/contracts/tauri-commands.md):
`groupBy` gains `"origin"`; `query` also matches origin, year, country,
importer and original marks._

_Amended by [spec 004](../../004-cartridges-action-types/contracts/tauri-commands.md): `groupBy` gains `"cartridge"` and
`"action_type"`; `FirearmSummary` carries `cartridge` and `actionType`; `query`
also matches both._

## Insurance & valuation (User Story 3)

### `list_insurance_policies`

- **Input**: `{}`.
- **Output**: `InsurancePolicy[]`, ordered by name, each with its derived
  status as of today: `isInForce`, `isExpired`, `isExpiringSoon` (facts about
  its dates) and `expiringWarning`, `expiredWarning` (what to warn about,
  after FR-028's suppression for a renewed or replaced blanket policy).
  `create_insurance_policy` and `update_insurance_policy` return the same
  shape. Populates the
  coverage-assignment policy picker (not itself an acceptance-scenario
  requirement, but necessary plumbing `assign_firearm_coverage` depends on
  the frontend already knowing).

### `create_insurance_policy` / `update_insurance_policy`

- **Input**: `InsurancePolicyInput` (all InsurancePolicy fields except id, including the optional `notes`; a blank value is stored as null, FR-027).
- **Output**: `InsurancePolicy`.
- **Errors**: `VALIDATION_ERROR` (e.g. end date before start date; blanket
  policy dates overlapping another blanket policy by more than a shared
  boundary day — FR-036, the message names the other policy).

### `get_policy_deletion_impact`

- **Input**: `id: number`.
- **Output**: `{ isExpired: boolean, isBlanketInForce: boolean, scheduledFirearmCount: number, scheduledFirearms: { id: number; make: string; model: string; nickname: string | null }[], blanketFirearmCount: number, unscheduleOutcome: "blanket" | "uninsured", otherPolicies: { id: number; name: string; isExpired: boolean }[] }`.
- **Errors**: `NOT_FOUND`.
- Read-only. `blanketFirearmCount` is the number of active unscheduled
  firearms that lose blanket coverage because this is the blanket policy
  currently in force (0 otherwise). `unscheduleOutcome` says what happens to
  firearms left unscheduled: they fall under another blanket policy that is
  in force after the deletion (`"blanket"`) or are uninsured
  (`"uninsured"`). Lets the frontend build the FR-034 dialog before anything
  changes.

### `delete_insurance_policy`

- **Input**: `id: number`, `confirmed: true`, and, when the policy has
  scheduled firearms, `scheduledFirearms`:
  `{ action: "move", targetPolicyId: number } | { action: "unschedule", confirmUnschedule?: true }`.
- **Output**: `{ deleted: true, movedCount: number, unscheduledCount: number }`.
- **Behavior** (FR-034): one transaction. `move` reassigns every scheduled
  firearm to the target policy, keeping `scheduledCoverageAmount`.
  `unschedule` clears the policy and scheduled amount. Then the policy is
  deleted. Unscheduled (blanket) firearms need no handling: their coverage is
  computed. Frontend re-invokes `get_value_summary` afterwards.
- **Errors**: `POLICY_HAS_FIREARMS` (scheduled firearms exist and
  `scheduledFirearms` omitted; nothing changed), `VALIDATION_ERROR` (target
  missing, equal to the deleted policy, or nonexistent; `unschedule` on a
  non-expired policy without `confirmUnschedule`), `NOT_FOUND`.

### `assign_firearm_coverage`

- **Input**: `firearmId: number`, `{ policyId: number | null, scheduledCoverageAmount?: number }`.
  A non-null `policyId` schedules the firearm under that policy and requires
  `scheduledCoverageAmount`; `policyId: null` makes it unscheduled, i.e.
  covered by the blanket policy in force (FR-036). There is no per-firearm
  blanket assignment.
- **Output**: `Firearm`.
- **Errors**: `VALIDATION_ERROR`.

### `get_value_summary`

- **Input**: `{}` (always reflects current active, non-disposed firearms per FR-025).
- **Output**:
  ```ts
  {
    collectionTotal: number;               // whole dollars, all active firearms
    blanket: {                             // blanket policy in force today; null if none
      policyId: number;
      policyName: string;
      limit: number;
      total: number;                       // sum of values of active unscheduled firearms
      firearmCount: number;
      underInsured: boolean;
    } | null;
    byPolicy: {                            // policies that have scheduled firearms
      policyId: number;
      policyName: string;
      isExpired: boolean;
      isExpiringSoon: boolean;
      individuallyScheduled: {
        firearmId: number;
        estimatedValue: number;
        scheduledAmount: number;
        underInsured: boolean;
      }[];
    }[];
    uninsured: { firearmId: number; estimatedValue: number }[]; // unscheduled firearms when no blanket policy is in force
  }
  ```
- **Recompute contract**: the frontend re-invokes this after every
  mutating command (`create_firearm`, `update_firearm`, `dispose_firearm`,
  `delete_firearm`, `assign_firearm_coverage`, policy CRUD) so the
  displayed summary is never stale (FR-015, SC-003) — no separate
  "refresh" action exists.

## Photos & documents (User Story 4)

### `list_photos` / `list_documents`

- **Input**: `firearmId: number`.
- **Output**: `PhotoSummary[]` / `DocumentSummary[]` — omit `originalBytes`/
  `fileBytes` (a document's bytes are read back only by `open_document`,
  and never at all for photos, which only ever need their pre-generated
  `thumbnailBytes` in the UI) to keep these list payloads small. Not in the original contract
  list, but necessary plumbing for `PhotoGallery`/`DocumentList` display.

### `add_photo`

- **Input**: `firearmId: number`, `fileBytes: Uint8Array`, `originalFilename: string`, `mimeType: string`.
- **Output**: `Photo` (with generated `thumbnailBytes`). First photo added
  to a firearm automatically becomes `thumbnail_photo_id` (FR-008).
- **Errors**: `VALIDATION_ERROR` (unsupported mime type), `NOT_FOUND`.

### `add_photo_from_path` / `add_document_from_path`

- **Input**: `firearmId: number`, `path: string`.
- **Output**: as `add_photo` / `add_document`. The backend reads the
  file itself and takes the original filename from the path and the mime
  type from its extension (`services::attachments`).
- **Why**: files dragged onto the window from the desktop reach the
  frontend as paths, never as `File` objects (WebKitGTK exposes none, and
  Tauri's own drag-drop handling must stay enabled so the webview doesn't
  navigate to a dropped file). The file pickers keep using
  `add_photo`/`add_document` with bytes.
- **Errors**: `VALIDATION_ERROR` (a folder, or — for photos — anything
  but JPEG/PNG), `NOT_FOUND` (path missing or unreadable).

### `get_photo_thumbnail`

- **Input**: `photoId: number`.
- **Output**: raw thumbnail bytes for one photo. Not in the original
  contract list, but lets browse views render a firearm's thumbnail
  without fetching its full photo list.

### `get_photo_original`

- **Input**: `photoId: number`.
- **Output**: the photo's full-resolution original bytes as a raw binary
  IPC response (an `ArrayBuffer` in the frontend, not a JSON number
  array, so multi-megabyte photos transfer quickly). Used by the record
  view's photo viewer and plate, where condition details must be legible.
- **Errors**: `NOT_FOUND`.

### `set_thumbnail_photo`

- **Input**: `firearmId: number`, `photoId: number`.
- **Output**: `Firearm`.
- **Errors**: `NOT_FOUND`.

### `delete_photo`

- **Input**: `photoId: number`, `confirmed: true`.
- **Output**: `{ deleted: true }`. If the deleted photo was the thumbnail,
  `thumbnail_photo_id` falls back to the next-oldest remaining photo, or
  `null` (generic thumbnail) if none remain.

### `add_document` / `delete_document`

- Analogous to photo commands, without thumbnail generation. The stored
  `file_bytes` never cross the IPC boundary; `open_document` is the only
  way to reopen a document.

### `open_document`

- **Input**: `id: number`.
- **Output**: nothing. Reopens the document from its record (FR-010) in
  the OS default app for its file type, by writing a temporary copy to
  `<app cache dir>/opened-documents/<id>/<original filename>` (reduced to
  a single, OS-safe path component) and handing that path to the OS. The
  webview can't display arbitrary files itself. **Lifecycle (FR-035):** the whole
  `opened-documents` folder is deleted when the app exits normally
  (window close, quit, or an OS shutdown where the platform lets the app
  run cleanup). The app cannot tell when an external viewer has closed the
  file, since many viewers hand off to an already-running process, so
  cleanup on exit is the guarantee. The folder is also swept at every
  startup as a backstop for crashes and forced kills, and anything that
  could not be deleted is retried then. Deletion uses secure deletion
  (overwrite before unlink) where the OS supports it. Decrypted copies
  therefore never outlive the session that created them.
- **Errors**: `NOT_FOUND`, `INTERNAL_ERROR` (copy couldn't be written or
  no app could open it).

### `get_generic_thumbnail`

- **Input**: `key: string` (a `FirearmType.generic_thumbnail_key`).
- **Output**: raw PNG bytes of the bundled generic-thumbnail asset
  (research.md §10). No longer used by the frontend, which draws each
  type's generic thumbnail as inline vector line art (FR-009) instead of
  fetching these placeholder images.

## Export / Import (User Story 5)

### `export_collection`

- **Input**: `{ format: "csv" | "xlsx", destinationFolder: string, scope: "all" | "filtered", filter?: ListFirearmsInput }`.
- **Output (progress events, then)**: `{ spreadsheetPath: string, photosFolderPath: string, exportedFirearmCount: number, exportedPhotoCount: number }`.
- **Behavior**: `scope` resolves the Edge Case "export while a filter is
  active" — the frontend must pass the currently-active `list_firearms`
  filter explicitly rather than the backend guessing; default is `"all"`.
  Produces one spreadsheet (all recorded fields per FR-018) plus a
  sibling folder of original-format photo files.

### `import_collection`

- **Input**: `{ filePath: string, format: "csv" | "xlsx" }`.
- **Output (progress events, then)**: `{ sessionId: string, importedCount: number, updatedCount: number, skippedCount: number, rowErrors: { row: number; message: string }[], conflicts: ImportConflict[] }`.
  Each `ImportConflict`
  carries `duplicateAllowed: boolean`, false where FR-032 would block the
  resulting record, so the frontend offers only skip and overwrite there.
- **Behavior**: Rows failing validation are reported per-row without
  discarding successful rows (FR-020). Rows matching an existing
  `(make, model, serial_number)` key produce an `ImportConflict` requiring
  resolution rather than being silently applied. FR-031/FR-032 apply per
  row: a duplicate nickname is a row error; a `duplicate` resolution is
  only valid where FR-032 would allow the resulting record.

_Amended by [spec 002](../../002-firearm-identification/contracts/tauri-commands.md):
output gains `warnings: { row: number; message: string }[]` for the FR-009
original-marks match (a row still imports); a row's main-marks match is not
a conflict at all when both it and the existing record have a year of
manufacture and the years differ._

_Amended by [spec 004](../../004-cartridges-action-types/contracts/tauri-commands.md): output gains `derivedCalibers` and
`snappedValues` for the report, and a row is matched after its make and model
are snapped._

### `resolve_import_conflicts`

- **Input**: `{ importSessionId: string, resolutions: { conflictId: string; action: "skip" | "overwrite" | "duplicate" }[], applyToRemaining?: "skip" | "overwrite" | "duplicate" }`.
- **Output**: `{ resolvedCount: number, unresolved: { row: number; message: string }[] }`.
- **Behavior**: Implements FR-026's per-row resolution plus "apply to all
  subsequent conflicting rows" option; `applyToRemaining` only affects
  conflicts not explicitly listed in `resolutions`. `duplicate` (explicit
  or via `applyToRemaining`) is rejected for a conflict FR-032 would
  block, and an `overwrite` that fails validation (for example a nickname
  clash, FR-031) is rejected likewise. Such conflicts are returned in
  `unresolved` and stay open in the session, so a different action can
  still be chosen for them; only infrastructure failures abort the call.
