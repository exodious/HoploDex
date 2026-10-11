# Data Model: Session Isolation, Safe Import, Browser Controls and Pending Changes from Another Version

**Feature**: [spec.md](./spec.md) · **Research**: [research.md](./research.md)

This is a delta against the data model as features 001 to 007 left it.
There is one schema change (a column on `app_state`), new validation rules
for free-text fields, and in-memory shapes for the session, the resume
lifecycle and the import helper.

## Stored: the database

### `app_state` (amended)

| Column | Type | Rule |
|---|---|---|
| `last_saved_version` | `TEXT`, nullable | The HoploDex version (`CARGO_PKG_VERSION`, e.g. `1.2.0`) that last saved the database (FR-015, research.md §16). NULL when none is recorded. |

- **Set** at creation; inside `write_pending`'s transaction whenever a draft
  is written (a lock, or the immediate close at sleep or shutdown); and after
  any `write`, `write_open` or `write_housekeeping` that succeeded and
  changed rows.
- **Never set** by opening, the open marker, closing, a backup, a restore's
  own bookkeeping, a passphrase change, or a write that changed nothing.
- Copied into backups like the rest of `app_state`. A restored database
  names the version that last saved it before the backup was made.
- `0001_initial.sql` is edited in place. Databases made before this feature
  must be created again (CLAUDE.md "Schema changes"). The developer's own
  databases are not touched.
- **Not change-tracked**: writing it doesn't make a backup due. It is
  `app_state`, which has no backup-due triggers, so
  `backup_due_tracking_test.rs` is unaffected.

### `pending_changes` (unchanged in shape)

What changes is its lifecycle (below): resuming no longer deletes the row.
Only the form opening with it (`opened`) or a discard does.

## Validation: free-text field maximums (research.md §10)

Counted in characters (Unicode scalar values) after trimming, checked on
save and on import (a row error), with the message `"<Label> can be at most
<n> characters."`.

| Record | Field (IPC name) | Maximum |
|---|---|---|
| Firearm | `make`, `model`, `cartridge`, `caliber`, `registrationForm`, `registeredTo` | 100 (unchanged) |
| Firearm | `nickname`, `serialNumber`, `finish`, `acquisitionSource`, `dispositionRecipient`, `countryOfManufacture`, `importerName`, `originalMake`, `originalModel`, `originalSerialNumber` | 200 |
| Firearm | `notes`, `accessories` | 4,000 |
| Accessory | `make`, `model`, `cartridge`, `caliber` | 100 (unchanged) |
| Accessory | `serialNumber`, `acquisitionSource`, `dispositionRecipient` | 200 |
| Accessory | `notes` | 4,000 |
| Disposition (both dispose dialogs) | `recipient` | 200 |
| Insurance policy | `name`, `policyNumber`, `insuranceCompany`, `companyContact`, `agentName`, `agentContact` | 200 |
| Insurance policy | `notes` | 4,000 |

Constants in `models/rules.rs`: `MAX_SHORT_TEXT_CHARS = 200`,
`MAX_LONG_TEXT_CHARS = 4_000`, beside `MAX_ENTRY_CHARS = 100`
(`services/entry_text.rs`). The frontend mirrors them in `src/lib/textLimits.ts`.

## In memory: the session (backend)

### `SessionId`

`SessionId(u64)`: the former `OpenDatabase::generation`, from a counter
that only goes up within a run (research.md §1). `0` is never issued and
means "none".

### `OpenDatabase` (amended)

| Field | Change |
|---|---|
| `generation: u64` | Renamed `session_id: SessionId`. |
| `pending_unresolved: bool` | Replaced by `pending: PendingState` (below). |

### `SessionInner` (amended)

| Field | Rule |
|---|---|
| `current: AtomicU64` | The open database's `SessionId`, or 0. Set by `install`, cleared by `take`/`forget_open`. Read without the session's mutex, by work running outside it (the import's reading, research.md §11). |

### `PendingState` and its transitions (FR-003, FR-017; research.md §6)

```text
          open finds a row                    resolve(resume)
 (none) ───────────────────▶ Unresolved ─────────────────────▶ Resuming
    ▲                          │   ▲                              │
    │      resolve(discard)    │   │      resolve(notOpened)      │
    ├──────────────────────────┘   └──────────────────────────────┤
    │                                                             │
    │                     resolve(opened)                         │
    └─────────────────────────────────────────────────────────────┘

 Any session end (lock, close, switch, restore, take-over, sleep, shutdown)
 in Unresolved or Resuming leaves the row; the next open starts in Unresolved.
 A lock writes a staged draft over the row only if one is staged.
```

| State | Row | Collection commands |
|---|---|---|
| `None` | none | served |
| `Unresolved` | kept | `PENDING_CHANGES_UNRESOLVED` |
| `Resuming` | kept | served (the record loads and its form opens) |

`close_database` is served in every state. It is session housekeeping,
which `close_normal` never refused.

### `ScopedSession` (Tauri command argument)

`{ session: Session, id: SessionId }`, built from the `Session` state and the
`HoploDex-Session` header. Its `read`, `write`, `write_open`,
`write_housekeeping`, `inspect`, `inspect_mut` and `hold` delegate to
`Session`'s with `id`. It also has `is_current()`, a lock-free comparison
with `SessionInner::current`. A missing or unparsable header refuses with
`DATABASE_CLOSED`.

## In memory: the session (frontend)

### `SessionScope` (`src/features/session/sessionScope.ts`)

| Member | Rule |
|---|---|
| `id: number` | `DatabaseStatus.sessionId`. |
| `ended: boolean`, `end()` | `end()` is synchronous and idempotent. It clears `thumbnails`, drops `resumed`, and resets the staging state (research.md §4). |
| `invoke<T>(command, args)` | Sends `HoploDex-Session: id`. Once ended, it rejects with `CommandFailure("DATABASE_CLOSED")` without sending, and a response arriving after `end()` is rejected the same way. |
| `thumbnails: Map<number, string>` | Photo id → data URL, for this session only. |
| `resumed: { draft: Draft; state: "opening" } \| null` | The resumed draft until its form opens (research.md §6). `resumeOpened()` and `resumeFailed(reason)` report the outcome. |

Lifetime: from `opened(status)` to the first of `session:closing` or
`session:closed` with its `sessionId`, "Lock now", the next `opened()`, or
`toChooser`.

## In memory: import reading (research.md §8, §9, §11)

### Import limits (`services/import_reader/limits.rs`)

| Name | Value | Unit |
|---|---|---|
| `MAX_FILE_BYTES` | 268,435,456 (256 MiB) | bytes, per file |
| `MAX_UNPACKED_BYTES` | 1,073,741,824 (1 GiB) | bytes, all parts of one workbook, measured by inflating |
| `MAX_ZIP_ENTRIES` | 1,000 | entries per workbook |
| `MAX_SHEETS` | 16 | sheets per workbook |
| `MAX_ROWS` | 100,000 | data rows per table |
| `MAX_COLUMNS` | 256 | cells per row |
| `MAX_CELL_CHARS` | 32,767 | characters per cell |
| `MAX_TOTAL_TEXT_BYTES` | 268,435,456 (256 MiB) | UTF-8 bytes of all cells read, per import |
| `MAX_SHARED_STRINGS` | 2,000,000 | declared (`count`, `uniqueCount`) or present |
| `HELPER_MEMORY_BYTES` | 536,870,912 (512 MiB) | the import helper's data limit (Linux, Windows) |
| `HELPER_IDLE_TIMEOUT` | 120 s | with no frame from the helper |

`ImportLimit` (serialized camelCase, for `IMPORT_LIMIT_EXCEEDED.details.limit`):
`fileSize`, `unpackedSize`, `zipEntries`, `sheets`, `rows`, `columns`,
`cellText`, `totalText`, `sharedStrings`.

### Import helper frames

These use the framing shared with the TIFF helper (`services/helper/frames.rs`:
a type byte, a 32-bit big-endian length, the payload).
- A `Rows` frame holds up to 500 rows and ends early once it passes 2 MiB.
  A single row larger than that goes alone.
- Either side refuses a frame over 64 MiB (`MAX_IMPORT_FRAME`), which is
  more than the largest possible row (256 cells × 32,767 characters × 4
  bytes).

| Direction | Frame | Payload |
|---|---|---|
| parent → helper | `Begin` | `format: csv \| xlsx`, `file_size: u64`, `file_name: String` (for messages only) |
| parent → helper | `Chunk` | ≤ 1 MiB of the file |
| parent → helper | `End` | none |
| helper → parent | `Progress` | `done: u64`, `total: u64` (bytes, research.md §11) |
| helper → parent | `Sheet` | `name: String` (an XLSX sheet. A CSV is one unnamed sheet.) |
| helper → parent | `Rows` | `Vec<Vec<String>>`, the header row first in a sheet's first batch |
| helper → parent | `Finished` | none |
| helper → parent | `Refused` | `limit: ImportLimit`, `sheet: Option<String>` |
| helper → parent | `Unreadable` | none (the reason is never shown; nothing from the file is logged) |

The parent sends one file per helper. An import of two files starts the
helper twice, one after the other, and refuses the whole import if either
file is refused (FR-006).

### Reading progress

`import_collection:progress` gains `phase: "reading" | "importing"`.
- In `reading`, `processed` and `total` are bytes across the files being
  read.
- In `importing`, they are rows, as before.
