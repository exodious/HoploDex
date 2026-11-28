# Command contract changes: feature 014

Only existing commands change. Shapes are in 006 [contracts/tauri-commands.md](../../006-accessory-links/contracts/tauri-commands.md); this file lists what 014 adds.

## `update_firearm`, `update_accessory`

```ts
input: {
  id: number;
  input: FirearmInput | AccessoryInput;   // unchanged
  confirmedWarnings?: boolean;            // update_firearm only, unchanged
  mountChanges?: { item: RecordRef; mounted: boolean }[];   // new; default []
}
```

- A separate argument, beside `input`: the form's Mounted list, not a field of the record (014 FR-024). `create_firearm` and `create_accessory` take none: an add form has no Mounted list.
- Applied after the record and its own `mountedOn`, in one savepoint with them: all or none (014 FR-027).
- `mounted: true`: the item is mounted on this record, moving from wherever it is, with everything on it (006 FR-010). A loop, a depth past 16, or an inactive or missing item: `VALIDATION_ERROR` with `fieldErrors.mounted` (the messages `mount_record` gives).
- `mounted: false`: the item must be directly on this record now, else `VALIDATION_ERROR` with `fieldErrors.mounted` "What is mounted on this record has changed. Close the form and open it again."; it is unmounted, keeping what is on it.
- An item listed twice: the same stale-list error. More than 1,000 entries (`MAX_MOUNT_CHANGES`): `VALIDATION_ERROR` on `mounted`.
- Ignored when the input's status is disposed (a disposed record has no Mounted list, 014 FR-024).

## `list_mount_candidates`

```ts
input: { role, record, query, limit?, hostMountedOn?: { on: RecordRef | null } }
```

- `hostMountedOn`, with `role: "item"` only: the host's Mounted on as the open form holds it (`on: null`: not mounted). When given, the host's chain is that record and its chain instead of the saved one (014 FR-026). Absent: as in 006.

## Unchanged

`mount_record` stays, as 006 describes it; the record page no longer calls it (014 FR-028).
