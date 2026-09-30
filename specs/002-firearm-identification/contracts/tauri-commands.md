# Contract: Tauri IPC Commands (delta)

This is a **delta** against
[the 001 IPC contract](../../001-firearms-inventory/contracts/tauri-commands.md).
It lists only the commands, shapes and errors that change; the common
`CommandError` shape, the async/progress conventions and every command not
named here are unchanged. Anchors in 001 amended here: `create_firearm`,
`update_firearm`, `reverse_disposition`, `get_firearm`, `list_firearms`,
`import_collection`, `resolve_import_conflicts`, and the `FirearmInput` /
`Firearm` shapes.

No command is added.

## Shared shape: identification fields

Added to `Firearm` (output of every command that returns one, and the
`get_firearm` detail) and to `FirearmInput`:

```ts
type Origin = "domestic" | "imported" | "reimported";

type IdentificationFields = {
  origin: Origin | null;                 // null = not specified; always allowed
  yearOfManufacture: number | null;      // any firearm; 1400..current local year
  countryOfManufacture: string | null;   // origin "imported" only
  importerName: string | null;           // import-marked origins only; a name, never a location
  originalMake: string | null;           // import-marked origins only
  originalModel: string | null;          // import-marked origins only
  originalSerialNumber: string | null;   // import-marked origins only
};
```

All seven are nullable and, on input, may be omitted (treated as `null`) so
that callers built before this feature keep working. A blank string is
stored as `null` after trimming. `FirearmSummary` (list results) does **not**
carry them: list and tile views show only the main make, model and nickname
(FR-013).

The `Firearm` output never contains "United States" for a re-imported
firearm: `countryOfManufacture` is `null` for it, and the frontend shows the
United States from the origin (data-model.md, Derived display values).

## `create_firearm`

- **Input**: `input: FirearmInput` (now including `IdentificationFields`),
  `confirmedWarnings?: boolean` (default `false`).
- **Output**: `Firearm`.
- **New/changed errors**:
  - `VALIDATION_ERROR` with `fieldErrors`:
    - `yearOfManufacture` — not a four-digit year from 1400 to the current
      local year (FR-003).
    - `importerName`, `originalMake`, `originalModel`,
      `originalSerialNumber` — set on a record whose origin is not
      `imported` or `reimported` (FR-002, FR-004).
    - `countryOfManufacture` — set on a record whose origin is not
      `imported`, including `reimported` (FR-002).
    - `serialNumber` — the main make/model/serial matches another active
      firearm and the pair is not distinguished by year (FR-007, FR-008).
      **Changed from 001:** the message now also says that a year of
      manufacture on each record is what would distinguish the two firearms,
      and the pair is **accepted** (no error) when both records have a year
      and the years differ.
  - `ORIGINAL_MARKS_MATCH` (**new code**, FR-009): the record is active, has
    original maker, model and serial all recorded, and another active
    firearm has the same three (ignoring case and surrounding whitespace).
    Nothing is saved. `message` names the other firearm and is safe to show
    verbatim. No `fieldErrors`. Never returned when `confirmedWarnings` is
    `true`, when the set is partial, or when the only match is disposed.

## `update_firearm`

- **Input**: `id: number`, `input: FirearmInput`, `confirmedWarnings?: boolean`.
- **Output**: `Firearm`.
- **Errors**: as `create_firearm`, with the record itself excluded from both
  the identity rule and the warning. Editing a record to the same year as
  another with identical main marks is blocked; editing it to a different
  year, or to a year where the other record had none, follows the rule in
  data-model.md (the pair must both have a year and differ).

## `reverse_disposition`

- **Input**: `id: number`, `{ history: "keep" | "discard", nickname?: string | null, confirmedWarnings?: boolean }`.
- **Output**: `Firearm`.
- **Errors**: as before, plus, from the restored record against the firearms
  active now:
  - `VALIDATION_ERROR` on `serialNumber` for an identity conflict not
    distinguished by year (FR-008), naming the other record; nothing is
    changed.
  - `ORIGINAL_MARKS_MATCH` for an original-marks match (FR-009); nothing is
    changed, including the `keep` history row, because the whole reversal is
    one transaction. Resend with `confirmedWarnings: true` to proceed.

## `dispose_firearm`, `assign_firearm_coverage`, `set_thumbnail_photo`

Unchanged. They start from the stored record and change nothing
identifying, so they never return `ORIGINAL_MARKS_MATCH` (research.md §5).
`dispose_firearm` produces a non-active record, which neither the identity
rule nor the warning considers.

## `get_firearm`

- **Output**: `FirearmDetail` = `Firearm` (with `IdentificationFields`) +
  `dispositionHistory`. No other change.

## `list_firearms`

- **Input**: `groupBy?: "type" | "caliber" | "make" | "origin"` (adds
  `"origin"`).
- **Output**: unchanged shape. With `groupBy: "origin"` the group `key` is
  `"Domestic"`, `"Imported"`, `"Re-imported"` or `"Not specified"` (firearms
  with no origin), returned in that fixed order and only for origins present
  (research.md §10).

_Amended by [spec 004](../../004-cartridges-action-types/contracts/tauri-commands.md): the `"Not specified"` group
key reads `"Unspecified"`._
- **Search semantics**: `query` also matches, as displayed, the origin label,
  year, country (including "United States" for a re-imported firearm),
  importer name, original make, original model and original serial number
  (FR-012). Phrase-with-trailing-prefix semantics are unchanged, so
  "imported" finds imported and re-imported firearms, "re-imported" finds
  only re-imported ones, and "domestic" finds only domestic ones.
- **Performance contract**: unchanged — within 500 ms at 10,000 records.

## `import_collection`

- **Input**: unchanged.
- **Output**: `ImportResult` gains `warnings: { row: number; message: string }[]`
  — the FR-009 original-marks matches for rows that were imported anyway
  (each message names the existing firearm). Rows are still counted in
  `importedCount`. `warnings` is separate from `rowErrors`; a row can appear
  in `warnings` only if it did not fail.
- **Behavior changes** (FR-014):
  - Matching key is `(make, model, serial_number)` plus the year rule: a row
    is a **new record** (no conflict) when an existing record has identical
    main marks but both have a year and the years differ (US4-5a). Original
    marks are never used to match.
  - A row whose origin is not `Domestic`, `Imported` or `Re-imported`
    (ignoring case), whose year fails FR-003, or that carries importer,
    country or original-marks values its origin does not allow (country is
    also disallowed on a `Re-imported` row) is a row error naming the value
    or column (US4-3, US4-4, US4-7).
  - `ImportConflict.duplicateAllowed` is `false` whenever the FR-007/FR-008
    rule would block the resulting record. For two records with identical main
    marks that never varies: if both years exist and differ there is no
    conflict at all (previous bullet), otherwise `duplicateAllowed` is
    `false`, so the "create a duplicate" option is never offered for
    identical-mark records.
  - Rows are saved in file order, so the identity rule and the warning also
    see earlier rows of the same file.

## `resolve_import_conflicts`

- **Output**: `ResolveResult` gains
  `warnings: { row: number; message: string }[]` for rows saved by an
  `overwrite` or `duplicate` resolution that match another active firearm's
  original marks. They do not put the conflict in `unresolved`.
- **Behavior**: `overwrite` and `duplicate` apply the year rule and the
  identity rule exactly as `update_firearm` / `create_firearm` do (a clash the
  rule blocks is `unresolved`, as in 001).

## New error code

| Code | When | Frontend response |
|---|---|---|
| `ORIGINAL_MARKS_MATCH` | Original maker, model and serial all match another active firearm and `confirmedWarnings` is not `true` (create, update, reverse) | Shared `ConfirmDialog` showing `message`, "Save anyway" resends with `confirmedWarnings: true`; cancel returns to the form with nothing saved |
