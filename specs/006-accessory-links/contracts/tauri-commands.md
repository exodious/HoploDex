# Contract: Tauri IPC Commands (delta)

This is a **delta** against
[the 001 IPC contract](../../001-firearms-inventory/contracts/tauri-commands.md),
as amended by [002](../../002-firearm-identification/contracts/tauri-commands.md),
[003](../../003-database-protection-management/contracts/tauri-commands.md),
[004](../../004-cartridges-action-types/contracts/tauri-commands.md) and
[005](../../005-regulated-item-types/contracts/tauri-commands.md). It lists
only what changes or is added. The `CommandError` shape is unchanged, and
no error code is added. Every collection command goes through
`session.read(...)` or `session.write(...)` as before, and every new
command is registered in `main.rs`'s `generate_handler!`.

**Anchors amended**:
- the commands `create_firearm`, `update_firearm`, `dispose_firearm`,
  `get_firearm`, `list_firearms`, the photo and document commands,
  `assign_firearm_coverage` (content only), `get_value_summary`,
  `get_policy_deletion_impact`, `delete_insurance_policy` (content only),
  `export_collection`, `import_collection`, `resolve_import_conflicts`,
  `suggest_entries` (content only), `stage_pending_changes` (content only);
- the shapes `Firearm`, `FirearmInput` and `FirearmSummary`.

**Added**:
- for accessories: `list_accessory_kinds`, `create_accessory`,
  `update_accessory`, `get_accessory`, `list_accessories`,
  `dispose_accessory`, `reverse_accessory_disposition`, `delete_accessory`
  and `assign_accessory_coverage`;
- for mounts: `mount_record` and `list_mount_candidates`;
- for export: `get_export_scope`.

## Shared shapes

```ts
type RecordKind = "firearm" | "accessory";
type RecordRef = { kind: RecordKind; id: number };

/** Enough to name a record the way it is named everywhere (FR-005; a
 *  firearm by make, model and nickname, 001 FR-031) and link to it. */
type RecordLabel = {
  record: RecordRef;
  make: string | null;          // always set for a firearm
  model: string | null;         // always set for a firearm
  nickname: string | null;      // firearms only
  typeName: string;             // the firearm type's or the accessory kind's name
  serialNumber: string | null;  // shown only where FR-012 searches by it
  status: "active" | "disposed";
};

/** One entry of a Mounted section or a dispose dialog's list (FR-013,
 *  FR-014): depth-first, below the record asked about. */
type MountedEntry = {
  label: RecordLabel;
  host: RecordRef;   // what it is mounted on (the record itself at depth 1)
  depth: number;     // 1 = mounted directly on the record
};

/** What a record page needs about mounts (FR-013). */
type MountDetail = {
  chain: RecordLabel[];     // direct host first, then its host, …; [] when not mounted
  mounted: MountedEntry[];  // everything below; [] on a disposed record
};
```

## Firearms (amended)

- **`Firearm`** gains `mountedOn: RecordRef | null`, the direct host. The
  record identifier is never sent (FR-019).
- **`FirearmInput`** gains `mountedOn: RecordRef | null`. The forms always
  send it. A host that FR-010 doesn't allow fails with `VALIDATION_ERROR`
  and `fieldErrors.mountedOn`, which is one of:
  - "Choose an active firearm or accessory." (the host is missing or
    disposed);
  - "A firearm can't be mounted on itself, or on something mounted on it."
  A record that is being disposed can't be mounted: the dispose commands
  clear it.
- **`validate_firearm_input`**: `dispositionPrice` is no longer required
  when `status` is `disposed` (research.md §9). Every other disposition
  rule is unchanged.
- **`get_firearm`** → `FirearmDetail` gains `mount: MountDetail`.
- **`list_firearms`**: `FirearmSummary` gains `mountedOn: RecordLabel | null`
  and `mountedCount: number`, which counts everything below, at any depth
  (FR-016a). Search and `groupBy` are unchanged.
- **`dispose_firearm`**:

  ```ts
  type DisposeInput = {
    dispositionType: DispositionType;
    recipient: string;
    date: string;       // YYYY-MM-DD
    price: number;      // the host's own price, required as before
    withMounted?: { record: RecordRef; price: number | null }[];  // default []
  };
  ```

  It runs in one transaction (research.md §9):
  1. The host is unmounted.
  2. Each `withMounted` record is disposed with the host's type, recipient
     and date, and its own price. A `null` price means no price.
  3. Every mount that involves a disposed record is removed.
  4. Kept records keep their mounts on kept records.

  A `withMounted` record that is not below the host fails the call, with
  nothing changed: `VALIDATION_ERROR`,
  `fieldErrors.withMounted = "What is mounted has changed. Close the dialog
  and try again."`. It returns the host `Firearm`.
- **`reverse_disposition`**: unchanged. The restored firearm is not
  mounted, and nothing is mounted on it.
- **`delete_firearm`**: unchanged. The `mounts` cascade leaves the records
  that were mounted on it in the collection, unmounted.

## Accessories (new)

Each mirrors its firearm counterpart, with the same session gating, error
codes and transaction behavior.

| Command | Input | Output |
|---|---|---|
| `list_accessory_kinds` | none | `{ kinds: AccessoryKind[] }`: every kind, offered or not, in `sortOrder` |
| `create_accessory` | `{ input: AccessoryInput }` | `Accessory` |
| `update_accessory` | `{ id, input: AccessoryInput }` | `Accessory` |
| `get_accessory` | `{ id }` | `AccessoryDetail` |
| `list_accessories` | `{ input: ListAccessoriesInput }` | `{ groups: AccessoryGroup[] }` |
| `dispose_accessory` | `{ id, input: DisposeInput }` | `Accessory` |
| `reverse_accessory_disposition` | `{ id, input: { history: "keep" \| "discard" } }` | `Accessory` |
| `delete_accessory` | `{ id, confirmed: boolean }` | `{ deleted: true }` |
| `assign_accessory_coverage` | `{ accessoryId, policyId: number \| null, scheduledCoverageAmount: number \| null }` | `Accessory` |

```ts
type AccessoryKind = {
  id: number;
  name: string;
  genericThumbnailKey: string;
  sortOrder: number;
  offered: boolean;   // false: still shown on records that hold it, not offered for new choices
};

type AccessoryInput = {
  accessoryKindId: number;             // required; any kind that exists
  make: string | null;                 // 004 entry rules when set
  model: string | null;
  serialNumber: string | null;         // free text; no uniqueness (FR-004)
  caliber: string | null;              // 004 entry rules; derived from cartridge on the form
  cartridge: string | null;
  notes: string | null;
  status: "active" | "disposed";
  estimatedValue: number | null;       // whole dollars
  acquisitionSource: string | null;
  acquisitionDate: string | null;      // YYYY-MM-DD, not in the future
  acquisitionPrice: number | null;
  dispositionType: DispositionType | null;
  dispositionRecipient: string | null;
  dispositionDate: string | null;
  dispositionPrice: number | null;
  insurancePolicyId: number | null;
  scheduledCoverageAmount: number | null;
  mountedOn: RecordRef | null;         // FR-010; same errors as FirearmInput's
};

type Accessory = AccessoryInput & {
  id: number;
  thumbnailPhotoId: number | null;
  createdAt: string;
  updatedAt: string;
};

type AccessoryDetail = Accessory & {
  dispositionHistory: DispositionHistoryEntry[];  // as FirearmDetail's
  mount: MountDetail;
};

type ListAccessoriesInput = {
  query?: string | null;             // FR-018: every text field; 1–2 chars use LIKE
  groupBy?: "kind" | "make" | "caliber" | "cartridge" | "mounted_on" | null;
  includeDisposed?: boolean;         // default false (001 FR-025)
};

type AccessorySummary = {
  id: number;
  accessoryKindId: number;
  kindName: string;
  genericThumbnailKey: string;
  make: string | null;
  model: string | null;
  serialNumber: string | null;
  caliber: string | null;
  cartridge: string | null;
  status: "active" | "disposed";
  thumbnailPhotoId: number | null;
  estimatedValue: number | null;
  insuranceWarning: "none" | "uninsured" | "under_insured";
  insurancePolicyId: number | null;
  scheduledCoverageAmount: number | null;
  mountedOn: RecordLabel | null;     // direct host only (FR-013)
};

type AccessoryGroup = {
  key: string;                // the group's heading: a kind, make, caliber,
                              // cartridge, "Unspecified", "Not mounted", or "All"
  host: RecordLabel | null;   // set only when grouped by mounted_on, for a host's group
  accessories: AccessorySummary[];
};
```

**Group order** (FR-017):
- `kind`: the kind list's order.
- `make`, `caliber`, `cartridge`: alphabetical, with "Unspecified" last.
- `mounted_on`: one group per host **record**, sorted by its name, with
  "Not mounted" last. Two hosts with the same name are two groups.
- No grouping: one group, "All".
- Within a group: make, model, then kind.

**Validation messages** specific to accessories: "Choose a kind."
(`accessoryKindId`) when it is missing or unknown. The rest are worded as
for a firearm, with "accessory" for "firearm" where the message names the
record.

## Mounts (new)

### `mount_record`

```ts
input: { item: RecordRef; host: RecordRef | null }   // null = unmount
output: { item: RecordLabel; host: RecordLabel | null }
```

- It moves the item in one step (FR-012). Everything mounted on it stays
  mounted on it (FR-010).
- No move confirmation is asked here: the frontend asks before calling,
  when the item is listed with a current host (FR-012).
- It fails with `VALIDATION_ERROR` and `fieldErrors.host` set to the same
  two messages as `mountedOn`, and with `NOT_FOUND` when the item doesn't
  exist.
- It doesn't check any kind, type, caliber or cartridge (FR-011).

### `list_mount_candidates`

```ts
input: {
  role: "host" | "item";
  record: RecordRef | null;   // role "host": the item being placed (null for a new record)
                              // role "item": the host receiving it (required)
  query: string;              // matched in make, model, nickname, serial number; "" = first 50
  limit?: number;             // default 50, at most 100
}
output: {
  candidates: { label: RecordLabel; mountedOn: RecordLabel | null }[];
}
```

- **`role: "host"`** returns every active firearm and accessory except the
  record and everything below it (FR-010, US2-6, US2-16).
- **`role: "item"`** returns every active firearm and accessory except the
  host, the host's chain and what is already directly on the host (US2-4,
  US2-3a).
- **Matching** is case-insensitive, matches inside a value, and ignores
  surrounding whitespace.
- **Order**: by name (make, model, nickname or kind), then id.
- It is read-only.

## Photos and documents (amended)

The commands that take a firearm id now take an owner:

| Command | Was | Now |
|---|---|---|
| `list_photos` | `{ firearmId }` | `{ owner: RecordRef }` |
| `add_photo` | `{ firearmId, fileBytes, originalFilename, mimeType }` | `{ owner, fileBytes, originalFilename, mimeType }` |
| `add_photo_from_path` | `{ firearmId, path }` | `{ owner, path }` |
| `set_thumbnail_photo` | `{ firearmId, photoId }` → `Firearm` | `{ owner, photoId }` → `{ thumbnailPhotoId: number }` |
| `list_documents` | `{ firearmId }` | `{ owner: RecordRef }` |
| `add_document` | `{ firearmId, fileBytes, originalFilename, mimeType }` | `{ owner, fileBytes, originalFilename, mimeType }` |
| `add_document_from_path` | `{ firearmId, path }` | `{ owner, path }` |

`PhotoSummary` and `DocumentSummary` replace `firearmId` with
`owner: RecordRef`. The first photo still becomes the thumbnail (001
FR-008). A photo that belongs to a different owner than the one named is
`NOT_FOUND`, as today. `delete_photo`, `get_photo_thumbnail`,
`get_photo_original`, `delete_document` and `open_document` are unchanged.

## Insurance (amended)

- **`get_value_summary`** → `ValueSummary`:

  ```ts
  {
    collectionTotal: number;      // firearmsTotal + accessoriesTotal (SC-005)
    firearmsTotal: number;        // new
    accessoriesTotal: number;     // new: FR-008's subtotal
    blanket: {
      policyId, policyName, limit, total, underInsured,
      firearmCount: number;
      accessoryCount: number;     // new
    } | null;
    byPolicy: {
      policyId, policyName, isExpired, isExpiringSoon,
      individuallyScheduled: {
        record: RecordRef;        // was firearmId
        estimatedValue, scheduledAmount, underInsured,
      }[];
    }[];
    uninsured: { record: RecordRef; estimatedValue: number }[];   // was firearmId
  }
  ```

- **`get_policy_deletion_impact`**: `scheduledFirearms` becomes
  `scheduledRecords: RecordLabel[]`. `scheduledFirearmCount` and
  `blanketFirearmCount` become `scheduledRecordCount` and
  `blanketRecordCount`, counting firearms and accessories together
  (FR-009).
- **`delete_insurance_policy`**: unchanged input. Moving or unscheduling
  now covers the policy's accessories as well as its firearms, in the same
  transaction.

## Export and import (amended)

### `get_export_scope` (new, read-only)

```ts
input: { scope: "all" | "filtered"; filter?: ListFirearmsInput }
output: {
  firearmCount: number;
  accessoryCount: number;
  includesRegistration: boolean;  // 005 FR-020's disclosure
  includesAccessories: boolean;   // FR-021's disclosure
}
```

- **Filtered**: the firearms the filter matches, plus everything below
  them at any depth, firearms and accessories alike (FR-020, US5-9).
- **All**: every firearm and every accessory.

`ExportDialog` reads it for the counts and the disclosure, instead of
counting through `list_firearms`.

### `export_collection`

- **Input**: unchanged. The record set is the one `get_export_scope`
  describes.
- **`ExportResult`** gains `accessorySpreadsheetPath: string | null` (the
  second CSV file; `null` for a workbook or when no accessory is exported)
  and `exportedAccessoryCount: number`. `exportedFirearmCount` and
  `exportedPhotoCount` keep their meanings, and the photo count now
  includes accessory photos.

### `import_collection`

```ts
input: { files: { filePath: string; format: "csv" | "xlsx" }[] }   // 1 or 2 files; was { filePath, format }
```

- **Before any row is saved**, the import stops with `VALIDATION_ERROR` and
  a message naming the file (and the sheet) when:
  - two tables of the same kind are given;
  - a file or sheet's header is neither table's;
  - more than two files are given.
- **`ImportResult`**:
  - `RowError`, the warnings and `SnappedValue` gain
    `table: "firearms" | "accessories"`. `row` is numbered within its
    table.
  - `ImportConflict` gains `table` and
    `existingRecord: RecordRef`, replacing `existingFirearmId`. For an
    accessory conflict, `duplicateAllowed` is always `true`, and `make`,
    `model` and `serialNumber` may be `null`; it gains `kindName`.
  - `importedCount`, `updatedCount` and `skippedCount` count both tables.
    `importedAccessoryCount` is added.
- **Mount warnings** (FR-023) arrive in `warnings`, each naming its row and
  reason:
  - "Mounted on {id}: no firearm or accessory has this record ID. Imported
    unmounted."
  - "Mounted on {id}: that record is disposed. Imported unmounted."
  - "Mounted on {id}: it would be mounted on itself through {name}.
    Imported unmounted."
  - "Mounted on {value}: not a record ID. Imported unmounted."

### `resolve_import_conflicts`

The input is unchanged. An `overwrite` or `duplicate` applies the row's
mount by the same rules, and its warnings arrive in `warnings`
(research.md §18).

## Entries (amended content)

- `suggest_entries` and `settle_entry` for `make`, `model`, `caliber` and
  `cartridge` draw on firearms and accessories together (FR-003).
- The `make` input of `suggest_entries` still narrows model suggestions, now
  across both tables.
- No field is added (research.md §16).

## Pending changes (amended content)

`Draft.kind` gains `"accessory"`, with the same modes as `"firearm"`
(`add`, `edit`, `dispose`, `restore`, `coverage`). The `values` are the
form's own, as for every draft. `PendingSummary` names the accessory by
its label (FR-027).
