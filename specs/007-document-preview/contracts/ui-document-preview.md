# Contract: Document Preview UI

The user-facing contract for 007. It covers:
- the document list's actions and the viewer;
- the native confirmation's text and the setting;
- attach refusals;
- keyboard and accessibility, and the screenshot walk.

Components are the shared ones in `src/components/` (`Dialog`, `Button`,
`Menu`, `SegmentedControl`, `useToast`). No new primitive is added;
`Dialog` gains a size (§2).

**Wording**:
- "Preview", never "View" or "Open" alone.
- "Open in another app…", with the ellipsis, because it asks first.
- "document", never "file" or "attachment", in the viewer's and the
  list's copy.
- "this computer" for the setting's scope and for PDF preview being off.

## 1. The document list (`DocumentList.tsx`)

Each row, left to right:

1. **The name**, a button.
   - **Setting "Preview in HoploDex"**: activating it opens the viewer
     (§2). For a document that can't be previewed, the viewer opens on
     the matching state (§3).
   - **Setting "Open in another app"**: it calls `open_document`. For a
     row whose `openable` is `false` there is nothing to open, so the name
     is plain text (not a button) with the reason as its `title`, matching
     the disabled "Open in another app…" button (amended 2026-10-07).
   - Its `title` says what will happen: "Preview {name}" or "Open {name}
     in another app".
2. **Meta line**: `{kind} · added {date}`, then, where it applies
   (FR-013: "the list MUST show which documents can be previewed"):
   - `previewKind` is `null`: "· opens in another app";
   - `previewKind` is set but `previewAvailable` is `false` (a PDF while
     PDF preview is off): "· can't be previewed on this computer";
   - `openable` is `false`: "· can't be opened: not a document type".
3. **"Preview"**: a ghost button with the `eye` icon, shown when
   `previewKind` is set, whatever the setting (FR-013). Disabled, with
   the reason as its `title`, when `previewAvailable` is `false`.
4. **"Open in another app…"**: a ghost button with the `open` icon,
   disabled with a `title` giving the reason when `openable` is `false`.
5. **Delete**: unchanged, an icon button with the same `ConfirmDialog`.

**Kind labels** (replacing `kindLabel`): from the `DocumentType.label` of
the recorded type: "PDF", "TIFF", "Plain text", "CSV", "RTF", "Word",
"Spreadsheet", "OpenDocument text", "OpenDocument spreadsheet". A pre-007
row of another type shows its extension in capitals.

**Busy state**: while `open_document` waits on the native dialog, its
button shows `pending`. On `{ opened: false }` nothing is said: the user
cancelled.

**Results of `open_document`**:
- **Success**: a toast, "Opened {name} in another app."
- **`NO_APP_FOR_DOCUMENT`**: an error toast, "This computer has no app
  that opens {kind} documents." (Amended 2026-10-07: on Windows the check
  comes before the dialog and no copy is made. Where the opener finds out
  later, the message adds " HoploDex deleted the copy it made.")
- **`DOCUMENT_TYPE_NOT_ALLOWED` / `DOCUMENT_CONTENT_MISMATCH`**: an error
  toast with the backend message.

## 2. The viewer (`DocumentPreview.tsx`, new)

A `Dialog` of the new size `xl` (90 vw × 90 vh), laid out like
`PhotoViewer` (constitution III).

**Header**:
- **Title**: the document's name.
- **Description**: `{kind} · added {date}`, and `{n} pages` for a
  multi-page TIFF. It also gives the position: `Document {i} of {count}`.
  A PDF's page count is shown by its viewer (FR-007).

**Toolbar** (above the page area; HoploDex's own, so the same on every
OS):
- **Multi-page TIFF**: first, previous, `Page {n} of {count}`, next, last;
  separator; zoom out, `{percent}%`, zoom in; fit width, fit page. The
  percent is a `Menu` offering 50–400% and the two fits.
- **Single-page TIFF**: zoom out, `{percent}%`, zoom in, fit, actual size.
- **Zoom range** (amended 2026-10-07): 50–400% in 25% steps. A fit can lie
  outside it (a tiny or a huge scan), and then the button that would move
  the wrong way is disabled: above 400%, zoom in is disabled and zoom out
  steps to 400%; below 50%, zoom out is disabled and zoom in steps to 50%.
  The `+` and `−` keys (§7) follow the same rule. Zoom in is also disabled
  at 400%, and zoom out at 50%.
- **PDF and text**: no toolbar. A PDF's page, zoom, find and selection
  controls are its viewer's own, inside the page area.

**Page area**:
- **PDF**: `PreviewSurface`, a placeholder `<div>` the PDF surface is
  placed over. It:
  - sends its rectangle with `set_preview_bounds` on mount, on resize and
    when the window resizes;
  - sends `visible: false` while anything is drawn above the viewer (the
    delete `ConfirmDialog`, a `Menu`, a toast) or while the native
    confirmation is up, and `visible: true` once it's gone;
  - shows "Preparing {name}…" with a spinner (`aria-live="polite"`)
    until `preview:pdf-ready`;
  - is a labelled region (`role="region"`, "{name}, PDF") with a visually
    hidden hint: "Press F6 to move into the document, and F6 again to come
    back."
- **TIFF**: a scroll area of pages, each a `<figure>` with an `<img>`
  (research.md §14–§15), with a 16 px gap and the page's shadow from
  `tokens.css`.
- **Text**: a `<pre class="hd-preview__text">` with `white-space:
  pre-wrap`, the mono font token, and the content as a text child.

**Toasts while a PDF is shown**: the viewer's footer has a status line
(`role="status"`) that takes every message the viewer causes while a PDF is
shown, in place of a toast: "Opened {name} in another app.", an open
failure, and, after "Delete document" in the viewer, "Deleted {name}." or
the deletion's failure (an error, in the danger colour). Nothing is then
drawn above the surface. The deletion's message follows the viewer to the
document it moves on to, and shows in the footer if that is a PDF; when it
is anything else, or the viewer closes, it is a toast. The line is cleared
when the user moves to another document or closes the viewer. A toast from
outside the viewer still hides the surface for as long as it shows
(`PreviewSurface`). (Amended 2026-10-07.)

**Footer**, matching `PhotoViewer`:
- previous and next document (icon buttons "Previous document" / "Next
  document"), moving only through this record's own documents (FR-007);
- "Delete document" (ghost), with the same `ConfirmDialog` as the list.
  After deleting, the viewer moves to the next document, or the previous,
  or closes if none is left;
- "Open in another app…" (secondary).

**Closing**: by Escape (in the surface too, through `preview:escape`),
the close control, or the record being navigated away from. Focus
returns to the control that opened the viewer. `close_preview` is called.

## 3. States in the page area

| State | Shown |
|---|---|
| PDF, before `preview:pdf-ready` | "Preparing {name}…" with a spinner |
| TIFF, before page 1 | page 1's slot, sized to an A4/letter placeholder, with a spinner and "Preparing page 1…" (`aria-live="polite"`). The toolbar is disabled. |
| TIFF page not yet rendered | a blank page-sized slot (`aria-busy="true"`) |
| `PREVIEW_UNSUPPORTED` | an icon and "{name} can't be previewed here. {Kind} documents open in another app.", plus a primary "Open in another app…" |
| `PDF_PREVIEW_UNAVAILABLE`, or `preview:pdf-ended` `noViewer` | "PDFs can't be previewed on this computer. {reason}", plus "Open in another app…". The reason is the backend's sentence, for example "This computer's PDF viewer couldn't be set up safely." |
| `preview:pdf-ended` `copyCaught` (macOS) | "This computer's PDF viewer saved a copy of {name} to disk. HoploDex deleted it and has turned PDF previews off on this computer until HoploDex is updated. You can still open PDFs in another app.", plus "Open in another app…" |
| `preview:pdf-ended` `failed`, or `PREVIEW_FAILED` | "{name} couldn't be previewed.", plus "Open in another app…" |
| `DOCUMENT_CONTENT_MISMATCH` | "{name} can't be previewed: its content isn't {a/an kind} document, or it holds macros, scripts, web page code, embedded objects or links that load outside content." (amended 2026-10-07: the viewer's own sentence repeats every reason `classify` refuses content, since a real Word or RTF document with an embedded object or an outside link is refused too.) No "Open in another app…" (FR-017). |
| `PREVIEW_DAMAGED` (TIFF) | "{name} can't be previewed: the document is damaged or incomplete.", plus "Open in another app…" |
| `PREVIEW_PAGE_FAILED` (TIFF) | in that page's slot: "Page {n} can't be shown." The other pages are unaffected. |
| A PDF the viewer can't show, or protected by a password | the viewer's own message or password prompt, inside the surface; "Open in another app…" stays in the footer (FR-006) |

**Kind in a sentence** (amended 2026-10-07): a kind label after the start of
a sentence ("its content isn't a {kind} document") is lower-cased, as in
"a plain text document", "a spreadsheet document". PDF, TIFF, CSV, RTF, a
pre-007 row's extension in capitals, the OpenDocument names and the product
name Word keep their case. At the start of a sentence it is as the label
is ("Plain text documents open in another app.").

Every message is a `role="status"` region. The previous and next document
buttons stay usable in every state. After `copyCaught` or `noViewer` the
record's document lists are reloaded.

## 4. The native confirmation (shown by Rust)

A system message dialog, warning kind, with two buttons: **"Open in
another app"** (default) and **"Cancel"** (Escape). It is parented to the
main window and modal. The PDF surface is hidden while it is up.

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

A new fieldset, **"Documents"**, after "Locking" and before "This
computer". It holds a `SegmentedControl` labelled "Open documents", with
"Preview in HoploDex" and "Open in another app", and the hint "Applies to
every database on this computer. Saved for this computer as soon as you
choose."

Choosing "Open in another app" calls `set_document_opening`:
- **while the native dialog is up**: the control shows the old value as
  pending;
- **on `{ changed: false }`**: it stays at "Preview in HoploDex".

Choosing "Preview in HoploDex" changes it at once. The setting is saved
immediately, not by the dialog's Save, because it is this computer's and
not the database's.

## 6. Attaching: refusals (FR-016)

- **The picker**: the document file input's `accept` is built from
  `list_document_types`
  (`.pdf,.tif,.tiff,.txt,.csv,.rtf,.doc,.docx,.xls,.xlsx,.odt,.ods`).
- **Drops**:
  - JPEG and PNG still go to Photos (`isPhotoPath`).
  - `isDocumentPath` is now "the extension is in `list_document_types`",
    no longer "not a photo".
  - A drop waits for `list_document_types` before it is routed, so a drop
    made just after the page opens is judged by the types and not by an
    empty list. If the types can't be loaded, every dropped file but a
    photo goes to the backend, whose content check refuses what isn't a
    document type.
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

**On the viewer, focus on HoploDex's controls**:
- **← / →**: previous and next document (as `PhotoViewer`).
- **Escape**: closes the viewer.
- **Tab**: reaches every toolbar and footer control in visual order.
- **F6**: for a PDF, moves focus into the surface (`focus_preview`).

**In a PDF** (focus in the surface): the computer's viewer's own keys for
scrolling, paging, zooming, find and selection, which differ by OS, except:
- **Escape**: closes the viewer (`preview:escape`);
- **F6**: back to the viewer's first control (`preview:focus-chrome`);
- the viewer's save, print and open shortcuts do nothing (research.md §7).

**In a TIFF's page area** (focusable, `tabIndex=0`, labelled "{name},
pages"):
- **↑ / ↓ / Space / Shift+Space**: scroll.
- **PageUp / PageDown**: previous and next page.
- **Home / End**: first and last page.
- **+ / −**: zoom in and out (the range rule of §2 applies).
- **0**: fit width; for a single-page TIFF, fit.
- **1**: actual size (single-page TIFF).
- **← / →**: scroll sideways when the page is wider than the view.

**In a text document's page area** (focusable, labelled "{name}"): the
usual scrolling keys.

## 8. Accessibility

- **The dialog**: `aria-labelledby` the title and `aria-describedby` the
  description.
- **PDF**: the surface's own accessibility tree, entered with F6 or by the
  screen reader's own navigation (research.md §15); the placeholder region
  names it and gives the F6 hint.
- **TIFF pages**: `alt="{name}, page {n} of {count}"`; a single-page TIFF,
  `alt="{name}"`.
- **The page indicator** (TIFF): `aria-live="polite"`, announcing on page
  changes made by the controls, not on scroll.
- **Zoom percent** (TIFF): in the zoom buttons' `aria-describedby`.
- **Contrast and focus rings**: the existing tokens, which pass WCAG 2.1
  AA.

## 9. Lock, close and the idle clock

- **Lock**: the viewer is inside the collection providers, so a lock,
  close, switch or sleep unmounts it with the rest (SC-006). The PDF
  surface is closed by Rust on the same path, before the frontend hears of
  the lock (research.md §20). The viewer's cleanup revokes `blob:` URLs and
  drops the text.
- **Idle**: input in HoploDex's controls and in TIFF and text pages
  counts through the existing window-level listeners; input in the PDF
  surface is reported by Rust (research.md §10). Reading without input
  counts as idle (003 FR-034).
- **The native dialog**: the idle clock is paused by the backend while
  the dialog is up (research.md §16).

## 10. Screens walk (`e2e/screenshots/screens.e2e.ts`)

On the seeded record with documents:
- the document list showing a previewable PDF, a TIFF, a Word document
  (opens in another app) and a pre-007 JPEG row (can't be opened);
- the viewer on a 3-page PDF;
- the viewer on a multi-page TIFF;
- the viewer on a text document;
- the viewer's "can't be previewed here" state for a Word document;
- the viewer's "PDFs can't be previewed on this computer" state
  (`HOPLODEX_E2E_PDF_PREVIEW=off`, an E2E-only switch that sets
  `PdfAvailability` to `CheckFailed`, held out of release builds by
  `check-no-webdriver.mjs` like the consent seam);
- the Database settings dialog with the "Documents" fieldset.

A WebDriver screenshot holds only the main web view, so the PDF screen is
taken as a screenshot of the whole window from the X display (`import
-window`), which includes the surface.

The native dialog isn't part of the walk; the web view doesn't draw it.
