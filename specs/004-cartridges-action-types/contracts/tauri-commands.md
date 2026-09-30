# Contract: Tauri IPC Commands (delta)

This is a **delta** against
[the 001 IPC contract](../../001-firearms-inventory/contracts/tauri-commands.md),
as amended by [002](../../002-firearm-identification/contracts/tauri-commands.md)
and [003](../../003-database-protection-management/contracts/tauri-commands.md).
It lists only the commands, shapes and errors that change or are added; the
`CommandError` shape, the session gating (`DATABASE_CLOSED`,
`PENDING_CHANGES_UNRESOLVED`, …) and every command not named here are
unchanged. Anchors amended: `create_firearm`, `update_firearm`,
`get_firearm`, `list_firearms`, `import_collection`, and the `Firearm`,
`FirearmInput` and `FirearmSummary` shapes.

Three read-only commands are added: `suggest_entries`, `settle_entry` and
`list_action_types`. Each goes through `session.read(...)` and is registered
in `main.rs`'s `generate_handler!`. No error code is added.

## Shared shape: cartridge and action type

Added to `Firearm` (every command that returns one, and the `get_firearm`
detail) and to `FirearmInput`:

```ts
type CartridgeAndAction = {
  cartridge: string | null;      // free text; blank → null; ≤ 100 chars, no control chars
  actionTypeId: number | null;   // an id from list_action_types; null = not specified
};
```

Both may be omitted on input (treated as `null`), so callers built before this
feature keep working (FR-021).

Also on input, **make, model and caliber are now trimmed** before they are
validated and stored (research.md §9).

## `create_firearm` / `update_firearm`

- **Input**: unchanged apart from the shape above.
- **Output**: `Firearm`.
- **New `VALIDATION_ERROR` field errors**:
  - `make`, `model`, `cartridge`, `caliber`: "Make can be at most 100
    characters." / "Make can't contain control characters." (FR-015). On
    `update_firearm`, only for a field whose trimmed value differs from the
    stored one (spec Assumptions: an existing over-long value stays valid
    until that field is edited).
  - `actionTypeId`: "Lever action doesn't apply to a Handgun." when the
    firearm's type has mapped actions and this is not one of them (FR-017,
    FR-019); "Choose an action from the list." when the id names no action.
- **Not done here**: snapping and caliber derivation. The values given are the
  values stored (research.md §6).

## `get_firearm`

- **Output**: `FirearmDetail`, whose `Firearm` now carries `cartridge` and
  `actionTypeId`. The action's name comes from `list_action_types`.

## `list_firearms`

- **Input**: `groupBy?: "type" | "caliber" | "make" | "origin" | "cartridge" | "action_type"`
  (adds the last two).
- **Output**: `FirearmSummary` gains:

  ```ts
  cartridge: string | null;
  actionTypeName: string | null;
  ```

- **Grouping**: with `"cartridge"` the group `key` is the stored cartridge text
  (so spelling variants already on record are separate groups, spec Edge
  Cases), groups sorted alphabetically; with `"action_type"` it is the
  action's name, groups in the action list's order. In both, firearms with
  none form a group keyed **`"Unspecified"`**, returned last
  (research.md §11). **Changed from 002:** with `"origin"`, the group for
  firearms with no origin is keyed `"Unspecified"` (was `"Not specified"`),
  still last.
- **Search**: `query` also matches the cartridge text and the action's name
  (FR-008, FR-020), with the existing phrase-with-trailing-prefix semantics:
  "7.62x39" finds "7.62x39mm", "bolt" finds "Bolt action".
- **Performance**: unchanged budget, 500 ms at 10,000 records.

## `suggest_entries` (new)

The suggestion list for one field, computed at call time (FR-009 to FR-012,
FR-016; research.md §4–§5).

- **Input**:

  ```ts
  type SuggestEntriesInput = {
    field: "make" | "model" | "cartridge" | "caliber";
    text: string;           // what is typed so far; may be empty
    make?: string | null;   // model only: the make on the form, ranked first
  };
  ```

- **Output**:

  ```ts
  type Suggestion = {
    value: string;          // display spelling (catalog's when it has one)
    inCatalog: boolean;     // built-in (FR-016)
    useCount: number;       // firearms on record, active and disposed; 0 = catalog only
    caliber: string | null; // catalog cartridges only: the class, a hint
  };
  type SuggestEntriesOutput = { suggestions: Suggestion[] };  // at most 20, best first
  ```

- **Errors**: `VALIDATION_ERROR` for an unknown `field` (serde rejects it).
  `text` longer than 100 characters is not an error; it simply matches
  nothing.
- **Performance contract**: within 50 ms at 10,000 firearms with 10,000
  distinct values in the field (SC-004 budgets 100 ms per keystroke end to
  end; `tests/performance_test.rs`).
- **Privacy**: reads only `firearms` and the catalog; writes and keeps
  nothing (FR-011).

## `settle_entry` (new)

What a value becomes when the user finishes entering it (FR-013), and for a
cartridge, the caliber it derives (FR-003, FR-005). Research.md §6–§7.

- **Input**:

  ```ts
  type SettleEntryInput = {
    field: "make" | "model" | "cartridge" | "caliber";
    text: string;
  };
  ```

- **Output**:

  ```ts
  type SettleEntryOutput = {
    value: string;                              // trimmed; the snapped spelling if any
    changedBy: "catalog" | "record" | null;     // null = kept as typed (apart from trimming)
    derivedCaliber: {                           // cartridge only; null otherwise
      caliber: string;                          // already snapped against calibers
      source: "catalog" | "guess";
    } | null;                                   // null for a cartridge = no caliber could be read
  };
  ```

- **Errors**: none beyond session gating. A value that breaks FR-015 is
  returned trimmed and unchanged (`changedBy: null`); the form reports the
  rule itself, and the backend enforces it on save.

## `list_action_types` (new)

The fixed action list and its mapping to firearm types (FR-017, FR-018).

- **Input**: none.
- **Output**:

  ```ts
  type ActionTypesOutput = {
    actions: { id: number; name: string }[];   // in list order
    allowedByFirearmType: Record<number, number[]>;
    // firearm type id → allowed action ids, in list order. A type that is
    // absent, or maps to [], allows every action (FR-017).
  };
  ```

- Loaded once per open database by `CollectionProvider`.

## `import_collection`

- **Input**: unchanged.
- **Reading** (research.md §12): columns are now matched **by header name**
  (trimmed, any letter case), not position. Unknown columns are ignored; a
  missing known column reads as blank. A header naming a column twice fails
  the whole import with `VALIDATION_ERROR` before any row is read.
- **Output**: `ImportResult` gains:

  ```ts
  derivedCalibers: {
    row: number;
    cartridge: string;           // as recorded (after snapping)
    caliber: string;             // as recorded
    source: "catalog" | "guess";
  }[];
  snappedValues: {
    row: number;
    field: "make" | "model" | "cartridge" | "caliber";
    sheetValue: string;          // as in the sheet, trimmed
    recordedValue: string;
  }[];
  ```

  Both list rows that were imported or became conflicts, never failed rows.
  The number of values changed by snapping is `snappedValues.length`
  (FR-025, FR-026, SC-007).
- **New row errors** (in `rowErrors`, FR-020):
  - `make`/`model`/`cartridge`/`caliber` over 100 characters or with control
    characters (FR-015, FR-026).
  - `caliber` blank and none derivable from `cartridge`, or both blank
    (FR-025).
  - `action_type` not on the list, or not allowed for the row's
    `firearm_type` (FR-024).
- **Matching** (001 FR-026, as amended by 002) compares the snapped make and
  model.

## `resolve_import_conflicts`

- Unchanged. A conflict's pending input was snapped and derived at import, so
  `overwrite` and `duplicate` save the values the import report listed.
