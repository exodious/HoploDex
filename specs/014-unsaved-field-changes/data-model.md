# Data Model: Unsaved Changes per Field, with Revert

Feature 014. No table, column or migration changes (spec Assumptions, "No data changes").

## Frontend state

- **Saved value** (spec Key Entities): the form state built from the saved record when the form opens (`initialValues`), never from a resumed draft. One per form; read-only while it is open.
- **Field change**: `stored(field, current) !== stored(field, saved)`, where `stored` returns what a save would store for the field, or `UNREADABLE` (a value that can't be parsed yet; FR-003). `changedFields` returns `changed: ReadonlySet<Field>`, and `isDirty = changed.size > 0` (research.md §1).
- **Mount change** (firearm and accessory edit forms of an active record): `mounted: { adds: { label: RecordLabel; from: RecordLabel | null }[]; removes: string[] }`, removals by record key, the two disjoint; an entry in neither is unchanged. Each row is marked and reverted on its own (research.md §8).
- **Kept draft** (003 FR-039): unchanged in shape apart from the firearm and accessory forms' `values`, which gain `mounted`; their `FORM_VERSION` goes up (firearm 5, accessory 2).

## Command inputs

- `update_firearm`'s and `update_accessory`'s new `mountChanges` argument: `{ item: RecordRef; mounted: boolean }[]`, default `[]`, at most 1,000. Rules: ignored when the record is saved as disposed (it has no Mounted list); a removal's item must be directly on the record; an addition's item must be active, not the record, and not form a loop once the record's own "Mounted on" is saved; one item at most once. See [contracts/tauri-commands.md](contracts/tauri-commands.md).
