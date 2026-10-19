import fs from "node:fs";
import path from "node:path";
import { $, addFirearm, back, browser, clickButton, expect, fill } from "../support/ui";
import { createDatabase, dialogHeightWhenOpened, listedNames } from "../support/ui";
import { scratchDocuments, search } from "../support/ui";
import { importFile, tally } from "../support/importExport";
import { writeSheetsWorkbook } from "../support/spreadsheets";

/**
 * End-to-end coverage of 001 US5's acceptance scenarios (001 spec.md),
 * driven against the real built app through its embedded WebDriver server.
 *
 * The folder and file fields accept typed paths (not only native pickers)
 * specifically so this spec can drive them — native OS file dialogs run
 * outside the webview and can't be automated through WebDriver. See
 * e2e/support/ui.ts for why interactions go through page JS.
 */

/** The spreadsheet's columns, in order (001 contracts/spreadsheet-format.md). */
const COLUMNS = [
  "make",
  "model",
  "nickname",
  "serial_number",
  "no_serial_attested",
  "caliber",
  "firearm_type",
  "notes",
  "accessories",
  "status",
  "estimated_value",
  "acquisition_source",
  "acquisition_date",
  "acquisition_price",
  "disposition_type",
  "disposition_recipient",
  "disposition_date",
  "disposition_price",
  "insurance_policy_name",
  "scheduled_coverage_amount",
  "barrel_length_in",
  "overall_length_in",
  "weight_oz",
  "capacity",
  "finish",
  "condition",
  "photo_filenames",
];
const HEADER = COLUMNS.join(",");

/** One data row from named cells; every column not named is blank. */
function csvRow(cells: Record<string, string>): string {
  for (const name of Object.keys(cells)) {
    if (!COLUMNS.includes(name)) throw new Error(`unknown spreadsheet column ${name}`);
  }
  return COLUMNS.map((column) => cells[column] ?? "").join(",");
}

/** A whole import file: the header plus the given rows. */
function csvFile(...rows: string[]): string {
  return `${[HEADER, ...rows].join("\n")}\n`;
}

/** A valid Handgun row for a make/model/serial, with any extra cells. */
function firearmRow(
  make: string,
  model: string,
  serial: string,
  extra: Record<string, string> = {},
): string {
  return csvRow({
    make,
    model,
    serial_number: serial,
    no_serial_attested: "FALSE",
    caliber: "9mm",
    firearm_type: "Handgun",
    ...extra,
  });
}

describe("001 US5 - Export and Import Records (was us5-export-import)", () => {
  // Each spec's session starts at the chooser with no databases (wdio.conf.ts).
  before(async () => {
    await createDatabase();
  });

  let workDir: string;

  before(() => {
    // In the sandbox, which afterSession removes, not the system temp folder.
    workDir = fs.mkdtempSync(path.join(scratchDocuments(), "export-"));
  });

  it("exports the collection to a spreadsheet and photos folder (001 US5-1)", async () => {
    await addFirearm({
      make: "ExportE2EGlock",
      model: "19",
      caliber: "9mm",
      type: "Handgun",
      serial: "EXP-001",
    });
    await back();

    await clickButton("Export");
    await fill("Save to folder", workDir);
    await clickButton("Export");

    await $(".hd-outcome__headline*=Exported").waitForExist({
      timeout: 15000,
      timeoutMsg: "export never finished",
    });
    await expect($('[role="alert"]')).not.toExist();

    const files = fs.readdirSync(workDir);
    const spreadsheet = files.find((f) => f.endsWith(".csv"));
    expect(spreadsheet).toBeDefined();
    expect(fs.readFileSync(path.join(workDir, spreadsheet!), "utf-8")).toContain("ExportE2EGlock");
    expect(files.find((f) => f.endsWith("_photos"))).toBeDefined();

    await clickButton("Done");

    // 001 US5-2: the exported file imports again. Make it a different
    // firearm, so it is added, not matched, and drop the record ID, which a
    // match would otherwise be made by (006 FR-019).
    const exported = fs.readFileSync(path.join(workDir, spreadsheet!), "utf-8");
    const copyPath = path.join(workDir, "copy.csv");
    fs.writeFileSync(
      copyPath,
      exported
        .replace("ExportE2EGlock,19,,EXP-001", "ExportE2EGlock,Copy,,EXP-002")
        .replace(/[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}/, ""),
    );
    await importFile(copyPath);
    expect(await tally("added")).toBe(1);
    expect(await tally("failed")).toBe(0);
    await clickButton("Done");
    await search("ExportE2EGlock");
    expect(await listedNames()).toContain("ExportE2EGlock Copy");

    // 001 US5-2 and US5-3: a prepared spreadsheet imports its good row, and
    // reports the failing one by table and row without discarding it.
    const preparedPath = path.join(workDir, "import-mixed.csv");
    fs.writeFileSync(
      preparedPath,
      csvFile(
        firearmRow("", "BadRow", "IMP-BAD", { estimated_value: "100.00" }),
        firearmRow("ImportE2ESig", "P226", "IMP-002", { estimated_value: "400.00" }),
      ),
    );
    await importFile(preparedPath);
    expect(await tally("added")).toBe(1);
    expect(await tally("failed")).toBe(1);
    await expect($(".hd-row-errors").$("li*=Firearms, row 1")).toExist();
    await clickButton("Done");
    await search("ImportE2ESig");
    expect(await listedNames()).toContain("ImportE2ESig P226");
    await search("");
  });

  // 008 US2 (008 spec.md FR-006 to FR-008; 008 contracts/ui.md §2).
  it("refuses a workbook over the import's limits and names it (008 US2, a refused file)", async () => {
    // Stands in for `import-samples/import-refused-too-many-sheets.xlsx`
    // (20 sheets; examples/human_seed.rs): this spec starts at a first run
    // with no seed, so it makes its own, with seventeen sheets, one more
    // than an import takes.
    const workbook = path.join(workDir, "too-many-sheets.xlsx");
    writeSheetsWorkbook(workbook, 17);

    await clickButton("Import");
    await fill("Spreadsheet file", workbook);
    await clickButton("Import");

    const banner = await $('[role="alert"]');
    await banner.waitForExist({ timeout: 15000, timeoutMsg: "the refusal never appeared" });
    const text = await banner.getText();
    expect(text).toContain("too-many-sheets.xlsx");
    expect(text).toContain("more than 16 sheets. HoploDex imports at most 16 sheets per workbook.");
    // The dialog stays on the file step, ready for another file.
    await expect($('[role="dialog"]').$("button=Import")).toExist();
    await clickButton("Cancel");
  });

  it("stops reading a large file when asked, and imports nothing (008 US2, Stop reading)", async () => {
    // 100,000 rows with a long note each: about 180 MB, under the 256 MB file
    // limit, and slow enough to read that the button can be pressed.
    const large = path.join(workDir, "large.csv");
    const note = "n".repeat(1800);
    const fd = fs.openSync(large, "w");
    fs.writeSync(fd, `${HEADER}\n`);
    const batch = 1000;
    for (let start = 0; start < 100_000; start += batch) {
      let chunk = "";
      for (let i = start; i < start + batch; i += 1) {
        chunk += `${firearmRow("StopE2E", `M${i}`, `STOP-${i}`, { notes: note })}\n`;
      }
      fs.writeSync(fd, chunk);
    }
    fs.closeSync(fd);

    await clickButton("Import");
    await fill("Spreadsheet file", large);
    // The reading is one request in flight, so the app stays busy until it ends.
    await clickButton("Import", { settles: false });
    // The bar names the file and shows how much is read.
    await browser.waitUntil(
      async () => /Reading large\.csv… [1-9]\d*%/.test(await $(".hd-progress__label").getText()),
      { timeout: 15000, timeoutMsg: "the reading never showed a percentage" },
    );

    await clickButton("Stop reading", { settles: false });
    const banner = await $('[role="alert"]');
    await banner.waitForExist({ timeout: 1000, timeoutMsg: "the reading didn't stop within 1 s" });
    expect(await banner.getText()).toContain("The import was cancelled. Nothing was imported.");
    await clickButton("Cancel");

    await search("StopE2E");
    expect(await listedNames()).toEqual([]);
    await search("");
  });
});

describe("Issue #116 - Export dialog shifts up a few pixels after it appears", () => {
  before(async () => {
    // The journey above leaves its database open when the whole file runs.
    if (await $("button=Create a new database…").isExisting()) await createDatabase();
  });

  // The dialog is centred and reads the export's counts and disclosure after
  // it appears. If those lengthened it then, it moved up by half the growth.
  for (const [name, term] of [
    ["the whole collection", ""],
    ["a search", "ExportE2E"],
  ] as const) {
    it(`opens as tall as it stays once its counts arrive, for ${name} (#116)`, async () => {
      await search(term);
      const { first, settled } = await dialogHeightWhenOpened(() =>
        clickButton("Export", { settles: false }),
      );
      expect(first).toBe(settled);
      await clickButton("Cancel");
      await search("");
    });
  }
});
