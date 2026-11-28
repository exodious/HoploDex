# Contract: UI

## 1. The Import dialog's link (FR-001, FR-011)

Below the "Spreadsheet file" field in the dialog's first step, a note row reads "No spreadsheet yet?" followed by a small secondary button, **Get a blank spreadsheet…**. It is disabled while an import runs. Pressing it opens §2 above the Import dialog. Closing §2 (Cancel, Done, Escape or its close button) returns focus to the button, with the Import dialog as it was (its chosen files kept).

## 2. "Get a blank spreadsheet" dialog (FR-002, FR-008, FR-009, FR-011)

A `Dialog` (size `md`), titled **Get a blank spreadsheet**, described: "A spreadsheet with the columns import reads and no records, to fill in and import. It holds nothing from your collection." It is a `<form>` with these fields:

- **Format**: `ChoiceCards` as Export's, Excel first and selected:
  - **Excel (.xlsx)**: "One file with a Firearms and an Accessories sheet, and a note on each column."
  - **CSV (.csv)**: "Two plain-text files, firearms and accessories, with headers only."
- **Save to folder** (required): `ChosenPathField` with **Choose a folder…**, as Export's. With nothing chosen, Save shows the field error "Choose the folder to save the spreadsheet in." and writes nothing.
- No privacy note (FR-008).

The footer has **Cancel** and **Save** (`type="submit"`, primary, the file icon, "Saving…" while pending). Enter in the form submits Save, per the One Enter rule. A failure shows the backend's message in an `hd-banner--error` (`role="alert"`), with the dialog and the chosen folder kept.

**After a save**: an `hd-outcome` ("Saved a blank spreadsheet.") with the paths as Export's result shows them: **Spreadsheet** for a workbook, or **Firearms** and **Accessories** for CSV. Then **Done** closes the dialog.

## 3. Screenshot walk

`e2e/screenshots/screens.e2e.ts` shoots §2 above the Import dialog, before a folder is chosen. The Import dialog's existing shot shows §1.
