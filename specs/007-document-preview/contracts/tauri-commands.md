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
- Every new command is registered in `main.rs`'s `generate_handler!`, in
  `build.rs`'s `COMMANDS` and in `capabilities/default.json`; a unit test
  fails unless the three agree (research.md §5).
- **Every command, old and new, is allowed only to the `main` web view.**
  The app ACL manifest makes Tauri refuse a command from any web view
  whose capabilities don't allow it; the PDF surface (`preview`) has none.

**Anchors amended**:
- the commands `add_document`, `add_document_from_path`, `list_documents`,
  `open_document`, `delete_document` (content only), `list_firearms` and
  `list_accessories` (search content only);
- the shape `DocumentSummary`.

**Added**:
- `list_document_types`;
- `open_preview`, `set_preview_bounds`, `focus_preview`,
  `render_preview_page`, `close_preview`;
- `get_document_opening`, `set_document_opening`;
- the events `preview:pdf-ready`, `preview:pdf-ended`, `preview:escape`,
  `preview:focus-chrome`;
- the custom protocol `hdpreview` (served to the PDF surface only).

**New error codes**:
- `DOCUMENT_TYPE_NOT_ALLOWED`, `DOCUMENT_CONTENT_MISMATCH`;
- `PREVIEW_UNSUPPORTED`, `PDF_PREVIEW_UNAVAILABLE`, `PREVIEW_DAMAGED`,
  `PREVIEW_FAILED`, `PREVIEW_PAGE_FAILED`, `PREVIEW_CLOSED`;
- `NO_APP_FOR_DOCUMENT`.

Each is defined in research.md §21.

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
  previewAvailable: boolean;     // false for a PDF while PDF preview is off on this computer (FR-003a)
  openable: boolean;             // false only for rows stored before 007 with a non-document type
};

type DocumentType = {
  label: string;                 // "PDF", "TIFF", "Plain text", "CSV", "RTF", "Word", "Spreadsheet", "OpenDocument text", "OpenDocument spreadsheet"
  extensions: string[];          // lowercase, no dot: ["pdf"], ["tif","tiff"], …
  mimeType: string;              // the canonical type recorded
  previewKind: PreviewKind | null;
};

type PageSize = { width: number; height: number }; // points (1/72 in), at the TIFF's DPI (200 if absent)

type PreviewInfo =
  | { previewId: number; documentId: number; kind: "pdf" }          // the surface shows it; wait for preview:pdf-ready
  | { previewId: number; documentId: number; kind: "tiff"; pages: PageSize[] }
  | { previewId: number; documentId: number; kind: "text"; text: string };

type SurfaceBounds = { x: number; y: number; width: number; height: number }; // logical px, relative to the window's content area

type PdfEndReason =
  | "copyCaught"     // macOS: a copy was written and deleted; PDF preview is off on this computer until HoploDex is updated (FR-003a)
  | "noViewer"       // the web view has no PDF viewer; PDF preview is off for this run
  | "failed";        // the surface couldn't be built, or PDF.js's hook didn't run (Linux)

type DocumentOpening = "preview" | "external";
```

## Amended commands

### `list_document_types` *(new)*

- **Input**: none.
- **Output**: `DocumentType[]`, in the table order of research.md §2.
- **Use**: the document picker's `accept` and the drop router's document
  test read this. The frontend keeps no list of its own; the backend's
  `classify` stays the authority.

### `add_document`

- **Input**: `{ owner: RecordRef, fileBytes: number[], originalFilename: string }`.
  The **`mimeType` argument is removed**: the type is the one the content
  check finds (FR-016).
- **Output**: `DocumentSummary`.
- **Errors**:
  - `DOCUMENT_TYPE_NOT_ALLOWED` (the message names the document types; for
    `.jpg`, `.jpeg` and `.png` it says photos are added under Photos);
  - `DOCUMENT_CONTENT_MISMATCH`;
  - `NOT_FOUND` (owner).

  Nothing is stored on any error.

### `add_document_from_path`

- **Input and output**: unchanged.
- **Errors**: as `add_document`.

### `list_documents`

- **Output**: each `DocumentSummary` now carries `previewKind`,
  `previewAvailable` and `openable`.

### `delete_document`

- **Input and output**: unchanged.
- **Behavior**: it now reclaims through `db::reclaim_deleted_record`, so
  the filename's search-index entries are merged away (FR-015,
  research.md §19). If the deleted document is the one previewed, the
  preview is closed first.

### `open_document` (amended: native consent)

- **Input**: `{ id: number }`. There is **no `confirmed` flag**. The web
  view can't confirm.
- **Output**: `{ opened: boolean }`. `false` means the user cancelled the
  native confirmation; nothing was written and nothing started.
- **Order of work**:
  1. Read the document and the session's generation (`session.read`).
  2. `classify` it. On failure, return `DOCUMENT_TYPE_NOT_ALLOWED` or
     `DOCUMENT_CONTENT_MISMATCH`, before any dialog (FR-017).
  3. If `needs_consent(setting, session.external_open_confirmed)`: hide
     the PDF surface if one is shown, pause idle, show the native
     confirmation for this document (ui contract §4), resume idle. On
     "Cancel", return `{ opened: false }`.
  4. Under the session lock, and only if the generation is unchanged:
     set `external_open_confirmed = true` if the dialog was answered
     "Open in another app", and write or reuse the copy (canonical
     extension; private from creation; Zone mark or quarantine;
     research.md §18). If the generation changed, return
     `DATABASE_CLOSED` with nothing written.
  5. Release the lock and call the opener.
- **Errors**:
  - `DOCUMENT_TYPE_NOT_ALLOWED` and `DOCUMENT_CONTENT_MISMATCH`, as above;
  - `NO_APP_FOR_DOCUMENT` (the copy was securely deleted);
  - `INTERNAL_ERROR` (the copy couldn't be written, or its folder isn't
    a private directory owned by the user);
  - `NOT_FOUND`;
  - the session's codes.

## Preview commands *(new)*

All refuse with `DATABASE_CLOSED` when nothing is open. They need no
`write`, so they work while pending changes wait.

### `open_preview`

- **Input**: `{ documentId: number }`.
- **Output**: `PreviewInfo`.
- **Behavior**:
  1. It replaces any open preview (its TIFF helper is killed; a PDF
     surface is kept for a following PDF, research.md §4).
  2. It classifies the document; a mismatch fails as below.
  3. **PDF**: if `PdfAvailability` isn't `Available`, it fails with
     `PDF_PREVIEW_UNAVAILABLE`. Otherwise it binds the tripwire if not yet
     bound, builds the surface if the viewer has none (hidden, at the last
     bounds sent), gives the document a new token, and navigates the
     surface to `hdpreview://localhost/<token>/document.pdf`. It returns
     at once; `preview:pdf-ready` follows.
  4. **TIFF**: it starts the helper (research.md §11) and loads the bytes;
     `pages` lists every page's size.
  5. **Text or CSV**: it decodes in process (research.md §12) and returns
     `text`, with no helper.
- **Errors**:
  - `PREVIEW_UNSUPPORTED` (RTF, Word, spreadsheet, or a pre-007 row of
    another type);
  - `DOCUMENT_CONTENT_MISMATCH`;
  - `PDF_PREVIEW_UNAVAILABLE`;
  - `PREVIEW_DAMAGED` (TIFF);
  - `PREVIEW_FAILED` (TIFF helper crash, time-out or memory limit during
    load; or the PDF surface couldn't be built);
  - `NOT_FOUND`.

  The message is a plain sentence the viewer can show as is.

### `set_preview_bounds`

- **Input**: `{ previewId: number, bounds: SurfaceBounds, visible: boolean }`.
- **Output**: `null`.
- **Behavior**: PDF only. Moves and sizes the surface to `bounds`, and
  shows or hides it. Before `preview:pdf-ready` it stays hidden whatever
  `visible` says. The frontend sends it on mount, on every resize of the
  page area, and with `visible: false` whenever something is drawn above
  the viewer (ui contract §2).
- **Errors**: `PREVIEW_CLOSED`; `VALIDATION_ERROR` (a negative or
  non-finite value, or a size under 1 px).

### `focus_preview`

- **Input**: `{ previewId: number }`.
- **Output**: `null`.
- **Behavior**: PDF only. Gives the surface the keyboard focus (F6 from
  the viewer's controls, research.md §10).
- **Errors**: `PREVIEW_CLOSED`.

### `render_preview_page`

- **Input**: `{ previewId: number, page: number /* 0-based */, widthPx: number }`.
  `widthPx` is clamped to 4096 px and to 24 megapixels for the page's
  aspect ratio (research.md §14).
- **Output**: binary `tauri::ipc::Response` holding PNG bytes, received as
  an `ArrayBuffer`.
- **Behavior**: TIFF only. The session lock isn't held while the helper
  renders.
- **Errors**:
  - `PREVIEW_PAGE_FAILED` (this page only; the helper is restarted for
    the others, up to twice);
  - `PREVIEW_FAILED` (restarts exhausted; the preview is closed);
  - `PREVIEW_CLOSED`;
  - `VALIDATION_ERROR` (page out of range, `widthPx` < 1, or not a TIFF
    preview).

### `close_preview`

- **Input**: `{ previewId: number }`.
- **Output**: `null`.
- **Behavior**: drops the `Preview`: a PDF's token stops resolving, its
  bytes are zeroized and its surface is closed (on macOS after a last
  sweep, research.md §7); a TIFF's helper is killed. Idempotent: a stale
  id is ignored. Called when the viewer closes, and harmlessly after a
  lock.

## Events *(new)*, Rust → main web view

| Event | Payload | When |
|---|---|---|
| `preview:pdf-ready` | `{ previewId }` | the surface has been served its document (and on Linux, PDF.js's hook has run); the frontend shows it |
| `preview:pdf-ended` | `{ previewId, reason: PdfEndReason }` | the surface was closed by Rust (research.md §4, §7, §8); the frontend shows the matching state (ui contract §3) and, for `copyCaught` and `noViewer`, reloads document lists so PDFs show as not previewable here |
| `preview:escape` | `{ previewId }` | Escape was pressed in the surface; the frontend closes the viewer |
| `preview:focus-chrome` | `{ previewId }` | F6 was pressed in the surface; the main web view has the focus again, and the frontend puts it on the viewer's first control |

Events go to the `main` web view only (`emit_to`).

## The `hdpreview` protocol *(new)*

Registered with `register_asynchronous_uri_scheme_protocol`. It answers
only the PDF surface's requests:

| Request | Answer |
|---|---|
| `GET /<token>/document.pdf`, token of the current PDF preview | `200`, `Content-Type: application/pdf`, `Cache-Control: no-store`, no `Accept-Ranges`, the whole document; then `preview:pdf-ready` (on Linux once the hook has also reported) |
| `GET /hooked/<surface secret>` (Linux) | `204`; marks the current document's PDF.js hook as run (research.md §8) |
| anything else, or a token no longer current | `404`, empty |

It holds the session lock only to copy out the current token's bytes.

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
    confirmation for the setting (ui contract §4). `false` means the user
    cancelled, and the setting is unchanged.
  - For `"preview"`, it changes without asking (FR-012).
  - Setting the current value is a no-op returning `{ changed: false }`.
- **Session**: this dialog doesn't set `external_open_confirmed`. The
  first document opened in another app in the session still asks
  (FR-012, spec US3-3).

## Search (content only)

### `list_firearms`, `list_accessories`

The search term also matches the original filenames of the record's own
documents (FR-015):
- through `document_names_fts` for three or more characters;
- through `LIKE` for one or two.

Nothing else changes, including the result shape. A record found by a
document name carries no indication of which document matched.
