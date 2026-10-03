# Contract: Document Preview UI

The user-facing contract for 007. It covers:
- the document list's actions and the viewer;
- the native confirmation's text and the setting;
- attach refusals;
- keyboard and accessibility, and the screenshot walk.

Components are the shared ones in `src/components/` (`Dialog`, `Button`,
`Menu`, `SegmentedControl`, `useToast`). No new primitive is added.

**Wording**:
- "Preview", never "View" or "Open" alone.
- "Open in another app…", with the ellipsis, because it asks first.
- "document", never "file" or "attachment", in the viewer's and the
  list's copy.
- "this computer" for the setting's scope.

## 1. The document list (`DocumentList.tsx`)

Each row, left to right:

1. **The name**, a button.
   - **Setting "Preview in HoploDex"**: activating it opens the viewer
     (§2). For a non-previewable document, the viewer opens on its "can't
     be previewed here" state (§3).
   - **Setting "Open in another app"**: it calls `open_document`.
   - Its `title` says what will happen: "Preview {name}" or "Open {name}
     in another app".
2. **Meta line**: `{kind} · added {date}`. When `previewKind` is `null`,
   it adds "· opens in another app" (FR-013: "the list MUST show which
   documents can be previewed"). When `openable` is `false`, it says
   "· can't be opened: not a document type" instead.
3. **"Preview"**: a ghost button with the `eye` icon, shown only when
   `previewKind` is set, whatever the setting (FR-013).
4. **"Open in another app…"**: a ghost button with the `open` icon,
   disabled with a `title` giving the reason when `openable` is `false`.
5. **Delete**: unchanged, an icon button with the same `ConfirmDialog`.

**Kind labels** (replacing `kindLabel`): from the `DocumentType.label` of
the recorded type, so "PDF", "TIFF", "Plain text", "CSV", "RTF", "Word",
"Spreadsheet", "OpenDocument text", "OpenDocument spreadsheet". A pre-007
row of another type shows its extension in capitals.

**Busy state**: while `open_document` waits on the native dialog, its
button shows `pending`. On `{ opened: false }`, nothing is said: the user
cancelled.

**Results of `open_document`**:
- **Success**: a toast, "Opened {name} in another app."
- **`NO_APP_FOR_DOCUMENT`**: an error toast, "This computer has no app
  that opens {kind} documents. HoploDex deleted the copy it made."
- **`DOCUMENT_TYPE_NOT_ALLOWED` / `DOCUMENT_CONTENT_MISMATCH`**: an error
  toast with the backend message.

## 2. The viewer (`DocumentPreview.tsx`, new)

It is a `Dialog` of size `xl`, a new size added to the shared `Dialog`
for both viewers: 90 vw × 90 vh, the most a page needs. It is laid out
like `PhotoViewer` (constitution III).

**Header**:
- **Title**: the document's name.
- **Description**: `{kind} · {n} pages · added {date}` for PDF and
  multi-page TIFF; `{kind} · added {date}` otherwise. It also gives the
  position: `Document {i} of {count}`.

**Toolbar** (above the pages; PDF and multi-page TIFF):
- first, previous, `Page {n} of {count}`, next, last;
- separator;
- zoom out, `{percent}%`, zoom in;
- fit width, fit page.

The percent is a `Menu` offering 50–400% and the two fits. For a
single-page TIFF the toolbar shows zoom out, `{percent}%`, zoom in, fit,
and actual size. For text there is no toolbar.

**Body**:
- **PDF / TIFF**: a scroll area of pages, each a `<figure>` (research.md
  §10–§11), with a 16 px gap and the page's shadow from `tokens.css`.
- **Text**: a `<pre class="hd-preview__text">` with `white-space:
  pre-wrap`, the mono font token, and the content as a text child.

**Footer**, matching `PhotoViewer`:
- previous and next document (icon buttons "Previous document" / "Next
  document"), moving only through this record's own documents (FR-007);
- "Delete document" (ghost), with the same `ConfirmDialog` as the list.
  After deleting, the viewer moves to the next document, or the previous,
  or closes if none is left;
- "Open in another app…" (secondary).

**Closing**: by Escape, the close control, or the record being navigated
away from. Focus returns to the control that opened the viewer.

## 3. States in the viewer body

| State | Shown |
|---|---|
| Loading, before page 1 | page 1's slot, sized to an A4/letter placeholder, with a spinner and "Preparing page 1…" (`aria-live="polite"`). The toolbar is disabled. |
| Page not yet rendered | a blank page-sized slot (`aria-busy="true"`) |
| `PREVIEW_UNSUPPORTED` | an icon and "{name} can't be previewed here. {Kind} documents open in another app.", plus a primary "Open in another app…" |
| `DOCUMENT_CONTENT_MISMATCH` | "{name} can't be previewed: its content isn't a {kind} document." No "Open in another app…" (FR-017). |
| `PREVIEW_DAMAGED` | "{name} can't be previewed: the document is damaged or incomplete.", plus "Open in another app…" |
| `PREVIEW_PASSWORD_PROTECTED` | "{name} is protected by its own password and can't be previewed here. HoploDex doesn't ask for or keep document passwords.", plus "Open in another app…" |
| `PREVIEW_FAILED` | "{name} couldn't be previewed.", plus "Open in another app…" |
| `PREVIEW_PAGE_FAILED` | in that page's slot: "Page {n} can't be shown." The other pages are unaffected. |

Every message is a `role="status"` region. The previous and next document
buttons stay usable in every state.

## 4. The native confirmation (shown by Rust)

It is a system message dialog, warning kind, with two buttons: **"Open in
another app"** (default) and **"Cancel"** (Escape). It is parented to the
main window and modal.

**For a document** (`open_document`):

> **Title**: Open “{original filename}” in another app?
>
> **Body**:
> HoploDex will put an unprotected copy of this document on this computer and open it in the app this computer uses for {kind} documents.
>
> • Other apps, and anyone who can use this computer account, can read the copy while it is there.
> • The other app may keep its own copies or records of the document, such as recent-files lists, autosaves, caches or cloud sync. HoploDex can't find or delete those.
> • The other app may connect to the internet if the document asks it to, for example to load a picture or follow a link, which can show that the document was opened.
> • HoploDex deletes its copy when this database is closed or locked or HoploDex quits, so the other app may lose the document then. Changes saved to the copy are not kept in your collection.

When the setting is "Open in another app" and this is the session's first
external open, one more line follows: "You won't be asked again until this
database is closed or locked."

**For the setting** (`set_document_opening("external")`):

> **Title**: Open documents in another app?
>
> **Body**: the same four bullets, introduced by: "Each document you open will be copied, unprotected, to this computer and opened in the app this computer uses for its type. This applies to every database on this computer." Followed by: "You'll be asked once each time a database is opened or unlocked."

The text is built in Rust (`services::consent`). The filename is the
stored one, with control characters replaced, and cut to 120 characters
with an ellipsis.

## 5. The setting (`DatabaseSettingsDialog.tsx`)

There is a new fieldset, **"Documents"**, placed after "Locking" and
before "This computer". It holds a `SegmentedControl` labelled "Open
documents", with "Preview in HoploDex" and "Open in another app", and the
hint "Applies to every database on this computer."

Choosing "Open in another app" calls `set_document_opening`:
- **while the native dialog is up**: the control shows the old value as
  pending;
- **on `{ changed: false }`**: it stays at "Preview in HoploDex";
- **choosing "Preview in HoploDex"**: it changes at once.

The setting is saved immediately, not by the dialog's Save, because it is
this computer's and not the database's. The hint says so: "Saved for this
computer as soon as you choose."

## 6. Attaching: refusals (FR-016)

- **The picker**: the document file input's `accept` is built from
  `list_document_types` (`.pdf,.tif,.tiff,.txt,.csv,.rtf,.doc,.docx,.xls,.xlsx,.odt,.ods`).
- **Drops**:
  - JPEG and PNG still go to Photos (`isPhotoPath`).
  - `isDocumentPath` is now "the extension is in `list_document_types`",
    no longer "not a photo".
  - Any other dropped file is refused before any command, with a toast:
    "{name} wasn't attached. Documents can be PDF, TIFF, text, CSV, RTF,
    Word, spreadsheet or OpenDocument files."
- **The empty-state drop zone** reads "Drop receipts, bills of sale,
  registration forms or service records here (PDF, TIFF, text, Word or
  spreadsheet), or choose files".
- **Backend refusals**: `DOCUMENT_TYPE_NOT_ALLOWED` and
  `DOCUMENT_CONTENT_MISMATCH` arrive through the existing batch handler,
  "carrying on past one that fails", with the backend's message. A photo
  picked as a document gets "{name} is a photo. Add it under Photos
  instead."

## 7. Keyboard

The two scopes match `PhotoViewer`.

**On the viewer, focus outside the page area**:
- **← / →**: previous and next document.

**In the page area** (focusable, `tabIndex=0`, labelled "{name}, pages"):
- **↑ / ↓ / Space / Shift+Space**: scroll.
- **PageUp / PageDown**: previous and next page.
- **Home / End**: first and last page.
- **+ / −**: zoom in and out.
- **0**: fit width; for a single-page TIFF, fit.
- **1**: actual size (single-page TIFF).
- **← / →**: scroll sideways when the page is wider than the view.

**Everywhere in the viewer**:
- **Escape**: closes the viewer.
- **Tab**: reaches every toolbar and footer control in visual order.

## 8. Accessibility

- **The dialog**: `aria-labelledby` the title and `aria-describedby` the
  description.
- **Pages**: as research.md §11.
  - A PDF page with text: `<img alt="">` plus hidden text in a
    `<div role="document" aria-label="Page {n} of {count}">` with
    `user-select: none`.
  - A page without text: `alt="{name}, page {n} of {count}"`.
  - A single-page TIFF: `alt="{name}"`.
- **The page indicator**: `aria-live="polite"`, announcing on page
  changes made by the controls, not on scroll.
- **Zoom percent**: in the zoom buttons' `aria-describedby`.
- **Copying**: `copy` and `contextmenu` are prevented inside the viewer.
- **Contrast and focus rings**: from the existing tokens, which pass WCAG
  2.1 AA.

## 9. Lock, close and the idle clock

- **Lock**: the viewer is inside the collection providers, so a lock,
  close, switch or sleep unmounts it with the rest (SC-006). Its cleanup
  revokes `blob:` URLs and drops the text.
- **Idle**: scrolling, paging, zooming and keys in the viewer count as
  activity through the existing window-level listeners (003 FR-034).
  Reading without input counts as idle.
- **The native dialog**: the idle clock is paused by the backend while
  the dialog is up (research.md §12).

## 10. Screens walk (`e2e/screenshots/screens.e2e.ts`)

Add these, on the seeded record with documents:
- the document list showing a previewable PDF, a TIFF, a Word document
  (opens in another app) and a pre-007 JPEG row (can't be opened);
- the viewer on a 3-page PDF at fit width;
- the viewer on a multi-page TIFF;
- the viewer on a text document;
- the viewer's "can't be previewed here" state for a Word document;
- the viewer's password-protected state;
- the Database settings dialog with the "Documents" fieldset.

The native dialog is not part of the walk. It isn't drawn by the web view.
