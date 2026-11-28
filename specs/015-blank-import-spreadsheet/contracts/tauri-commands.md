# Contract: Tauri commands

## `save_import_template` (new, scoped)

Writes a blank import spreadsheet into the folder the user picked with `choose_export_folder` (FR-001, FR-007). Scoped to the open session (the `HoploDex-Session` header), like `export_collection`.

**Input** `{ input: { format: "xlsx" | "csv", folderToken: string } }`

**Output** `ImportTemplateResult`:

```json
{ "spreadsheetPath": "/…/hoplodex-import-template.csv",
  "accessorySpreadsheetPath": "/…/hoplodex-import-template-accessories.csv" }
```

`accessorySpreadsheetPath` is `null` for a workbook, which holds both sheets.

**Behaviour**

- The folder is the answer `choose_export_folder` holds under `folderToken` (`ops::export_to_chosen_folder`). The save uses it up when it succeeds. When it fails, the save puts it back, so the dialog can try again.
- It reads the open database's four fixed lists and nothing else (data-model.md), and writes the files in [spreadsheet-format.md](spreadsheet-format.md), each created new.
- It asks nothing before a network folder, and registers no long-running operation (it takes well under a second).

**Errors**

| Code | When |
|---|---|
| `VALIDATION_ERROR` | `format` isn't `xlsx` or `csv`; the token isn't a held answer (a path, a guess, one already used, one for another question or session); the folder isn't there any more ("That folder isn't there any more. Choose the folder to export to again.", as export) |
| `DATABASE_CLOSED` | the session is no longer the open one |
| `LOCATION_OUT_OF_REACH` | inside a Flatpak, a folder it can't write (011 FR-004), as export |
| `DISK_FULL` | the disk filled while writing; nothing is left behind |
| `INTERNAL_ERROR` | any other failure to create or write a file; what was made is removed |
