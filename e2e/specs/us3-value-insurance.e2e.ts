import { $, browser, expect } from "@wdio/globals";

/**
 * End-to-end coverage of User Story 3's acceptance scenarios (spec.md),
 * driven against the real built app via tauri-driver / WebKitWebDriver.
 *
 * The app's SQLCipher database persists across E2E spec files, so this
 * spec seeds its own uniquely-named policies/firearms rather than assuming
 * a clean slate. See us1-record-firearm.e2e.ts for the JS-click rationale.
 *
 * Policies only appear in the value summary once at least one firearm is
 * assigned to them (byPolicy is built from firearms, not an independent
 * policy list — there is no separate "manage policies" list view in this
 * feature), so the expiry-warning scenarios (7-8) assign a firearm to make
 * the policy's warning visible.
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
  // Set the value via the native property setter and dispatch input/change
  // directly, bypassing WebDriver's interactability-checked setValue()
  // entirely. Two independent issues motivated this: (1) date inputs —
  // WebDriver's keystroke-based setValue() types into whichever date-
  // spinner segment currently has focus, in locale display order, silently
  // garbling an ISO string like "2020-01-01" into "0001-12-01"; (2) a
  // field can be flagged "not interactable" after ValueSummaryPanel
  // re-renders and shifts the page layout above it, even though it's
  // genuinely visible and usable. Both are WebDriver-level false
  // negatives/mis-translations, not real app defects.
  await browser.execute(
    (element: HTMLElement, text: string) => {
      const proto =
        element.tagName === "TEXTAREA"
          ? window.HTMLTextAreaElement.prototype
          : window.HTMLInputElement.prototype;
      const setter = Object.getOwnPropertyDescriptor(proto, "value")!.set!;
      setter.call(element, text);
      element.dispatchEvent(new Event("input", { bubbles: true }));
      element.dispatchEvent(new Event("change", { bubbles: true }));
    },
    field,
    value,
  );
  await browser.pause(100);
}

async function selectByLabel(labelText: string, optionLabel: string) {
  const trigger = await $(`label=${labelText}`).parentElement().$('[role="combobox"]');
  await trigger.waitForExist();
  await browser.execute((element: HTMLElement) => element.click(), trigger);
  await browser.pause(300);
  // Target the option's inner <span> text directly rather than the
  // `[role="option"]` div itself — WebKitWebDriver's role-based lookup can
  // fail to find a specific option's div by attribute+text even when it's
  // confirmed present in the live DOM, while its labelled child <span> is
  // reliably found (the same span-text pattern already used elsewhere in
  // this suite, e.g. PhotoGallery's "Thumbnail" label).
  await clickEl(`span=${optionLabel}`);
}

function isoDaysFromNow(days: number): string {
  const d = new Date();
  d.setDate(d.getDate() + days);
  return d.toISOString().slice(0, 10);
}

async function addPolicy(opts: {
  name: string;
  policyNumber: string;
  blanketLimitDollars: string;
  startDate: string;
  endDate: string;
}) {
  await clickEl("button=Add insurance policy");
  await setValueBySiblingInput("label=Policy name", "input", opts.name);
  await setValueBySiblingInput("label=Policy number", "input", opts.policyNumber);
  await setValueBySiblingInput("label=Insurance company", "input", "Acme Insurance");
  await setValueBySiblingInput(
    "label=Blanket coverage limit ($)",
    "input",
    opts.blanketLimitDollars,
  );
  await setValueBySiblingInput("label=Effective start date", "input", opts.startDate);
  await setValueBySiblingInput("label=Effective end date", "input", opts.endDate);
  await clickInRole("dialog", "Add policy");
}

/** Adds a firearm with an estimated value and leaves the app on its detail view. */
async function addFirearmWithValue(opts: {
  make: string;
  model: string;
  serial: string;
  valueDollars: string;
}) {
  await clickEl("button=Add firearm");
  await setValueBySiblingInput("label=Make", "input", opts.make);
  await setValueBySiblingInput("label=Model", "input", opts.model);
  await setValueBySiblingInput("label=Caliber", "input", "9mm");
  // Scoped to the open dialog: BrowsePage's own "Group by" combobox stays
  // mounted underneath and would otherwise ambiguously match first.
  const typeTrigger = await $('[role="dialog"]').$('[role="combobox"]');
  await typeTrigger.waitForExist();
  await browser.execute((element: HTMLElement) => element.click(), typeTrigger);
  await browser.pause(200);
  await clickEl('[role="option"]=Handgun');
  await setValueBySiblingInput("label=Serial number", "input", opts.serial);
  await setValueBySiblingInput("label=Estimated value ($)", "input", opts.valueDollars);
  await clickInRole("dialog", "Add firearm");
}

async function assignCoverage(opts: {
  policyName: string;
  kind: "Individually scheduled" | "Blanket";
  amountDollars?: string;
}) {
  // CoverageAssignment fetches its policy list asynchronously on mount;
  // give that IPC round trip time to resolve before opening the dropdown,
  // otherwise it can render with only "None" and the option WebDriver
  // looks for a moment later won't be found (a real fetch/render race,
  // not just a fixed animation delay).
  await browser.pause(500);
  await selectByLabel("Policy", opts.policyName);
  await selectByLabel("Coverage kind", opts.kind);
  if (opts.amountDollars) {
    // Radix's closing popper can transiently intercept pointer events at
    // the amount field's position right after the Coverage-kind dropdown
    // closes; a short settle pause avoids a spurious "not interactable".
    await browser.pause(300);
    await setValueBySiblingInput(
      "label=Scheduled coverage amount ($)",
      "input",
      opts.amountDollars,
    );
  }
  await clickEl("button=Save coverage");
  // Saving triggers an async value-summary refetch (two IPC round trips)
  // that re-renders ValueSummaryPanel, which sits above FirearmDetail and
  // can shift the whole page's layout once new content appears in it —
  // settle before any subsequent interaction (e.g. a second
  // assignCoverage call in the same test) touches now-relocated fields.
  await browser.pause(500);
}

describe("User Story 3 - Track Value and Insurance Coverage", () => {
  before(async () => {
    const farFuture = isoDaysFromNow(365);
    await addPolicy({
      name: "InsE2E Policy A",
      policyNumber: "A-1",
      blanketLimitDollars: "100000.00",
      startDate: "2020-01-01",
      endDate: farFuture,
    });
    await addPolicy({
      name: "InsE2E Policy B",
      policyNumber: "B-1",
      blanketLimitDollars: "800.00",
      startDate: "2020-01-01",
      endDate: farFuture,
    });
  });

  it("flags an uninsured firearm with no assigned policy (Scenario 1)", async () => {
    await addFirearmWithValue({
      make: "InsE2EGlock",
      model: "19",
      serial: "INS-U-1",
      valueDollars: "500.00",
    });

    await clickEl("button=← Back to collection");
    await expect($("h3=Unassigned")).toExist();
    const unassignedSection = await $("h3=Unassigned").parentElement();
    await expect(unassignedSection).toHaveText("InsE2EGlock 19", { containing: true });
    await expect(unassignedSection).toHaveText("Uninsured", { containing: true });
  });

  it("flags under-insured then clears once sufficiently scheduled (Scenarios 2-3)", async () => {
    await clickEl("button*=InsE2EGlock 19");
    await assignCoverage({
      policyName: "InsE2E Policy A",
      kind: "Individually scheduled",
      amountDollars: "300.00",
    });

    let policySection = await $("h3*=InsE2E Policy A").parentElement();
    await expect(policySection).toHaveText("$300.00 scheduled for a $500.00 value", {
      containing: true,
    });
    await expect(policySection).toHaveText("Under-insured", { containing: true });

    await assignCoverage({
      policyName: "InsE2E Policy A",
      kind: "Individually scheduled",
      amountDollars: "600.00",
    });

    policySection = await $("h3*=InsE2E Policy A").parentElement();
    await expect(policySection).toHaveText("$600.00 scheduled for a $500.00 value", {
      containing: true,
    });
    await expect(policySection).not.toHaveText("Under-insured", { containing: true });
  });

  it("flags a group blanket-limit-exceeded warning (Scenario 4)", async () => {
    await clickEl("button=← Back to collection");
    await addFirearmWithValue({
      make: "InsE2ESig",
      model: "P226",
      serial: "INS-B-1",
      valueDollars: "500.00",
    });
    await assignCoverage({ policyName: "InsE2E Policy B", kind: "Blanket" });

    await clickEl("button=← Back to collection");
    await addFirearmWithValue({
      make: "InsE2EMossberg",
      model: "500",
      serial: "INS-B-2",
      valueDollars: "500.00",
    });
    await assignCoverage({ policyName: "InsE2E Policy B", kind: "Blanket" });

    await clickEl("button=← Back to collection");
    const policySection = await $("h3*=InsE2E Policy B").parentElement();
    await expect(policySection).toHaveText("$1000.00 of $800.00", { containing: true });
    await expect(policySection).toHaveText("Under-insured", { containing: true });
  });

  it("updates the collection total immediately after add/edit/dispose (Scenario 5)", async () => {
    const totalBefore = await $("h2*=Collection value").getText();
    const before = Number(totalBefore.replace(/[^0-9.]/g, ""));

    await addFirearmWithValue({
      make: "InsE2ERuger",
      model: "10-22",
      serial: "INS-T-1",
      valueDollars: "200.00",
    });
    const afterCreate = await $("h2*=Collection value").getText();
    expect(Number(afterCreate.replace(/[^0-9.]/g, ""))).toBeCloseTo(before + 200, 2);

    await clickEl("button=Edit");
    await setValueBySiblingInput("label=Estimated value ($)", "input", "300.00");
    await clickInRole("dialog", "Save changes");
    const afterEdit = await $("h2*=Collection value").getText();
    expect(Number(afterEdit.replace(/[^0-9.]/g, ""))).toBeCloseTo(before + 300, 2);

    await clickEl("button=Mark disposed");
    // Scoped to the dialog: CoverageAssignment's own "Policy" combobox
    // stays mounted underneath and would otherwise match first.
    const dispositionTrigger = await $('[role="dialog"]').$('[role="combobox"]');
    await dispositionTrigger.waitForExist();
    await browser.execute((element: HTMLElement) => element.click(), dispositionTrigger);
    await browser.pause(200);
    await clickEl('[role="option"]=Sold');
    await setValueBySiblingInput("label=Recipient", "input", "Jane Doe");
    await setValueBySiblingInput("label=Date", "input", "2025-01-01");
    await setValueBySiblingInput("label=Price ($)", "input", "250.00");
    await clickInRole("dialog", "Confirm disposal");
    const afterDispose = await $("h2*=Collection value").getText();
    expect(Number(afterDispose.replace(/[^0-9.]/g, ""))).toBeCloseTo(before, 2);
  });

  it("breaks down the value summary by policy and unassigned group (Scenario 6)", async () => {
    await clickEl("button=← Back to collection");
    // Every other firearm created so far now has coverage assigned (or is
    // disposed) — add one more, left unassigned, so the "Unassigned"
    // group has something to actually show.
    await addFirearmWithValue({
      make: "InsE2EUnassigned",
      model: "1",
      serial: "INS-UNASSIGNED-1",
      valueDollars: "50.00",
    });
    await clickEl("button=← Back to collection");

    await expect($("h3*=InsE2E Policy A")).toExist();
    await expect($("h3*=InsE2E Policy B")).toExist();
    await expect($("h3=Unassigned")).toExist();
    await expect($("h2*=Collection value")).toExist();
  });

  it("flags a policy expiring within 30 days (Scenario 7)", async () => {
    await addPolicy({
      name: "InsE2E Policy C",
      policyNumber: "C-1",
      blanketLimitDollars: "100000.00",
      startDate: "2020-01-01",
      endDate: isoDaysFromNow(15),
    });
    await addFirearmWithValue({
      make: "InsE2EColt",
      model: "1911",
      serial: "INS-EXP-1",
      valueDollars: "100.00",
    });
    await assignCoverage({ policyName: "InsE2E Policy C", kind: "Blanket" });

    await clickEl("button=← Back to collection");
    const policySection = await $("h3*=InsE2E Policy C").parentElement();
    await expect(policySection).toHaveText("Policy expiring soon", { containing: true });
  });

  it("flags firearms on an expired policy as uninsured (Scenario 8)", async () => {
    await addPolicy({
      name: "InsE2E Policy D",
      policyNumber: "D-1",
      blanketLimitDollars: "100000.00",
      startDate: "2020-01-01",
      endDate: isoDaysFromNow(-5),
    });
    await addFirearmWithValue({
      make: "InsE2EWinchester",
      model: "94",
      serial: "INS-EXP-2",
      valueDollars: "100.00",
    });
    await assignCoverage({
      policyName: "InsE2E Policy D",
      kind: "Individually scheduled",
      amountDollars: "100.00",
    });

    await clickEl("button=← Back to collection");
    const policySection = await $("h3*=InsE2E Policy D").parentElement();
    await expect(policySection).toHaveText("Policy expired", { containing: true });
    await expect(policySection).toHaveText("Under-insured", { containing: true });
  });
});
