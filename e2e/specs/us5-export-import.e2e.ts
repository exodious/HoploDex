import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { $, browser, expect } from "@wdio/globals";

/**
 * End-to-end coverage of User Story 5's acceptance scenarios (spec.md),
 * driven against the real built app via tauri-driver / WebKitWebDriver.
 *
 * The destination-folder/file-path fields are plain editable text inputs
 * (not just native-dialog pickers) specifically so this spec can drive
 * them directly — native OS file/folder pickers run outside the webview
 * and cannot be automated through WebDriver. See us1-record-firearm.e2e.ts
 * for the JS-click rationale.
 */
async function clickEl(selector: string) {
  const el = await $(selector);
  await el.waitForExist();
  await browser.execute((element: HTMLElement) => element.click(), el);
  await browser.pause(200);
}

async function clickInRole(role: "dialog" | "alertdialog", buttonText: string) {
  const container = await $(`[role="${role}"]`);
  const btn = await container.$(`button=${buttonText}`);
  await btn.waitForExist();
  await browser.execute((element: HTMLElement) => element.click(), btn);
  await browser.pause(200);
}

async function setValueBySiblingInput(
  labelSelector: string,
  tag: "input" | "textarea",
  value: string,
) {
  const field = await $(labelSelector).parentElement().$(tag);
  await field.setValue(value);
  await browser.pause(100);
}

async function addFirearm(opts: { make: string; model: string; serial: string }) {
  await clickEl("button=Add firearm");
  await setValueBySiblingInput("label=Make", "input", opts.make);
  await setValueBySiblingInput("label=Model", "input", opts.model);
  await setValueBySiblingInput("label=Caliber", "input", "9mm");
  // Scoped to the open dialog: BrowsePage's own "Group by" combobox stays
  // mounted underneath and would otherwise match first.
  const typeTrigger = await $('[role="dialog"]').$('[role="combobox"]');
  await typeTrigger.waitForExist();
  await browser.execute((element: HTMLElement) => element.click(), typeTrigger);
  await browser.pause(200);
  await clickEl('[role="option"]=Handgun');
  await setValueBySiblingInput("label=Serial number", "input", opts.serial);
  await clickInRole("dialog", "Add firearm");
  await clickEl("button=← Back to collection");
}

const HEADER =
  "make,model,serial_number,no_serial_attested,caliber,firearm_type,notes,accessories,status,estimated_value,acquisition_source,acquisition_date,acquisition_price,disposition_type,disposition_recipient,disposition_date,disposition_price,insurance_policy_name,coverage_kind,scheduled_coverage_amount,photo_filenames";

describe("User Story 5 - Export and Import Records", () => {
  let workDir: string;

  before(() => {
    workDir = fs.mkdtempSync(path.join(os.tmpdir(), "hoplodex-e2e-export-"));
  });

  it("exports the collection to a spreadsheet and photos folder (Scenario 1)", async () => {
    await addFirearm({ make: "ExportE2EGlock", model: "19", serial: "EXP-001" });

    await clickEl("button=Export");
    const destinationInput = await $("label=Destination folder").parentElement().$("input");
    await destinationInput.setValue(workDir);
    await clickInRole("dialog", "Export");

    await browser.waitUntil(
      async () =>
        (await $("p*=Exported").isExisting()) || (await $('p[role="alert"]').isExisting()),
      { timeout: 15000, timeoutMsg: "export never completed" },
    );
    await expect($('p[role="alert"]')).not.toExist();
    await expect($("p*=Exported")).toExist();

    const files = fs.readdirSync(workDir);
    const spreadsheet = files.find((f) => f.endsWith(".csv"));
    expect(spreadsheet).toBeDefined();
    const contents = fs.readFileSync(path.join(workDir, spreadsheet!), "utf-8");
    expect(contents).toContain("ExportE2EGlock");

    const photosFolder = files.find((f) => f.endsWith("_photos"));
    expect(photosFolder).toBeDefined();

    await clickEl("button=Close");
  });

  it("imports new records from a prepared spreadsheet (Scenario 2)", async () => {
    const csvPath = path.join(workDir, "import-new.csv");
    fs.writeFileSync(
      csvPath,
      `${HEADER}\nImportE2ERuger,10-22,IMP-001,FALSE,.22 LR,Rifle,,,,300.00,,,,,,,,,,,\n`,
    );

    await clickEl("button=Import");
    const fileInput = await $("label=File path").parentElement().$("input");
    await fileInput.setValue(csvPath);
    await clickInRole("dialog", "Import");

    await browser.waitUntil(async () => await $("p*=Imported").isExisting(), {
      timeout: 15000,
      timeoutMsg: "import never completed",
    });
    await expect($("p*=Imported 1 new record")).toExist();
    await clickEl("button=Close");

    const searchField = await $("label=Search").parentElement().$("input");
    await searchField.setValue("ImportE2ERuger");
    await browser.pause(400);
    await expect($("button*=ImportE2ERuger 10-22")).toExist();
  });

  it("reports a failing row without discarding the successful one (Scenario 3)", async () => {
    const csvPath = path.join(workDir, "import-mixed.csv");
    fs.writeFileSync(
      csvPath,
      `${HEADER}\n,BadRow,IMP-BAD,FALSE,9mm,Handgun,,,,100.00,,,,,,,,,,,\nImportE2ESig,P226,IMP-002,FALSE,9mm,Handgun,,,,400.00,,,,,,,,,,,\n`,
    );

    await clickEl("button=Import");
    const fileInput = await $("label=File path").parentElement().$("input");
    await fileInput.setValue(csvPath);
    await clickInRole("dialog", "Import");

    await browser.waitUntil(async () => await $("p*=Imported").isExisting(), {
      timeout: 15000,
      timeoutMsg: "import never completed",
    });
    await expect($("p*=1 row(s) failed")).toExist();
    await expect($("li*=Row 1")).toExist();
    await clickEl("button=Close");

    const searchField = await $("label=Search").parentElement().$("input");
    await searchField.setValue("ImportE2ESig");
    await browser.pause(400);
    await expect($("button*=ImportE2ESig P226")).toExist();
  });
});
