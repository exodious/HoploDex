# Contract: Tauri IPC Commands (delta)

This is a **delta** against
[the 001 IPC contract](../../001-firearms-inventory/contracts/tauri-commands.md),
as amended by 002 to
[006](../../006-accessory-links/contracts/tauri-commands.md). It lists only
what changes or is added. The `CommandError` shape is unchanged.

**Common rules**:
- Every collection command goes through `session.read(...)` or
  `session.write(...)` as before. `get_document_opening` and
  `set_document_opening` work on `MachineSettings` and need no open
  database.
- Every new command is registered in `main.rs`'s `generate_handler!`.

**Anchors amended**:
- the commands `add_document`, `add_document_from_path`, `list_documents`,
  `open_document`, `delete_document` (content only), `list_firearms` and
  `list_accessories` (search content only);
- the shape `DocumentSummary`.

**Added**:
- `list_document_types`;
- `open_preview`, `render_preview_page`, `get_preview_page_text`,
  `close_preview`;
- `get_document_opening`, `set_document_opening`.

**New error codes**:
- `DOCUMENT_TYPE_NOT_ALLOWED`, `DOCUMENT_CONTENT_MISMATCH`;
- `PREVIEW_UNSUPPORTED`, `PREVIEW_DAMAGED`, `PREVIEW_PASSWORD_PROTECTED`,
  `PREVIEW_FAILED`, `PREVIEW_PAGE_FAILED`, `PREVIEW_CLOSED`;
- `NO_APP_FOR_DOCUMENT`.

Each is defined in research.md §17.

## Shapes

```ts
type PreviewKind = "pdf" | "tiff" | "text";

type DocumentSummary = {
  id: number;
  owner: RecordRef;
  originalFilename: string;
  mimeType: string;              // always the canonical type found by the content check (FR-016)
  createdAt: string;
  previewKind: PreviewKind | null; // from the recorded type; content is confirmed at open_preview
  openable: boolean;             // false only for rows stored before 007 with a non-document type
};

type DocumentType = {
  label: string;                 // "PDF", "TIFF", "Plain text", "CSV", "RTF", "Word", "Spreadsheet", "OpenDocument text", "OpenDocument spreadsheet"
  extensions: string[];          // lowercase, no dot: ["pdf"], ["tif","tiff"], …
  mimeType: string;              // the canonical type recorded
  previewKind: PreviewKind | null;
};

type PageSize = { width: number; height: number }; // points (1/72 in); a TIFF page converted at its DPI (200 if absent)

type PreviewInfo =
  | { previewId: number; documentId: number; kind: "pdf" | "tiff"; pages: PageSize[] }
  | { previewId: number; documentId: number; kind: "text"; text: string };

type DocumentOpening = "preview" | "external";
```

## Amended commands

### `list_document_types` *(new)*

- **Input**: none.
- **Output**: `DocumentType[]`, in the table order of research.md §2.
- **Use**: the document picker's `accept` and the drop router's
  document test read this. The frontend keeps no list of its own. The
  backend's `classify` stays the authority.

### `add_document`

- **Input**: `{ owner: RecordRef, fileBytes: number[], originalFilename: string }`.
  The **`mimeType` argument is removed**: the type is the one the content
  check finds (FR-016).
- **Output**: `DocumentSummary`.
- **Errors**:
  - `DOCUMENT_TYPE_NOT_ALLOWED` (message names the document types; for
    `.jpg`, `.jpeg` and `.png` it says photos are added under Photos);
  - `DOCUMENT_CONTENT_MISMATCH`;
  - `NOT_FOUND` (owner).

  Nothing is stored on any error.

### `add_document_from_path`

- **Input and output**: unchanged.
- **Errors**: as `add_document`.

### `list_documents`

- **Output**: each `DocumentSummary` now carries `previewKind` and
  `openable`.

### `delete_document`

- **Input and output**: unchanged.
- **Behavior**: it now reclaims through `db::reclaim_deleted_record`, so
  the filename's search-index entries are merged away (FR-015,
  research.md §15).

### `open_document` (amended: native consent)

- **Input**: `{ id: number }`. There is **no `confirmed` flag**. The web
  view cannot confirm.
- **Output**: `{ opened: boolean }`. `false` means the user cancelled the
  native confirmation, and then nothing was written and nothing started.
- **Order of work**:
  1. Read the document (`session.read`).
  2. `classify` it. On failure, return `DOCUMENT_TYPE_NOT_ALLOWED` or
     `DOCUMENT_CONTENT_MISMATCH`, before any dialog (FR-017).
  3. If `needs_consent(setting, session.external_open_confirmed)`:
     pause idle, show the native confirmation for this document
     (contracts/ui-document-preview.md §4), resume idle. On "Cancel",
     return `{ opened: false }`.
  4. Check that the same database session is still open (generation).
     Otherwise return `DATABASE_CLOSED` with nothing written.
  5. Set `external_open_confirmed = true` if the dialog was answered
     "Open in another app".
  6. Write or reuse the copy: canonical extension, `0700`/`0600`, Zone
     mark or quarantine (research.md §14).
  7. Call the opener.
- **Errors**:
  - `DOCUMENT_TYPE_NOT_ALLOWED` and `DOCUMENT_CONTENT_MISMATCH`, as
    above;
  - `NO_APP_FOR_DOCUMENT` (the copy was securely deleted);
  - `INTERNAL_ERROR` (copy couldn't be written);
  - `NOT_FOUND`;
  - the session's codes.

## Preview commands *(new)*

All four refuse with `DATABASE_CLOSED` when nothing is open. They need no
`write`, so they work while pending changes wait.

### `open_preview`

- **Input**: `{ documentId: number }`.
- **Output**: `PreviewInfo`.
- **Behavior**:
  1. It closes any open preview.
  2. It classifies the document; a mismatch fails as below.
  3. **PDF or TIFF**: it starts a render helper (research.md §4) and loads
     the bytes; `pages` lists every page's size.
  4. **Text or CSV**: it decodes in process (research.md §8) and returns
     `text`, with no helper.
- **Errors**:
  - `PREVIEW_UNSUPPORTED` (RTF, Word, spreadsheet, or a pre-007 row of
    another type);
  - `DOCUMENT_CONTENT_MISMATCH`;
  - `PREVIEW_DAMAGED`;
  - `PREVIEW_PASSWORD_PROTECTED`;
  - `PREVIEW_FAILED` (helper crash, time-out or memory limit during load);
  - `NOT_FOUND`.

  The message is a plain sentence the viewer can show as is.

### `render_preview_page`

- **Input**: `{ previewId: number, page: number /* 0-based */, widthPx: number }`.
  `widthPx` is clamped to 4096 px and to 24 megapixels for the page's
  aspect ratio (research.md §10).
- **Output**: binary `tauri::ipc::Response` holding PNG bytes, received as
  an `ArrayBuffer`.
- **Behavior**: the session lock is not held while the helper renders.
- **Errors**:
  - `PREVIEW_PAGE_FAILED` (this page only; the helper is restarted for
    the others, up to twice);
  - `PREVIEW_FAILED` (restarts exhausted; the preview is closed);
  - `PREVIEW_CLOSED`;
  - `VALIDATION_ERROR` (page out of range, `widthPx` < 1).

### `get_preview_page_text`

- **Input**: `{ previewId: number, page: number }`.
- **Output**: `string`. It is the PDF page's text in reading order, with
  control characters other than line breaks removed. For a TIFF page, or a
  PDF page without text, it is empty.
- **Errors**: `PREVIEW_PAGE_FAILED`, `PREVIEW_CLOSED`, `VALIDATION_ERROR`.

### `close_preview`

- **Input**: `{ previewId: number }`.
- **Output**: `null`.
- **Behavior**: it kills the helper and drops the held bytes. It is
  idempotent: a stale id is ignored. It is called on close, on moving to
  another document (`open_preview` also does it), and needlessly but
  harmlessly after a lock.

## Setting commands *(new)*

### `get_document_opening`

- **Input**: none.
- **Output**: `DocumentOpening`, from `machine.json` (default
  `"preview"`).

### `set_document_opening`

- **Input**: `{ value: DocumentOpening }`.
- **Output**: `{ changed: boolean }`.
- **Behavior**:
  - For `"external"` while the setting is `"preview"`, it shows the native
    confirmation for the setting (contracts/ui-document-preview.md §4).
    `false` means the user cancelled, and the setting is unchanged.
  - For `"preview"`, it changes without asking (FR-012).
  - Setting the current value is a no-op returning `{ changed: false }`.
- **Session**: the session flag `external_open_confirmed` is not set by
  this dialog. The first document opened in another app in the session
  still asks (FR-012, spec US3-3).

## Search (content only)

### `list_firearms`, `list_accessories`

The search term also matches the original filenames of the record's own
documents (FR-015):
- through `document_names_fts` for three or more characters;
- through `LIKE` for one or two.

Nothing else changes, including the result shape. A record found by a
document name carries no indication of which document matched.
