# Data Model: Blank Import Spreadsheet

No table, column or migration changes. Nothing is stored.

## Read from the open database

The four fixed lists the notes name (research.md §4), never a record:

| List | Query | Used in the note of |
|---|---|---|
| Firearm types | `firearm_types`, every row, by `sort_order` | `firearm_type` |
| Actions | `action_types`, every row, by `sort_order` | `action_type` |
| Accessory kinds | `accessory_kinds` where `offered`, by `sort_order` | `kind` |
| Registration classifications | `registration_classes` where `offered`, by `sort_order` | `registered_as` |

## IPC types

- **`SaveImportTemplateInput`** `{ format: "xlsx" | "csv", folderToken: string }`
- **`ImportTemplateResult`** `{ spreadsheetPath: string, accessorySpreadsheetPath: string | null }`: the workbook, or the firearm CSV and the accessory CSV. As `ExportResult`'s paths, they are the paths actually used (`-2`, `-3`, … included).

## The template

See [contracts/spreadsheet-format.md](contracts/spreadsheet-format.md).
