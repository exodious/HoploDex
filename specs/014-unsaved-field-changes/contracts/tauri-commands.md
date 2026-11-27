# Command contract changes: feature 014

Only existing commands change. Shapes are in 006 [contracts/tauri-commands.md](../../006-accessory-links/contracts/tauri-commands.md); this file lists what 014 adds.

## `update_firearm`, `update_accessory`

```ts
input: { id, input: FirearmInput | AccessoryInput }
// FirearmInput and AccessoryInput gain:
mountChanges?: { item: RecordRef; mounted: boolean }[];   // default [], at most 1,000
```

- Applied in the same transaction as the record and its own `mountedOn`, after them; all or none (014 FR-027).
- `mounted: true`: the item is mounted on this record, moving from wherever it is, with everything on it (006 FR-010). A loop, an inactive or missing item, or the record itself: `VALIDATION` with `fieldErrors.mounted`.
- `mounted: false`: the item must be directly on this record now, else `CONFLICT` ("What is mounted on this record has changed. Close the form and open it again."); it is unmounted, keeping what is on it.
- An item listed twice, or more than 1,000 entries: `VALIDATION`.
- Ignored when the input's status is disposed.
- `create_firearm` and `create_accessory` refuse a non-empty list (`VALIDATION`): an add form has no Mounted list.

## `list_mount_candidates`

```ts
input: { role, record, query, limit?, hostMountedOn?: RecordRef | null }
```

- `hostMountedOn`, with `role: "item"` only: the host's "Mounted on" as the open form holds it. When given, the host's chain is that record and its chain instead of the saved one (014 FR-026). Absent: as in 006.
