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
  setting `documentOpening` and the PDF preview hold `pdfPreviewHold`.
- **In memory, per open database**: the open preview and the session's
  confirmation.
- **In memory, per run**: whether PDF preview is available, the tripwire,
  and whether WebKit's sandbox is on (Linux).

## Entity: DocumentAttachment (stored as before; rules amended)

The table `document_attachments` is unchanged (001, owner pair from 006).

| Field | Change |
|---|---|
| `original_filename` | Unchanged as stored. Now also indexed by `document_names_fts` (FR-015). |
| `mime_type` | Now **always the canonical type found by the content check** (research.md §2), never the value the file chooser supplied. One of `application/pdf`, `image/tiff`, `text/plain`, `text/csv`, `application/rtf`, `application/msword`, `application/vnd.openxmlformats-officedocument.wordprocessingml.document`, `application/vnd.ms-excel`, `application/vnd.openxmlformats-officedocument.spreadsheetml.sheet`, `application/vnd.oasis.opendocument.text`, `application/vnd.oasis.opendocument.spreadsheet`. |
| `file_bytes` | Must pass `document_types::classify` at insert (FR-016). Read back by `open_preview` and `open_document` only. A PDF's bytes go to the PDF surface through the `hdpreview` protocol, a TIFF's to the helper by pipe; neither is ever sent to the main web view (research.md §1). |

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
- `previewAvailable: boolean` is `previewKind != null`, except that a PDF
  is `false` while PDF preview is unavailable on this computer (FR-003a;
  the run's `PdfAvailability` below). The list says why (ui contract §1).
- `openable: boolean` is whether the recorded type is a document type.
  It is `false` only for rows stored before this feature with another
  type. The list shows "Open in another app…" disabled for them, with the
  reason (FR-017).

**Rows stored before this feature** with another type, for example a JPEG
attached as a document in a development database:
- they stay listed and can be deleted;
- `previewKind` is `null`, and `previewAvailable` and `openable` are
  `false`;
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
  `VACUUM`. `delete_document` now calls it (research.md §19), so a deleted
  filename leaves no FTS segment behind (`deletion_wipe_test.rs`).
- **Backups**: no backup-due trigger, since an FTS table's writes follow a
  `document_attachments` write, which already marks a backup due.
- **Seeding**: `human_seed_coverage_test.rs` treats `document_names_fts`
  and its shadow tables like the other FTS tables (not user tables).

## Machine-local: `machine.json` (amended)

`VERSION` stays `1`: both fields default, so a file without them reads as
before.

| Field | Type | Default | Notes |
|---|---|---|---|
| `documentOpening` | `"preview"` \| `"external"` | `"preview"` | FR-011. Applies to every database opened on this computer and never travels with a database. Changed to `"external"` only after a "yes" in the native confirmation (FR-012), and back without one. |
| `pdfPreviewHold` | `{ version: string \| null, leftovers: string[] }` \| `null` | `null` | FR-003a (research.md §7, §17). `version` is the HoploDex version during which the macOS watch caught a copy; while it equals the running version, PDF preview is off. At startup a different `version` is cleared to `null`. `leftovers` are the paths of caught `WebKitPDFs-*` folders not yet confirmed gone; each sweep (surface close, startup, shutdown signal) deletes them and drops the ones that are gone. The field returns to `null` when both are empty. Paths only, never a document's name or content. |

## In memory, per open database: `OpenDatabase` (amended)

Never written to disk or to the database; gone with the `OpenDatabase` at
lock, close, switch, sleep, shutdown or quit (FR-012, FR-014).

| Field | Type | Meaning |
|---|---|---|
| `external_open_confirmed` | `bool` | The user answered "Open in another app" in the native confirmation for a document during this session. `needs_consent(setting, confirmed)` reads it (research.md §17). |
| `preview` | `Option<Preview>` | The one open preview, if any. |
| `generation` | `u64` | New. Taken from a per-run counter at each open or unlock, so `open_document` writes its copy only into the session that asked (research.md §18). |

### `Preview` (in memory, new)

| Field | Type | Meaning |
|---|---|---|
| `id` | `u64` | Increases per session. Requests carry it, so a request for a replaced preview gets `PREVIEW_CLOSED`. |
| `document_id` | `i64` | The document shown. |
| `content` | `PreviewContent` | Below. |

`PreviewContent` is one of:

| Kind | Holds | Dropped |
|---|---|---|
| `Pdf` | `token: [u8; 16]` (the URL's one-time path segment); `bytes: Zeroizing<Vec<u8>>`; `surface: Arc<dyn PreviewSurface>` (the child web view, shared with the viewer's next PDF); `served: bool`; `hooked: bool` (Linux: PDF.js's `webviewerloaded` hook reported, research.md §8) | bytes zeroized; the token stops resolving. The surface is closed when the last `Pdf` holding it goes and no successor PDF takes it (the viewer moved to a TIFF or text, closed, or the session ended) |
| `Tiff` | `helper: HelperHandle` (the child process, its pipes, its restart count ≤ 2); `pages: Vec<PageSize>`; `bytes: Zeroizing<Vec<u8>>`, kept only to restart a crashed helper | helper killed and waited for; bytes zeroized |
| `Text` | nothing: the decoded text was returned by `open_preview` | n/a |

`PreviewSurface` is a trait (research.md §20): the app's implementation
wraps the `tauri::Webview` and, on macOS, the watch thread; session tests
use a recorder. Its methods are `navigate(url)`, `set_bounds(rect,
visible)`, `focus()`, and `close()` (also called by `Drop`).

**State transitions**:

```text
                     ┌──────── PDF ─────────▶ Serving ──(served; Linux: hooked)──▶ Shown ─┐
(none) ──open_preview┤                           └──(5 s, no hook / no viewer)──▶ (none, PREVIEW_FAILED / PDF_PREVIEW_UNAVAILABLE)
                     ├──────── TIFF ────────▶ Loading ──Loaded──▶ Open ─┤
                     │                           └─Failed──▶ (none, error returned)
                     └──────── Text ────────▶ Open ─────────────────────┤
                                                                         │
   close_preview / open_preview(other) / OpenDatabase dropped ◀──────────┘ ──▶ (none)

   Shown ──macOS watch catches a copy──▶ (none); hold set; preview:pdf-ended {copyCaught}
   Open (TIFF) ──helper crash on a page──▶ Open (restarted, count+1; >2 ⇒ PREVIEW_FAILED, (none))
```

## In memory, per run (new)

Held in Tauri state, not in the session: they belong to the computer and
the running build, not to a database.

| Item | Type | Meaning |
|---|---|---|
| `PdfAvailability` | `Available` \| `Unavailable { reason }` | Decided at startup (research.md §7): `Held` when `pdfPreviewHold.version` equals the running version; `CheckFailed` when the OS check fails; `NoViewer` when a surface turned its PDF into a download this run; becomes `Held` when the watch catches a copy. Read by `list_documents` (`previewAvailable`) and `open_preview`. |
| `Tripwire` | `Option<TcpListener>` and a counter | Bound on `127.0.0.1:0` at the first PDF preview, held until exit (research.md §6). |
| `WebKitSandbox` (Linux) | `On` \| `Off { reason }` | The startup probe's result (research.md §9); logged once and reported in the surface check. |

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
  previewAvailable: boolean;        // new: false for a PDF while PDF preview is off on this computer
  openable: boolean;                // new: false only for pre-007 rows of another type
}

export type DocumentOpening = "preview" | "external";
```

The preview's own shapes (`PreviewInfo`, `PageSize`, `SurfaceBounds`) and
its events are in contracts/tauri-commands.md.

## Unchanged

- `photos` and the photo types (JPEG, PNG). A drop still routes them to
  Photos. The document picker no longer accepts them (FR-016).
- The opened-documents folder and its clean-up at close, lock, quit and
  startup (001 FR-035, 003 FR-022, FR-036, FR-037); only how the copy is
  written changes (research.md §18).
- Cipher settings, every other table and the spreadsheet format.
  Documents are not exported.
