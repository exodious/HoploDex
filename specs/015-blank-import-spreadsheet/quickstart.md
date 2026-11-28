# Quickstart: Blank Import Spreadsheet

## Automated

Run through the dev container (`DEVELOPMENT.md`):

```bash
scripts/dev-container.sh bash -c 'cd src-tauri && cargo nextest run --test integration import_template'
scripts/dev-container.sh npx vitest run src/features/import-export
scripts/dev-container.sh bash -c 'npm run build && npm run test:e2e -- --spec e2e/specs/export-import.e2e.ts'
```

They cover: the template's headers are the columns import reads (both tables, both formats); every workbook header has a note and every column is formatted as text; a blank template imports with nothing added or failed; a filled template imports its firearms and accessories with the mounts made; a second save takes a new name; a failed save leaves nothing; the dialog's states; and the command refuses a folder that wasn't picked.

## Manual (best effort, before a release)

**Setup**: run `scripts/human-testing.sh`. Open "Main collection" with the passphrase `human testing passphrase`. Have a spreadsheet tool (Excel, Numbers or LibreOffice Calc) installed.

**M1. Save and fill a workbook template**
1. Press **Import** in the top bar.
2. Press **Get a blank spreadsheet…**. Expect: Excel (.xlsx) selected.
3. Press **Choose a folder…** and pick `.human-testing/` (the sandbox's documents folder).
4. Press **Save**. Expect: "Saved a blank spreadsheet." and a path ending `hoplodex-import-template.xlsx`.
5. Open that file in the spreadsheet tool. Expect: sheets "Firearms" and "Accessories", headers only, a note on each header.
6. On "Firearms" row 2, type: make `M1Make`, model `M1`, serial_number `00123`, caliber `9mm`, firearm_type `Handgun`, acquisition_date `2024-03-01`. Save and close.
7. In HoploDex, press **Done**, then **Choose a file…**, pick the workbook, and press **Import**. Expect: 1 added, 0 failed.
8. Open "M1Make M1". Expect: serial number `00123`, acquired 2024-03-01.

**M2. A CSV template in a spreadsheet tool**
1. Repeat M1 steps 1–4 with **CSV (.csv)**. Expect: two paths, `hoplodex-import-template.csv` and `hoplodex-import-template-accessories.csv`.
2. Import both files together without opening them. Expect: 0 added, 0 failed.
