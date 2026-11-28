# Contract: The blank import spreadsheet

The template is laid out as an export is ([006's "Two tables"](../../006-accessory-links/contracts/spreadsheet-format.md#two-tables)), and is read by the import exactly as an export is. No import rule changes.

## Files

| Format | Firearm table | Accessory table |
|---|---|---|
| Excel | sheet **"Firearms"** of `{base}.xlsx` | sheet **"Accessories"** of the same workbook |
| CSV | `{base}.csv` | `{base}-accessories.csv`, beside it |

`{base}` is `hoplodex-import-template`, or `hoplodex-import-template-2`, `-3`, … when the folder already holds any of that name's files (research.md §3). Both tables are always written. The workbook has no other sheet.

## Columns

Each table's header row is its column list without `photo_filenames` (`EXPORT_ONLY_COLUMNS`): 41 firearm columns, `record_id` to `mounted_on` in `FIREARM_COLUMNS`' order, and 20 accessory columns, `record_id` to `mounted_on` in `ACCESSORY_COLUMNS`' order. There is no data row.

A CSV template is these header lines only, written by the export's CSV writer.

## Workbook extras (FR-005, FR-006)

- **A note on every header cell**, with no author prefix. Notes are not cells: import never reads them.
- **Every column formatted as text** (number format `@`), so what is typed is stored as typed.
- Each column as wide as its header.

## The notes

`{…}` is read from the open database (data-model.md). "Whole dollars" notes all add: "A $ sign and commas are fine; cents are not." Where both tables have a column, the note is the same unless the table is named.

| Column | Note |
|---|---|
| `record_id` (Firearms) | Optional. Leave blank for a new firearm; HoploDex gives it an ID. The ID of a firearm you already have matches that firearm. To mount an accessory on a firearm in this file, give the firearm a made-up ID (a version 4 UUID, such as 3f2b8c1e-5d4a-4e6f-9b7c-2a1d0e9f8c7b, a different one for each) and put the same ID in the accessory's mounted_on. |
| `record_id` (Accessories) | Optional. Leave blank for a new accessory; HoploDex gives it an ID. The ID of an accessory you already have matches that accessory. |
| `make` | Required. The maker, for example Ruger. |
| `model` | Required. For example 10/22. |
| `nickname` | Optional. A name of your own for it, different from every other active firearm's. |
| `serial_number` (Firearms) | Required, unless no_serial_attested is TRUE; then leave it blank. As marked, leading zeros included. |
| `serial_number` (Accessories) | Optional. As marked, leading zeros included. |
| `no_serial_attested` | Optional. TRUE when the firearm has no serial number; otherwise FALSE or blank. |
| `caliber` (Firearms) | Required, unless cartridge is given and the caliber can be worked out from it (never for a Suppressor). For example 9mm. |
| `caliber` (Accessories) | Optional. Worked out from cartridge when blank, if it can be. |
| `cartridge` | Optional. The cartridge it is chambered for, for example 9x19mm Parabellum. |
| `action_type` | Optional. One of: {actions}. It must suit firearm_type; a Suppressor has none. |
| `firearm_type` | Required. One of: {types}. |
| `kind` | Required. One of: {kinds}. |
| `notes` | Optional. Any text. |
| `accessories` | Optional. Any text, such as a list separated by semicolons. To record an accessory as an item of its own, use the Accessories sheet. |
| `status` | Optional. active (the default) or disposed. |
| `estimated_value` | Optional. Whole dollars, for example 450. |
| `acquisition_source` | Optional. Where it came from, for example a shop or a person. |
| `acquisition_date` | Optional. The date it was acquired, as YYYY-MM-DD, for example 2024-03-01. Not a future date. |
| `acquisition_price` | Optional. Whole dollars. |
| `disposition_type` | Required when status is disposed. One of: sold, traded, gifted, destroyed, lost_stolen. |
| `disposition_recipient` | Required when status is disposed. Who it went to. |
| `disposition_date` | Required when status is disposed. YYYY-MM-DD, not a future date and not before acquisition_date. |
| `disposition_price` | Optional, even when disposed. Whole dollars. |
| `insurance_policy_name` | Optional. The name of an insurance policy already in this database, to schedule it under that policy. |
| `scheduled_coverage_amount` | Required when insurance_policy_name is given; blank otherwise. Whole dollars. |
| `barrel_length_in` | Optional. Inches, up to two decimal places, for example 16.25. Not for a Suppressor. |
| `overall_length_in` | Optional. Inches, up to two decimal places, for example 36.5. |
| `weight_oz` | Optional. Ounces, up to one decimal place, for example 40.5. |
| `capacity` | Optional. A whole number of rounds, 1 or more. Not for a Suppressor. |
| `finish` | Optional. For example Blued. |
| `condition` | Optional. One of: {the six condition labels}. |
| `origin` | Optional. One of: Domestic, Imported, Re-imported. Blank when not known. |
| `year_of_manufacture` | Optional. A four-digit year, for example 1943. |
| `country_of_manufacture` | Optional. Only when origin is Imported. |
| `importer_name` | Optional. Only when origin is Imported or Re-imported. |
| `original_make`, `original_model`, `original_serial_number` | Optional. The original maker's marks, only when origin is Imported or Re-imported. |
| `registered_as` | Optional. One of: {classifications}. |
| `registration_form` | Optional. For example Form 4. Needs registered_as. |
| `registration_approved` | Optional. The date it was approved, as YYYY-MM-DD. Not a future date. Needs registered_as. |
| `registered_to` | Optional. Who it is registered to. Needs registered_as. |
| `mounted_on` (Firearms) | Optional. To record this firearm as mounted on another item, that item's record_id, from either sheet or from your collection. |
| `mounted_on` (Accessories) | Optional. The record_id of the firearm or accessory it is mounted on, from either sheet or from your collection. To mount it on a firearm in this file, give that firearm a made-up record_id and use the same one here. Or leave it blank and mount it in HoploDex after the import. |

Every value is matched in any letter case where a list is given, as import already does.
