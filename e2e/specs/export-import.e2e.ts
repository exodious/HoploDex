import fs from "node:fs";
import path from "node:path";
import {
  $,
  addFirearm,
  back,
  browser,
  clickButton,
  clickEl,
  expect,
  invokeFailure,
} from "../support/ui";
import { createDatabase, dialogHeightWhenOpened, listedNames } from "../support/ui";
import { scratchDocuments, search } from "../support/ui";
import { chooseExportFolder, chooseImportFiles, importFile, tally } from "../support/importExport";
import { saveBlankSpreadsheet } from "../support/importExport";
import { writeSheetsWorkbook } from "../support/spreadsheets";
import { logLines } from "../support/documentPreview";

/**
 * End-to-end coverage of 001 US5's acceptance scenarios (001 spec.md),
 * driven against the real built app through its embedded WebDriver server.
 *
 * The folder and file are picked in the native choosers the backend opens,
 * which run outside the web view; an E2E build answers them from a queue the
 * spec fills (e2e/support/importExport.ts). See e2e/support/ui.ts for why
 * interactions go through page JS.
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
    await chooseExportFolder(workDir);
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

  // Code scanning alerts paths/cwe-73 on export_collection_stoppable and
  // add_document_from_path (amended 2026-11-19 to 001 FR-018, FR-019): the export writes
  // only to a folder, and the import reads only files, the user picked in the chooser
  // the backend opened, whatever the page names.
  it("exports only to a folder and imports only a file picked in the chooser (001 FR-018, FR-019)", async () => {
    const folder = fs.mkdtempSync(path.join(scratchDocuments(), "not-picked-"));
    const spreadsheet = path.join(folder, "not-picked.csv");
    fs.writeFileSync(spreadsheet, csvFile(firearmRow("NotPickedE2E", "19", "NP-001")));

    const exported = await invokeFailure("export_collection", {
      input: { format: "csv", folderToken: folder, scope: "all" },
    });
    expect(exported).toMatchObject({ code: "VALIDATION_ERROR" });
    expect(fs.readdirSync(folder)).toEqual(["not-picked.csv"]);

    const imported = await invokeFailure("import_collection", {
      input: { fileTokens: [spreadsheet] },
    });
    expect(imported).toMatchObject({ code: "VALIDATION_ERROR" });
    await search("NotPickedE2E");
    expect(await listedNames()).toEqual([]);
    await search("");
  });

  it("asks in the native confirmation, naming the folder and the records, before an export to a folder that may be on another computer", async () => {
    // An E2E build takes this folder to be on a network share (sandbox.ts).
    const share = path.join(process.env.HOPLODEX_E2E_NETWORK_FOLDER!, "Exports");
    fs.mkdirSync(share, { recursive: true });
    const asked = logLines("HOPLODEX_E2E_CONSENT_LOG").length;

    await clickButton("Export");
    await chooseExportFolder(share);
    await clickButton("Export");

    // Answered "Export" by the E2E build (HOPLODEX_E2E_CONSENT), so it is written.
    await $(".hd-outcome__headline*=Exported").waitForExist({
      timeout: 15000,
      timeoutMsg: "export never finished",
    });
    const consent = logLines("HOPLODEX_E2E_CONSENT_LOG");
    expect(consent).toHaveLength(asked + 1);
    expect(consent.at(-1)).toContain("Export to a folder that may be on another computer?");
    expect(consent.at(-1)).toContain(`“${share}” may be on another computer.`);
    expect(consent.at(-1)).toMatch(/HoploDex will write \d+ records? there/);
    expect(fs.readdirSync(share).some((f) => f.endsWith(".csv"))).toBe(true);
    await clickButton("Done");

    // A folder on this computer asks nothing.
    await clickButton("Export");
    await chooseExportFolder(fs.mkdtempSync(path.join(scratchDocuments(), "local-")));
    await clickButton("Export");
    await $(".hd-outcome__headline*=Exported").waitForExist({ timeout: 15000 });
    expect(logLines("HOPLODEX_E2E_CONSENT_LOG")).toHaveLength(asked + 1);
    await clickButton("Done");
  });

  it("takes a new backup location only as a folder picked in the chooser", async () => {
    const folder = fs.mkdtempSync(path.join(scratchDocuments(), "not-picked-"));
    const refused = await invokeFailure("update_backup_settings", {
      enabled: true,
      keepCount: 5,
      location: { kind: "chosen", token: folder },
    });
    expect(refused).toMatchObject({ code: "VALIDATION_ERROR" });
    const typed = await invokeFailure("update_backup_settings", {
      enabled: true,
      keepCount: 5,
      location: { kind: "custom", path: folder },
    });
    expect(typed).not.toBeNull();
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
    await chooseImportFiles(workbook);
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
    await chooseImportFiles(large);
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

// 015 (the blank import spreadsheet): saved from the Import dialog, imported
// untouched and filled in.

/** The database the 001 journey leaves open when the whole file runs, or a new one. */
async function openDatabase() {
  if (await $("button=Create a new database…").isExisting()) await createDatabase();
}

/** A data line for a template CSV file whose header line is `header`. */
function templateLine(header: string, cells: Record<string, string>): string {
  const columns = header.split(",");
  for (const name of Object.keys(cells)) {
    if (!columns.includes(name)) throw new Error(`the template has no ${name} column`);
  }
  return columns.map((column) => cells[column] ?? "").join(",");
}

const TEMPLATE = "hoplodex-import-template";

describe("015 US1 - Get a blank spreadsheet to fill in", () => {
  before(openDatabase);

  it("saves a workbook, then the CSV files, from the Import dialog, which stays open beneath (015 US1-1 to US1-5)", async () => {
    const folder = fs.mkdtempSync(path.join(scratchDocuments(), "template-"));
    await clickButton("Import");
    await clickButton("Get a blank spreadsheet…");
    expect(await $('input[type="radio"][value="xlsx"]').isSelected()).toBe(true);
    await chooseExportFolder(folder);
    await clickButton("Save");
    await $(".hd-outcome__headline=Saved a blank spreadsheet.").waitForExist({ timeout: 15000 });
    const workbook = path.join(folder, `${TEMPLATE}.xlsx`);
    // By name: the path shown is the folder as Rust reached it (a link resolved on macOS).
    await expect($(`dd*=${TEMPLATE}.xlsx`)).toExist();
    expect(fs.existsSync(workbook)).toBe(true);
    await clickButton("Done");

    await clickButton("Get a blank spreadsheet…");
    await clickEl('input[type="radio"][value="csv"]');
    await chooseExportFolder(folder);
    await clickButton("Save");
    await $(".hd-outcome__headline=Saved a blank spreadsheet.").waitForExist({ timeout: 15000 });
    const firearms = path.join(folder, `${TEMPLATE}.csv`);
    const accessories = path.join(folder, `${TEMPLATE}-accessories.csv`);
    await expect($(`dd*=${TEMPLATE}-accessories.csv`)).toExist();
    expect(fs.readFileSync(firearms, "utf-8")).toMatch(
      /^record_id,make,model,[^\n]*,mounted_on\r?\n$/,
    );
    expect(fs.readFileSync(accessories, "utf-8")).toMatch(
      /^record_id,kind,make,[^\n]*,mounted_on\r?\n$/,
    );
    await clickButton("Done");

    // 015 US1-5: the Import dialog is still open beneath it.
    await expect($('[role="dialog"]').$("button=Choose a file…")).toExist();
    await clickButton("Cancel");
  });

  // 015 FR-007, FR-008: the folder is only ever the chooser's answer, and a
  // network folder is written without export's question, since nothing of the
  // collection is in the file.
  it("writes only to a folder picked in the chooser, and asks nothing before a network folder (015 FR-007, FR-008)", async () => {
    const notPicked = fs.mkdtempSync(path.join(scratchDocuments(), "not-picked-"));
    const refused = await invokeFailure("save_import_template", {
      input: { format: "csv", folderToken: notPicked },
    });
    expect(refused).toMatchObject({ code: "VALIDATION_ERROR" });
    expect(fs.readdirSync(notPicked)).toEqual([]);

    // An E2E build takes this folder to be on a network share (sandbox.ts).
    const share = path.join(process.env.HOPLODEX_E2E_NETWORK_FOLDER!, "Templates");
    fs.mkdirSync(share, { recursive: true });
    const asked = logLines("HOPLODEX_E2E_CONSENT_LOG").length;
    await saveBlankSpreadsheet(share, "xlsx");
    expect(fs.existsSync(path.join(share, `${TEMPLATE}.xlsx`))).toBe(true);
    expect(logLines("HOPLODEX_E2E_CONSENT_LOG")).toHaveLength(asked);
  });
});

describe("015 US2 - Import a filled-in template", () => {
  before(openDatabase);

  it("imports the CSV template untouched with nothing added, then filled with a firearm and an accessory mounted on it (015 US2-1, US2-2, US2-4)", async () => {
    const folder = fs.mkdtempSync(path.join(scratchDocuments(), "template-"));
    await saveBlankSpreadsheet(folder, "csv");
    const firearms = path.join(folder, `${TEMPLATE}.csv`);
    const accessories = path.join(folder, `${TEMPLATE}-accessories.csv`);

    const importBoth = async () => {
      await clickButton("Import");
      await chooseImportFiles(firearms, accessories);
      await clickButton("Import");
      await $(".hd-tally").waitForExist({ timeout: 15000, timeoutMsg: "import never finished" });
    };
    await importBoth();
    expect(await tally("added")).toBe(0);
    expect(await tally("failed")).toBe(0);
    await clickButton("Done");

    // A made-up identifier in the firearm's record_id and the accessory's mounted_on.
    const uid = "0b6c3a8e-1f2d-4c5b-9a7e-3d2f1e0c9b8a";
    const firearmHeader = fs.readFileSync(firearms, "utf-8").trim();
    const accessoryHeader = fs.readFileSync(accessories, "utf-8").trim();
    fs.appendFileSync(
      firearms,
      `${templateLine(firearmHeader, {
        record_id: uid,
        make: "TemplateE2EMake",
        model: "T1",
        serial_number: "TPL-001",
        caliber: "9mm",
        firearm_type: "Handgun",
      })}\n`,
    );
    fs.appendFileSync(
      accessories,
      `${templateLine(accessoryHeader, {
        kind: "Optic",
        make: "TemplateE2EOptic",
        model: "O1",
        mounted_on: uid,
      })}\n`,
    );
    await importBoth();
    expect(await tally("added")).toBe(2);
    expect(await tally("failed")).toBe(0);
    await clickButton("Done");

    await search("TemplateE2EMake");
    expect(await listedNames()).toEqual(["TemplateE2EMake T1"]);
    expect(await $("body").getText()).toContain("1 accessory mounted");
    await search("");
  });
});

describe("015 US3 - Guidance that is never imported", () => {
  before(openDatabase);

  it("imports the workbook template, notes and all, with nothing added or failed (015 US3-2)", async () => {
    const folder = fs.mkdtempSync(path.join(scratchDocuments(), "template-"));
    await saveBlankSpreadsheet(folder, "xlsx");

    await importFile(path.join(folder, `${TEMPLATE}.xlsx`));
    expect(await tally("added")).toBe(0);
    expect(await tally("failed")).toBe(0);
    await clickButton("Done");
  });
});
