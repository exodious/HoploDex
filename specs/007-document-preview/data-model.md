# Data Model: Document Preview and Consent Before Opening Externally

This is a **delta** against
[001's data model](../../001-firearms-inventory/data-model.md), as amended
by 002 to 006. Only what changes is listed.

**Schema changes**: only one, in `0002_fts5.sql`, made in place because
the app is unreleased (CLAUDE.md). It is the new index
`document_names_fts` and its three triggers. No table gains or loses a
column, and an existing development database must be recreated.

**What else changes**:
- **On this computer** (`machine.json`), outside every database: the
  setting `documentOpening`.
- **In memory, per open database**: the open preview and the session's
  confirmation.

## Entity: DocumentAttachment (stored as before; rules amended)

The table `document_attachments` is unchanged (001, owner pair from 006).

| Field | Change |
|---|---|
| `original_filename` | Unchanged as stored. Now also indexed by `document_names_fts` (FR-015). |
| `mime_type` | Now **always the canonical type found by the content check** (research.md §2), never the value the file chooser supplied. One of `application/pdf`, `image/tiff`, `text/plain`, `text/csv`, `application/rtf`, `application/msword`, `application/vnd.openxmlformats-officedocument.wordprocessingml.document`, `application/vnd.ms-excel`, `application/vnd.openxmlformats-officedocument.spreadsheetml.sheet`, `application/vnd.oasis.opendocument.text`, `application/vnd.oasis.opendocument.spreadsheet`. |
| `file_bytes` | Must pass `document_types::classify` at insert (FR-016). They are read back by `open_preview` and `open_document` only, and never sent to the web view as bytes for PDF or TIFF (research.md §1). |

**Validation (FR-016)**, in `services::document_types::classify(filename,
bytes)`:

1. The extension, compared case-insensitively, must be one of `pdf`, `tif`,
   `tiff`, `txt`, `csv`, `rtf`, `doc`, `docx`, `xls`, `xlsx`, `odt`, `ods`.
   - `jpg`, `jpeg` and `png` → `DOCUMENT_TYPE_NOT_ALLOWED` with the
     photo message.
   - Anything else → `DOCUMENT_TYPE_NOT_ALLOWED`.
2. The content must carry that type's signature (research.md §2's table).
   Otherwise → `DOCUMENT_CONTENT_MISMATCH`. This includes:
   - macro-enabled OOXML, OLE or ODF content;
   - text or CSV that begins with markup.

There is no database `CHECK` for the type. The rule needs the content and
lives in one Rust function, which both attach paths, `open_preview` and
`open_document` call.

**Derived on read** (not stored):
- `previewKind: "pdf" | "tiff" | "text" | null` comes from the recorded
  type alone: PDF → `pdf`; TIFF → `tiff`; text and CSV → `text`; others →
  `null`. The content is confirmed again at `open_preview`, where a
  mismatch is reported (FR-001, FR-006).
- `openable: boolean` is whether the recorded type is a document type.
  It is `false` only for rows stored before this feature with another
  type. The list shows "Open in another app…" disabled for them, with the
  reason (FR-017).

**Rows stored before this feature** with another type, for example a JPEG
attached as a document in a development database:
- they stay listed and can be deleted;
- `previewKind` is `null` and `openable` is `false`;
- `open_document` refuses them with `DOCUMENT_TYPE_NOT_ALLOWED` before any
  confirmation;
- no migration rewrites them.

## Virtual table: `document_names_fts` (new, in `0002_fts5.sql`)

```sql
CREATE VIRTUAL TABLE document_names_fts USING fts5(
    original_filename,
    content = 'document_attachments',
    content_rowid = 'id',
    tokenize = 'trigram remove_diacritics 1'
);
-- AFTER INSERT / AFTER DELETE / AFTER UPDATE OF original_filename triggers on
-- document_attachments, in the same external-content pattern as firearms_fts.
```

- **Owner**: none. A match is joined back to `document_attachments` for
  its `firearm_id` or `accessory_id`. `list_firearms` joins only
  `firearm_id` and `list_accessories` only `accessory_id`, so a record is
  never found through the documents of a record mounted on it (FR-015).
- **Cascades**: deleting a firearm or an accessory deletes its documents
  through the foreign key. SQLite fires `document_attachments`' delete
  trigger for each, which removes the index entries.
- **Reclaiming**: `db::reclaim_deleted_record` merges `firearms_fts`,
  `accessories_fts` **and** `document_names_fts` (`'optimize'`) before
  `VACUUM`. `delete_document` now calls it (research.md §15), so a deleted
  filename leaves no FTS segment behind (`deletion_wipe_test.rs`).
- **Backups**: there is no backup-due trigger, since an FTS table's writes
  follow a `document_attachments` write, which already marks a backup due.
- **Seeding**: `human_seed_coverage_test.rs` treats `document_names_fts`
  and its shadow tables like the other FTS tables (not user tables).

## Machine-local: `machine.json` (amended)

| Field | Type | Default | Notes |
|---|---|---|---|
| `documentOpening` | `"preview"` \| `"external"` | `"preview"` (`#[serde(default)]`) | FR-011. It applies to every database opened on this computer and never travels with a database. `VERSION` stays `1`, since a file without the field reads as the default. Changed to `"external"` only after a "yes" in the native confirmation (FR-012). Changed back without one. |

## In memory: `OpenDatabase` (amended)

These are never written to disk or to the database, and are gone with the
`OpenDatabase` at lock, close, switch, sleep, shutdown or quit (FR-012,
FR-014).

| Field | Type | Meaning |
|---|---|---|
| `external_open_confirmed` | `bool` | The user answered "Open in another app" in the native confirmation for a document during this session. `needs_consent(setting, confirmed)` reads it (research.md §13). |
| `preview` | `Option<Preview>` | The one open preview, if any. |

### `Preview` (in memory, new)

| Field | Type | Meaning |
|---|---|---|
| `id` | `u64` | Increases per session. Requests carry it, so a request for a replaced preview gets `PREVIEW_CLOSED`. |
| `document_id` | `i64` | The document shown. |
| `kind` | `pdf \| tiff \| text` | |
| `helper` | `Option<HelperHandle>` | For `pdf` and `tiff`: the child process, its pipes, its restart count (≤ 2). `Drop` kills it and waits. |
| `pages` | `Vec<PageSize>` | Width and height in points (PDF) or at the TIFF's DPI. |
| `bytes` | `Zeroizing<Vec<u8>>` | Kept only to restart a crashed helper. Zeroized on drop. |

**State transitions**:

```text
(none) ──open_preview──▶ Loading ──Loaded──▶ Open ──close_preview / open_preview(other) / OpenDatabase dropped──▶ (none)
                            │                  │
                            └─Failed──▶ (none, error returned)
                                               └─helper crash on a page──▶ Open (restarted, count+1; >2 ⇒ PREVIEW_FAILED, (none))
```

## Frontend types (amended)

`src/features/media/types.ts`:

```ts
export type PreviewKind = "pdf" | "tiff" | "text";

export interface DocumentSummary {
  id: number;
  owner: RecordRef;
  originalFilename: string;
  mimeType: string;
  createdAt: string;
  previewKind: PreviewKind | null;  // new
  openable: boolean;                // new: false only for pre-007 rows of another type
}

export type DocumentOpening = "preview" | "external";
```

The preview's own shapes (`PreviewInfo`, `PageSize`) are in
contracts/tauri-commands.md.

## Unchanged

- `photos` and the photo types (JPEG, PNG). A drop still routes them to
  Photos. The document picker no longer accepts them (FR-016).
- The opened-documents folder, its clean-up at close, lock, quit and
  startup (001 FR-035, 003 FR-022, FR-036, FR-037).
- Cipher settings, every other table and the spreadsheet format.
  Documents are not exported.
