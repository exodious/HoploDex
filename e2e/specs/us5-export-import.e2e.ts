import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { $, addFirearm, back, browser, clickButton, expect, fill } from "../support/ui";
import { choose, fillFirearmForm, listedNames, openFirearm, search } from "../support/ui";
import { openPhysicalGroup, selectOption } from "../support/ui";
import { createDatabase } from "../support/ui";

/**
 * End-to-end coverage of User Story 5's acceptance scenarios (spec.md),
 * driven against the real built app via tauri-driver / WebKitWebDriver.
 *
 * The folder and file fields accept typed paths (not only native pickers)
 * specifically so this spec can drive them — native OS file dialogs run
 * outside the webview and can't be automated through WebDriver. See
 * e2e/support/ui.ts for why interactions go through page JS.
 */

/** The spreadsheet's columns, in order (contracts/spreadsheet-format.md). */
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

async function importFile(csvPath: string) {
  await clickButton("Import");
  await fill("Spreadsheet file", csvPath);
  await clickButton("Import");
  await $(".hd-tally").waitForExist({ timeout: 15000, timeoutMsg: "import never finished" });
}

async function tally(label: string): Promise<number> {
  return browser.execute((wanted: string) => {
    const item = [...document.querySelectorAll(".hd-tally__item")].find(
      (i) => i.querySelector(".hd-tally__label")?.textContent?.trim() === wanted,
    );
    return Number(item?.querySelector(".hd-tally__value")?.textContent ?? "NaN");
  }, label);
}

describe("User Story 5 - Export and Import Records", () => {
  // Each spec's session starts at the chooser with no databases (wdio.conf.ts).
  before(async () => {
    await createDatabase();
  });

  let workDir: string;

  before(() => {
    workDir = fs.mkdtempSync(path.join(os.tmpdir(), "hoplodex-e2e-export-"));
  });

  it("exports the collection to a spreadsheet and photos folder (Scenario 1)", async () => {
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
  });

  it("imports new records from a prepared spreadsheet (Scenario 2)", async () => {
    const csvPath = path.join(workDir, "import-new.csv");
    fs.writeFileSync(
      csvPath,
      csvFile(
        firearmRow("ImportE2ERuger", "10-22", "IMP-001", {
          caliber: ".22 LR",
          firearm_type: "Rifle",
          estimated_value: "300.00",
        }),
      ),
    );

    await importFile(csvPath);
    expect(await tally("added")).toBe(1);
    expect(await tally("failed")).toBe(0);
    await clickButton("Done");

    await search("ImportE2ERuger");
    expect(await listedNames()).toContain("ImportE2ERuger 10-22");
    await search("");
  });

  it("reports a failing row without discarding the successful one (Scenario 3)", async () => {
    const csvPath = path.join(workDir, "import-mixed.csv");
    fs.writeFileSync(
      csvPath,
      csvFile(
        firearmRow("", "BadRow", "IMP-BAD", { estimated_value: "100.00" }),
        firearmRow("ImportE2ESig", "P226", "IMP-002", { estimated_value: "400.00" }),
      ),
    );

    await importFile(csvPath);
    expect(await tally("added")).toBe(1);
    expect(await tally("failed")).toBe(1);
    await expect($(".hd-row-errors").$("li*=Row 1")).toExist();
    await clickButton("Done");

    await search("ImportE2ESig");
    expect(await listedNames()).toContain("ImportE2ESig P226");
    await search("");
  });

  it("exports only the current search results when asked (Edge Case: filtered export)", async () => {
    // Regression: the export dialog never offered the current results.
    await search("ImportE2E");
    await clickButton("Export");
    await $(".hd-choice__label=Current results (2)").waitForExist();
    await choose("Current results (2)");
    const filteredDir = fs.mkdtempSync(path.join(os.tmpdir(), "hoplodex-e2e-filtered-"));
    await fill("Save to folder", filteredDir);
    await clickButton("Export");

    await $(".hd-outcome__headline*=Exported 2 firearms").waitForExist({ timeout: 15000 });
    const spreadsheet = fs.readdirSync(filteredDir).find((f) => f.endsWith(".csv"))!;
    const contents = fs.readFileSync(path.join(filteredDir, spreadsheet), "utf-8");
    expect(contents).toContain("ImportE2ERuger");
    expect(contents).toContain("ImportE2ESig");
    expect(contents).not.toContain("ExportE2EGlock");
    await clickButton("Done");
    await search("");
  });

  it("asks what to do with rows matching an existing firearm (FR-026)", async () => {
    const csvPath = path.join(workDir, "import-conflict.csv");
    fs.writeFileSync(
      csvPath,
      csvFile(firearmRow("ExportE2EGlock", "19", "EXP-001", { notes: "re-imported" })),
    );

    await importFile(csvPath);
    expect(await tally("need a decision")).toBe(1);
    await expect($("button=Apply decisions")).toBeDisabled();

    await clickButton("Keep existing");
    await clickButton("Apply decisions");
    await $(".hd-outcome__headline*=Resolved 1 matching row").waitForExist();
    await clickButton("Done");

    // Keeping the existing record means no duplicate was added.
    await search("ExportE2EGlock");
    expect(await listedNames()).toEqual(["ExportE2EGlock 19"]);
  });

  it("carries physical details through an export and a re-import (US1 Scenario 17, FR-039)", async () => {
    await clickButton("Add firearm");
    await $('[role="dialog"]').waitForExist();
    await fillFirearmForm({
      make: "PhysE2E",
      model: "Colt",
      caliber: ".45 ACP",
      type: "Handgun",
      serial: "PHYS-001",
    });
    await openPhysicalGroup();
    await fill("Barrel length (in)", "5.25");
    await fill("Overall length (in)", "8.5");
    await fill("Weight (lb)", "2");
    await fill("Weight (oz)", "6.5");
    await fill("Capacity", "7");
    await fill("Finish", "Parkerized");
    await selectOption("Condition", "Like new");
    await clickButton("Add firearm");
    await $("#record-name").waitForExist();
    await back();

    await search("PhysE2E");
    await clickButton("Export");
    await $(".hd-choice__label=Current results (1)").waitForExist();
    await choose("Current results (1)");
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), "hoplodex-e2e-physical-"));
    await fill("Save to folder", dir);
    await clickButton("Export");
    await $(".hd-outcome__headline*=Exported 1 firearm").waitForExist({ timeout: 15000 });
    await clickButton("Done");

    // The six columns are plain numbers and the condition's display name.
    const exported = fs.readdirSync(dir).find((f) => f.endsWith(".csv"))!;
    const contents = fs.readFileSync(path.join(dir, exported), "utf-8");
    const [header, row] = contents.trim().split("\n");
    expect(header).toContain(
      "barrel_length_in,overall_length_in,weight_oz,capacity,finish,condition",
    );
    expect(row).toContain("5.25,8.5,38.5,7,Parkerized,Like new");

    // Re-import the file as a different firearm, so it is added, not matched.
    const copyPath = path.join(dir, "copy.csv");
    fs.writeFileSync(
      copyPath,
      contents.replace("PhysE2E,Colt,,PHYS-001", "PhysE2E,Copy,,PHYS-002"),
    );
    await search("");
    await importFile(copyPath);
    expect(await tally("added")).toBe(1);
    expect(await tally("failed")).toBe(0);
    await clickButton("Done");

    await search("PhysE2E");
    await openFirearm("PhysE2E Copy");
    const panel = await $('section[aria-labelledby="physical-title"]');
    await panel.waitForExist();
    const shown = (await panel.getText()).replace(/\s+/g, " ");
    for (const text of ["5.25 in", "8.5 in", "2 lb 6.5 oz", "7 rounds", "Parkerized", "Like new"]) {
      expect(shown).toContain(text);
    }
    await back();
    await search("");
  });
});
