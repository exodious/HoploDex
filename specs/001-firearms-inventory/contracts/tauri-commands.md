# Contract: Tauri IPC Commands

HoploDex has no network-facing API; the frontend/backend boundary is
Tauri's IPC command layer (`invoke("command_name", args)` from React,
`#[tauri::command] async fn command_name(...)` in Rust). This document is
the contract between `src` and `src-tauri` — the frontend MUST only reach
the database, filesystem, or keyring through these commands.

All commands are `async` and run on Tauri's async runtime, never on the
UI thread (constitution Principle IV). Long-running commands (`import_*`,
`export_*`) additionally emit progress events over a Tauri event channel
named `"{command}:progress"` with a `{ processed: number, total: number }`
payload, so the frontend can render a shared progress-bar component.

All error returns use a common shape:

```ts
type CommandError = {
  code: string;        // e.g. "VALIDATION_ERROR", "NOT_FOUND", "POLICY_HAS_FIREARMS"
  message: string;      // human-readable, safe to show the user
  fieldErrors?: Record<string, string>; // for per-field validation failures
};
```

## Firearm records (User Story 1)

### `create_firearm`

- **Input**: `FirearmInput` — all `Firearm` fields from data-model.md
  except `id`, `created_at`, `updated_at`, `thumbnail_photo_id`.
- **Output**: `Firearm` (full record as persisted).
- **Errors**: `VALIDATION_ERROR` (e.g. missing serial number without
  attestation — FR-029; disposition fields inconsistent with status).

### `update_firearm`

- **Input**: `id: number`, `FirearmInput` (partial or full; validation
  rules from data-model.md apply to the resulting record).
- **Output**: `Firearm`.
- **Errors**: `VALIDATION_ERROR`, `NOT_FOUND`.

### `dispose_firearm`

- **Input**: `id: number`, `{ dispositionType, recipient, date, price }`.
- **Output**: `Firearm` (status now `disposed`).
- **Errors**: `VALIDATION_ERROR`, `NOT_FOUND`.

### `delete_firearm`

- **Input**: `id: number`, `confirmed: true` (frontend enforces the
  confirmation dialog before ever sending `confirmed: true`).
- **Output**: `{ deleted: true }`.
- **Errors**: `NOT_FOUND`.
- **Side effects**: cascades to delete associated Photos and
  DocumentAttachments (spec Assumption).

### `get_firearm`

- **Input**: `id: number`.
- **Output**: `FirearmDetail` (Firearm + its Photos + DocumentAttachments +
  resolved InsurancePolicy summary + computed insurance-status flags).
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
  where `FirearmSummary` includes id, make, model, caliber, type,
  status, thumbnail reference (`thumbnailPhotoId` or generic type key),
  estimated value, and computed insurance-warning flags (for SC-004's
  "always visibly flagged" requirement).
- **Performance contract**: MUST return within 500ms at 10,000-record
  scale (Principle IV) — implemented via the FTS5 index and indexed
  columns on `firearm_type_id`/`caliber`/`make`.

## Insurance & valuation (User Story 3)

### `create_insurance_policy` / `update_insurance_policy`

- **Input**: `InsurancePolicyInput` (all InsurancePolicy fields except id).
- **Output**: `InsurancePolicy`.
- **Errors**: `VALIDATION_ERROR` (e.g. end date before start date).

### `delete_insurance_policy`

- **Input**: `id: number`, `confirmed: true`.
- **Output**: `{ deleted: true }`.
- **Errors**: `POLICY_HAS_FIREARMS` (Edge Case: deleting a policy with
  firearms still assigned is rejected, not silently cascaded — the
  frontend surfaces the affected firearm list and asks the user to
  reassign/unassign first).

### `assign_firearm_coverage`

- **Input**: `firearmId: number`, `{ policyId: number | null, coverageKind?: "individually_scheduled" | "blanket", scheduledCoverageAmount?: number }`.
- **Output**: `Firearm`.
- **Errors**: `VALIDATION_ERROR`.

### `get_value_summary`

- **Input**: `{}` (always reflects current active, non-disposed firearms per FR-025).
- **Output**:
  ```ts
  {
    collectionTotal: number;               // cents, all active firearms
    byPolicy: {
      policyId: number;
      policyName: string;
      isExpired: boolean;
      isExpiringSoon: boolean;
      blanketTotal: number;                // sum of blanket-covered firearm values
      blanketLimit: number;
      blanketUnderInsured: boolean;
      individuallyScheduled: {
        firearmId: number;
        estimatedValue: number;
        scheduledAmount: number;
        underInsured: boolean;
      }[];
    }[];
    unassigned: { firearmId: number; estimatedValue: number }[]; // uninsured group
  }
  ```
- **Recompute contract**: the frontend re-invokes this after every
  mutating command (`create_firearm`, `update_firearm`, `dispose_firearm`,
  `delete_firearm`, `assign_firearm_coverage`, policy CRUD) so the
  displayed summary is never stale (FR-015, SC-003) — no separate
  "refresh" action exists.

## Photos & documents (User Story 4)

### `add_photo`

- **Input**: `firearmId: number`, `fileBytes: Uint8Array`, `originalFilename: string`, `mimeType: string`.
- **Output**: `Photo` (with generated `thumbnailBytes`). First photo added
  to a firearm automatically becomes `thumbnail_photo_id` (FR-008).
- **Errors**: `VALIDATION_ERROR` (unsupported mime type), `NOT_FOUND`.

### `set_thumbnail_photo`

- **Input**: `firearmId: number`, `photoId: number`.
- **Output**: `Firearm`.
- **Errors**: `NOT_FOUND`.

### `delete_photo`

- **Input**: `photoId: number`, `confirmed: true`.
- **Output**: `{ deleted: true }`. If the deleted photo was the thumbnail,
  `thumbnail_photo_id` falls back to the next-oldest remaining photo, or
  `null` (generic thumbnail) if none remain.

### `add_document` / `delete_document` / `get_document`

- Analogous to photo commands, without thumbnail generation.
  `get_document` returns the full `file_bytes` for reopening (FR-010).

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
- **Output (progress events, then)**: `{ importedCount: number, updatedCount: number, skippedCount: number, rowErrors: { row: number; message: string }[], conflicts: ImportConflict[] }`.
- **Behavior**: Rows failing validation are reported per-row without
  discarding successful rows (FR-020). Rows matching an existing
  `(make, model, serial_number)` key produce an `ImportConflict` requiring
  resolution rather than being silently applied.

### `resolve_import_conflicts`

- **Input**: `{ importSessionId: string, resolutions: { conflictId: string; action: "skip" | "overwrite" | "duplicate" }[], applyToRemaining?: "skip" | "overwrite" | "duplicate" }`.
- **Output**: `{ resolvedCount: number }`.
- **Behavior**: Implements FR-026's per-row resolution plus "apply to all
  subsequent conflicting rows" option; `applyToRemaining` only affects
  conflicts not explicitly listed in `resolutions`.
