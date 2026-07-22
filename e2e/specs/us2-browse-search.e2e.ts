import { $, $$, browser, expect } from "@wdio/globals";

/**
 * End-to-end coverage of User Story 2's acceptance scenarios (spec.md),
 * driven against the real built app via tauri-driver / WebKitWebDriver.
 *
 * The app's SQLCipher database persists across E2E spec files (it lives in
 * the OS app-data directory, not reset per run), so this spec seeds its own
 * uniquely-named records rather than assuming an empty collection, and
 * asserts on those specific records rather than total collection counts.
 *
 * See us1-record-firearm.e2e.ts for the clickEl/clickInRole/setValueBySiblingInput
 * helper rationale (JS clicks, not native pointer clicks, for WebKitWebDriver
 * compatibility in this environment).
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

/**
 * Selects an option from the Type combobox inside the currently open
 * dialog. Scoped to `[role="dialog"]` because BrowsePage's own "Group by"
 * combobox stays mounted (just visually covered) underneath the dialog —
 * an unscoped `[role="combobox"]` selector would ambiguously match it
 * instead, since it appears first in document order.
 */
async function selectOption(optionLabel: string) {
  const trigger = await $('[role="dialog"]').$('[role="combobox"]');
  await trigger.waitForExist();
  await browser.execute((element: HTMLElement) => element.click(), trigger);
  await browser.pause(200);
  await clickEl(`[role="option"]=${optionLabel}`);
}

async function addFirearm(opts: {
  make: string;
  model: string;
  caliber: string;
  type: string;
  serial: string;
  notes?: string;
}) {
  await clickEl("button=Add firearm");
  await setValueBySiblingInput("label=Make", "input", opts.make);
  await setValueBySiblingInput("label=Model", "input", opts.model);
  await setValueBySiblingInput("label=Caliber", "input", opts.caliber);
  await selectOption(opts.type);
  await setValueBySiblingInput("label=Serial number", "input", opts.serial);
  if (opts.notes) {
    await setValueBySiblingInput("label=Notes", "textarea", opts.notes);
  }
  await clickInRole("dialog", "Add firearm");
  await clickEl("button=← Back to collection");
}

async function setSearch(term: string) {
  const field = await $("label=Search").parentElement().$("input");
  await field.setValue(term);
  await browser.pause(400);
}

async function clearSearch() {
  const field = await $("label=Search").parentElement().$("input");
  // Neither setValue("") (WebKitWebDriver: "Missing text parameter") nor
  // clearValue() (clears the DOM value without firing the `input` event
  // React's controlled component listens to, so its state never updates)
  // works here — send real Backspace keystrokes instead.
  await field.click();
  await browser.keys(["End"]);
  const currentValue = await field.getValue();
  for (let i = 0; i < currentValue.length + 5; i++) {
    await browser.keys(["Backspace"]);
  }
  await browser.pause(400);
}

describe("User Story 2 - Browse, Search, and Group", () => {
  before(async () => {
    await addFirearm({
      make: "BrowseSig",
      model: "P226",
      caliber: "9mm-Browse",
      type: "Handgun",
      serial: "BR-001",
    });
    await addFirearm({
      make: "BrowseRuger",
      model: "10-22-Browse",
      caliber: ".22 LR",
      type: "Rifle",
      serial: "BR-002",
      notes: "distinctivenotecrackedhandle",
    });
    await addFirearm({
      make: "BrowseMossberg",
      model: "500-Browse",
      caliber: "9mm-Browse",
      type: "Shotgun",
      serial: "BR-003",
    });
  });

  it("shows the same firearms in both list and tile view (Scenario 1)", async () => {
    await setSearch("Browse");
    await expect($("button*=BrowseSig P226")).toExist();

    await clickEl("button=Tiles");
    await expect($("button*=BrowseSig P226")).toExist();
    await expect($("button*=BrowseRuger 10-22-Browse")).toExist();
    await expect($("button*=BrowseMossberg 500-Browse")).toExist();

    await clickEl("button=List");
    await expect($("button*=BrowseSig P226")).toExist();
  });

  it("groups firearms by type (Scenario 2)", async () => {
    await clickEl('[role="combobox"]');
    await clickEl('[role="option"]=Type');
    await browser.pause(300);

    await expect($("h3=Handgun")).toExist();
    await expect($("h3=Rifle")).toExist();
    await expect($("h3=Shotgun")).toExist();

    // Reset grouping for subsequent tests.
    await clickEl('[role="combobox"]');
    await clickEl('[role="option"]=None');
  });

  it("searches free-form notes and returns only the matching firearm (Scenario 3)", async () => {
    await setSearch("distinctivenotecrackedhandle");
    await expect($("button*=BrowseRuger 10-22-Browse")).toExist();
    await expect($("button*=BrowseSig P226")).not.toExist();
  });

  it("searches a caliber value shared by multiple firearms (Scenario 4)", async () => {
    await setSearch("9mm-Browse");
    await expect($("button*=BrowseSig P226")).toExist();
    await expect($("button*=BrowseMossberg 500-Browse")).toExist();
    await expect($("button*=BrowseRuger 10-22-Browse")).not.toExist();
  });

  it("clearing search shows the full collection again (Scenario 5)", async () => {
    await clearSearch();
    const rows = await $$("li");
    expect(rows.length).toBeGreaterThanOrEqual(3);
    await expect($("button*=BrowseSig P226")).toExist();
    await expect($("button*=BrowseRuger 10-22-Browse")).toExist();
    await expect($("button*=BrowseMossberg 500-Browse")).toExist();
  });
});
