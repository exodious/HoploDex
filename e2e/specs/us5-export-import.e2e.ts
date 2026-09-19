import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { $, addFirearm, back, browser, clickButton, expect, fill } from "../support/ui";
import { choose, listedNames, search } from "../support/ui";

/**
 * End-to-end coverage of User Story 5's acceptance scenarios (spec.md),
 * driven against the real built app via tauri-driver / WebKitWebDriver.
 *
 * The folder and file fields accept typed paths (not only native pickers)
 * specifically so this spec can drive them — native OS file dialogs run
 * outside the webview and can't be automated through WebDriver. See
 * e2e/support/ui.ts for why interactions go through page JS.
 */

const HEADER =
  "make,model,serial_number,no_serial_attested,caliber,firearm_type,notes,accessories,status,estimated_value,acquisition_source,acquisition_date,acquisition_price,disposition_type,disposition_recipient,disposition_date,disposition_price,insurance_policy_name,coverage_kind,scheduled_coverage_amount,photo_filenames";

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
      `${HEADER}\nImportE2ERuger,10-22,IMP-001,FALSE,.22 LR,Rifle,,,,300.00,,,,,,,,,,,\n`,
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
      `${HEADER}\n,BadRow,IMP-BAD,FALSE,9mm,Handgun,,,,100.00,,,,,,,,,,,\nImportE2ESig,P226,IMP-002,FALSE,9mm,Handgun,,,,400.00,,,,,,,,,,,\n`,
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
      `${HEADER}\nExportE2EGlock,19,EXP-001,FALSE,9mm,Handgun,re-imported,,,,,,,,,,,,,,\n`,
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
});
