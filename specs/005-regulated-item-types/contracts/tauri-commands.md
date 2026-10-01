# Contract: Tauri IPC Commands (delta)

This is a **delta** against
[the 001 IPC contract](../../001-firearms-inventory/contracts/tauri-commands.md),
as amended by [002](../../002-firearm-identification/contracts/tauri-commands.md),
[003](../../003-database-protection-management/contracts/tauri-commands.md) and
[004](../../004-cartridges-action-types/contracts/tauri-commands.md). It lists
only the commands, shapes and errors that change or are added. The
`CommandError` shape, the session gating and every command not named here
are unchanged. Anchors amended: `create_firearm`, `update_firearm`,
`get_firearm`, `list_firearms`, `suggest_entries`, `settle_entry`,
`list_action_types` (content only), `import_collection`, and the `Firearm`,
`FirearmInput` and `FirearmSummary` shapes.

Two read-only commands are added: `list_firearm_types` and
`list_registration_classes`. Each goes through `session.read(...)` and is
registered in `main.rs`'s `generate_handler!`. No error code is added.

## Shared shape: registration

Added to `Firearm` (every command that returns one, and the `get_firearm`
detail) and to `FirearmInput`:

```ts
type Registration = {
  registrationClassId: number | null;  // an id from list_registration_classes; null = none
  registrationForm: string | null;     // free text; blank → null; ≤ 100 chars, no control chars
  registrationApproved: string | null; // "YYYY-MM-DD", not after today (local)
  registeredTo: string | null;         // free text; rules as registrationForm
};
```

All four may be omitted on input (treated as `null`), so callers built before
this feature keep working. The three details must be `null` when
`registrationClassId` is `null`. `registrationForm` and `registeredTo` are
trimmed before they are validated and stored.

## `create_firearm` / `update_firearm`

- **Input**: unchanged apart from the shape above.
- **Output**: `Firearm`.
- **Order of checks**: normalize → `validate_firearm_input` →
  **`check_fields_apply`** (new) → `check_action_allowed` → uniqueness → the
  original-marks warning.
- **New `VALIDATION_ERROR` field errors**:
  - `actionTypeId`, `barrelLengthHundredths`, `capacity`: "Action doesn't
    apply to a Suppressor." / "Barrel length doesn't apply to a Suppressor." /
    "Capacity doesn't apply to a Suppressor." when the type omits the field
    and it has a value (FR-003). The backend never clears them itself. The
    form leaves them out of the input (contracts/ui-registration.md §1).
  - `registrationForm`, `registrationApproved`, `registeredTo`: "Choose what
    the firearm is registered as first." when set without a classification
    (FR-009).
  - `registrationClassId`: "Choose a classification from the list." when the
    id names none. A classification that is no longer offered is accepted
    (research.md §5).
  - `registrationApproved`: "Approved date can't be in the future." /
    "Approved date must be a date in YYYY-MM-DD format." (FR-010).
  - `registrationForm`, `registeredTo`: "Form can be at most 100
    characters." / "Registered to can't contain control characters."
    (004 FR-015). On `update_firearm`, only for a value that changed.
- **Not done here**: snapping. The values given are the values stored, as in
  004.

## `get_firearm`

- **Output**: `FirearmDetail`, whose `Firearm` now carries the registration
  shape. The classification's name comes from `list_registration_classes`.

## `list_firearms`

- **Input**: `groupBy?: … | "registered_as" | "registered_to"` (adds the last
  two).
- **Output**: `FirearmSummary` gains:

  ```ts
  registeredAs: string | null;  // the classification's name; not displayed in browse (FR-018)
  ```

- **Grouping**:
  - With `"registered_as"`, the group `key` is the classification's name,
    groups in the list's `sort_order`.
  - With `"registered_to"`, the key is the stored text, groups sorted
    alphabetically.
  - In both, firearms with none form the group `"Unspecified"`, returned
    last. For `"registered_to"`, that includes every firearm with no
    classification (FR-016).
  - With `"type"`, Suppressor is a group like any other.
- **Search**: `query` also matches the classification's name, the form and
  "Registered to" (FR-017), with the existing semantics: "short-barreled"
  finds "Short-barreled rifle", "form 1" finds "Form 1" (and "Form 10"),
  "smith family" finds "Smith Family Trust".
- **Performance**: unchanged budget, 500 ms at 10,000 records.

## `suggest_entries` / `settle_entry`

- **Input**: `field` gains `"registrationForm"` and `"registeredTo"`
  (`EntryField` is now serialized camelCase; the four existing names are
  unchanged). `make` is still read for `"model"` only.
- **Behavior**:
  - `"registrationForm"` offers the built-in form names (data-model.md) with
    `inCatalog: true`, then the forms on record. `settle_entry` snaps to a
    built-in name first (`changedBy: "catalog"`), then to the record
    spelling.
  - `"registeredTo"` offers only the values on record, and snaps to them
    (`changedBy: "record"`).
  - `caliber` and `derivedCaliber` stay `null` for both.
- **Performance contract**: within 50 ms at 10,000 firearms, as in 004.
- **Privacy**: reads `firearms` at call time and keeps nothing, so a value
  used only by deleted firearms is never offered (FR-013).

## `list_firearm_types` (new)

The type list and which fields apply to each type (FR-001, FR-003;
research.md §3).

- **Input**: none.
- **Output**:

  ```ts
  type FirearmTypesOutput = {
    types: {
      id: number;
      name: string;                 // "Suppressor"
      genericThumbnailKey: string;  // "suppressor"
      actionTypeApplies: boolean;
      barrelLengthApplies: boolean;
      capacityApplies: boolean;
    }[];                            // by sort_order: Other last
  };
  ```

- Loaded once per open database by `CollectionProvider`. It replaces the
  frontend's hard-coded `FIREARM_TYPE_OPTIONS`.

## `list_registration_classes` (new)

The classification list (FR-007).

- **Input**: none.
- **Output**:

  ```ts
  type RegistrationClassesOutput = {
    classes: { id: number; name: string; offered: boolean }[];  // in list order
  };
  ```

  Every classification is returned, including those no longer offered, so a
  record holding one can show its name. The form offers `offered` entries
  plus the record's own (contracts/ui-registration.md §2).
- Loaded once per open database by `CollectionProvider`.

## `list_action_types`

- **Shape unchanged.** The output now includes "Automatic or select-fire"
  (id 13, seventh in list order, after Break action), and `allowedByFirearmType` lists it for
  types 1–3. Suppressor (5) is absent from `allowedByFirearmType`, which by
  004's reading allows every action. The frontend must check
  `actionTypeApplies` from `list_firearm_types` first, as the backend does.

## `import_collection`

- **Input**: unchanged.
- **Output**: `ImportResult` is unchanged in shape. `snappedValues[].field`
  may now be `"registrationForm"` or `"registeredTo"`.
- **New row errors** (in `rowErrors`, FR-021, FR-022): see
  [spreadsheet-format.md](./spreadsheet-format.md).

## `export_collection`

- **Input and output**: unchanged. The file gains four columns
  ([spreadsheet-format.md](./spreadsheet-format.md)).

## `dispose_firearm`, `reverse_disposition`, `assign_firearm_coverage`

- Unchanged. They rebuild the input from the stored record, so registration
  details are kept (FR-013).
