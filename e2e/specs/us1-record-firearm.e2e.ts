import { $, browser, expect } from "@wdio/globals";

/**
 * End-to-end coverage of User Story 1's acceptance scenarios (spec.md),
 * driven against the real built app via tauri-driver / WebKitWebDriver —
 * no mocks, exercising the full stack (React UI -> Tauri IPC -> SQLCipher).
 *
 * Clicks go through a plain JS `element.click()` rather than
 * WebdriverIO's native-pointer click: WebKitWebDriver's native click
 * pipeline in this environment has a driver-level "element click
 * intercepted" / "did not become interactable" quirk even when the
 * element is independently verified (via elementFromPoint at the same
 * coordinates) to be genuinely on top and clickable. None of this app's
 * interactions depend on real pointer-event coordinates, so a JS click is
 * behaviorally equivalent for React's onClick handlers.
 *
 * Note: creating a firearm navigates straight to its detail view (per
 * FirearmsPage.tsx), not back to the collection list — scenarios below
 * assert against the detail view right after creation rather than
 * expecting a list item to appear.
 */
async function clickEl(selector: string) {
  const el = await $(selector);
  await el.waitForExist();
  await browser.execute((element: HTMLElement) => element.click(), el);
  await browser.pause(200);
}

/** Some button labels (e.g. "Add firearm", "Delete") appear both as the
 * page-level trigger and inside the dialog/alertdialog it opens once that
 * dialog is showing — scope to the open dialog to click the right one. */
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

async function selectOption(optionLabel: string) {
  await clickEl('[role="combobox"]');
  await clickEl(`[role="option"]=${optionLabel}`);
}

describe("User Story 1 - Record a Firearm", () => {
  beforeEach(async () => {
    await browser.pause(300);
  });

  it("creates a firearm and shows it in the collection with its values intact (Scenario 1)", async () => {
    await clickEl("button=Add firearm");

    await setValueBySiblingInput("label=Make", "input", "Glock");
    await setValueBySiblingInput("label=Model", "input", "19");
    await setValueBySiblingInput("label=Caliber", "input", "9mm");
    await selectOption("Handgun");
    await setValueBySiblingInput("label=Serial number", "input", "E2E-001");

    await clickInRole("dialog", "Add firearm");

    // Creating navigates straight to the new record's detail view.
    await expect($("h2*=Glock 19")).toExist();
    await expect($("dd*=E2E-001")).toExist();
  });

  it("persists an edited field on reopen (Scenario 2)", async () => {
    // Still on the Glock 19 detail view from Scenario 1.
    await clickEl("button=Edit");

    await setValueBySiblingInput("label=Notes", "textarea", "scratch on left side");
    await clickInRole("dialog", "Save changes");

    await expect($("dd*=scratch on left side")).toExist();

    // Reopen via the collection list to confirm persistence beyond the
    // in-memory form/detail state. Target the list row's <button> (the
    // <li> wrapper itself has no click handler).
    await clickEl("button=← Back to collection");
    await clickEl("button*=Glock 19");
    await expect($("dd*=scratch on left side")).toExist();
  });

  it("saves acquisition details and shows them on the record (Scenario 3)", async () => {
    await clickEl("button=Edit");
    await setValueBySiblingInput("label=Acquisition source", "input", "Local gun shop");
    await setValueBySiblingInput("label=Acquisition date", "input", "2025-03-01");
    await setValueBySiblingInput("label=Acquisition price ($)", "input", "450.00");
    await clickInRole("dialog", "Save changes");

    await expect($("dd*=Local gun shop")).toExist();
  });

  it("marks a firearm disposed while retaining its history (Scenario 4)", async () => {
    await clickEl("button=Mark disposed");
    await selectOption("Sold");
    await setValueBySiblingInput("label=Recipient", "input", "Jane Doe");
    await setValueBySiblingInput("label=Date", "input", "2025-06-15");
    await setValueBySiblingInput("label=Price ($)", "input", "400.00");
    await clickInRole("dialog", "Confirm disposal");

    await expect($("dd*=Disposed")).toExist();
    // History retained: identifying details still shown.
    await expect($("dd*=E2E-001")).toExist();
  });

  it("deletes a firearm only after confirmation (Scenario 5)", async () => {
    await clickEl("button=← Back to collection");
    await clickEl("button=Add firearm");
    await setValueBySiblingInput("label=Make", "input", "ToDelete");
    await setValueBySiblingInput("label=Model", "input", "X");
    await setValueBySiblingInput("label=Caliber", "input", ".22");
    await selectOption("Other");
    await setValueBySiblingInput("label=Serial number", "input", "DEL-1");
    await clickInRole("dialog", "Add firearm");

    // Already on the new record's detail view.
    await expect($("h2*=ToDelete X")).toExist();
    await clickEl("button=Delete");

    const dialog = await $('[role="alertdialog"]');
    await expect(dialog).toExist();
    await clickInRole("alertdialog", "Delete");

    // Deleting returns to the collection list.
    await expect($("li*=ToDelete X")).not.toExist();
  });

  it("saves a blank serial number once attested (Scenario 6)", async () => {
    await clickEl("button=Add firearm");
    await setValueBySiblingInput("label=Make", "input", "Homemade");
    await setValueBySiblingInput("label=Model", "input", "80% build");
    await setValueBySiblingInput("label=Caliber", "input", ".223");
    await selectOption("Rifle");
    await clickEl("label=This firearm has no serial number");

    await clickInRole("dialog", "Add firearm");

    await expect($("h2*=Homemade 80% build")).toExist();
    await expect($("dd*=None (attested)")).toExist();
  });

  it("blocks saving a blank, unattested serial number (Scenario 7)", async () => {
    await clickEl("button=Add firearm");
    await setValueBySiblingInput("label=Make", "input", "Blocked");
    await setValueBySiblingInput("label=Model", "input", "Case");
    await setValueBySiblingInput("label=Caliber", "input", "9mm");
    await selectOption("Handgun");

    await clickInRole("dialog", "Add firearm");

    await expect($("p*=Enter a serial number, or confirm this firearm has none.")).toExist();
    await expect($("h2*=Blocked Case")).not.toExist();
  });
});
