# Contract: UI (feature 008 deltas)

The visible changes:
- the pending changes dialog (amending 003's `contracts/ui-databases.md` §13);
- the import dialog's reading phase;
- the free-text maximums in the forms;
- what right-clicking and the browser keys do in the main window.

The session isolation of Story 1 has no visible surface of its own. What
the user sees is the absence of another session's data.

## 1. Pending changes dialog (amends 003 ui-databases.md §13; FR-003, FR-016, FR-017)

Still a non-dismissable `Dialog`, "Unsaved changes to <label>", shown
after an open that reports `pendingChanges` and before the collection can be
used.

**Body**: "The database locked on <date, time> while you were editing
<label>. Your changes were kept." Then, at most one of:

| Case | Added sentence |
|---|---|
| The record no longer exists (`resumable: false`) | "<label> no longer exists, so these changes can only be discarded." (unchanged) |
| Form version unknown, `savedByVersion` recorded and ≠ `appVersion` | "They were kept by HoploDex <savedByVersion>. Open the database with that version to resume them, or discard them here." |
| Form version unknown, `savedByVersion` = `appVersion` | "They were kept by a different build of HoploDex <appVersion>. Open the database with that build to resume them, or discard them here." |
| Form version unknown, `savedByVersion` null | "They were kept by another version of HoploDex. Open the database with that version to resume them, or discard them here." |

**Footer**, left to right:
- **Discard changes** (`danger`): opens the destructive `ConfirmDialog`
  ("Discard the changes to <label>?"), unchanged.
- **Close the database** (`secondary`, new, every case): a normal close (the
  closing screen, its backup when due, then the chooser with this database
  selected). The pending changes stay for the next open.
- **Resume editing** (`primary`, `autoFocus`, only when the changes can be
  resumed).

**Resuming** (research.md §6):
- **Resume editing** shows as pending, labelled "Opening…". The dialog stays
  up while the record or policy loads and its form opens beneath it, then
  closes as the form opens with the changes as unsaved input.
- **Close the database** stays available while it is "Opening…".
- If the form can't open, the dialog stays, Resume editing is offered
  again, and an error line (`hd-field__error`, `role="alert"`) says: "The
  form for <label> couldn't be opened. Your changes are still kept. Try
  again, close the database, or discard them."

**Focus**: Resume editing when offered, otherwise Close the database.
Discard is never the default.

## 2. Import dialog: reading the file (FR-006, FR-008; research.md §11)

- **While the file is read**, the progress bar's label reads "Reading
  <file name>…" (for two files, the file being read). It shows a
  percentage of the file read. The rows bar follows as before once rows
  are saved.
- **The footer's secondary button**:
  - while reading: **Stop reading**, enabled. It calls `cancel_import`. The
    dialog then shows `hd-banner--error`: "The import was cancelled.
    Nothing was imported." The chosen files stay, so Import can be pressed
    again.
  - while rows are saved: **Cancel**, disabled, as today.
- **A refused file** shows the backend's message in the dialog's existing
  error banner (`role="alert"`), for example: `inventory.xlsx, sheet
  "Sheet3": more than 100,000 rows. HoploDex imports at most 100,000 rows
  per table.` The chosen files stay listed.
- **A lock while reading** ends the session. The dialog goes with the
  collection, and no message is shown afterwards (FR-005).

## 3. Free-text maximums in forms (research.md §10)

Carried to every form with these fields (constitution III): FirearmForm,
AccessoryForm, DisposeDialog (recipient, for both kinds of record), and
InsurancePolicyForm.
- **Over the maximum**: the field shows its error at once, as the entry
  fields do ("<Label> can be at most <n> characters."), and the form's
  submit is refused with the same message from the backend. No `maxLength`
  attribute is used, so pasted text is never cut off silently.
- **Long text areas** (notes, accessories) take `TextArea`'s new `limit`
  prop. Past 90% of the maximum (3,600 characters) a counter appears under
  the field: "<n> of 4,000", announced politely (`aria-live="polite"`,
  through the field's `aria-describedby`). Over the maximum it takes the
  error style.

## 4. Right-click and browser keys in the main window (FR-010 to FR-014)

In a release (and E2E) build:

| Where | Right-click shows |
|---|---|
| Blank area, a row, a card, a button, an image, a dialog's body | nothing |
| A text field, text area or editable text | the system's editing menu, trimmed: cut, copy, paste, delete, select all, undo and redo where each applies and the system has it, plus its spelling suggestions and emoji |
| Text selected anywhere (a read-only field included) | the system's menu with copy (and select all where the system adds it) |

No menu ever offers back, forward, reload, stop, print, save, view source,
inspect, share, look up, translate, web search, services, speech or writing
tools.

| Keys and buttons | Effect |
|---|---|
| F5, Shift+F5, Ctrl+F5, Ctrl/⌘+R, Ctrl/⌘+Shift+R, Alt+←/→, ⌘[ / ⌘], the keyboard's Back, Forward and Refresh keys, the mouse's back and forward buttons | nothing; typed input is kept |
| Ctrl/⌘+P, Ctrl/⌘+S, Ctrl/⌘+U, Ctrl/⌘+F, F3, Ctrl/⌘+G, F7, F12, Ctrl/⌘+Shift+I/J/C | nothing |
| "/", Ctrl/⌘+L, Escape, and cut, copy, paste, undo, redo, select all, moving and selecting within text | unchanged |

In a development build (`npm run tauri dev`) the web view's own menu and
reload are as before.

## 5. Screenshot walk additions (`e2e/screenshots/screens.e2e.ts`)

- The pending changes dialog kept by another version: "Shared collection"
  seeded with `--pending-from 1.2.0`.
- The same dialog after a failed resume, showing its error line.
- The import dialog reading a large file, with **Stop reading**.
- The import dialog after a refused file, showing the limit message.
- FirearmForm with a note past 3,600 characters (the counter) and one over
  4,000 (the error).
