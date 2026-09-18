import { $, addFirearm, back, browser, choose, clickButton, expect, fill } from "../support/ui";
import { goTo, openFirearm, policyCardText, selectOption, titleBlock } from "../support/ui";

/**
 * End-to-end coverage of User Story 3's acceptance scenarios (spec.md),
 * driven against the real built app via tauri-driver / WebKitWebDriver.
 * Seeds its own uniquely-named policies and firearms rather than assuming
 * a clean slate. See e2e/support/ui.ts for why interactions go through
 * page JS.
 */

/** A local calendar date `days` from today, as YYYY-MM-DD. */
function isoDaysFromNow(days: number): string {
  const d = new Date();
  d.setDate(d.getDate() + days);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

async function addPolicy(opts: {
  name: string;
  policyNumber: string;
  blanketLimitDollars: string;
  startDate: string;
  endDate: string;
}) {
  await goTo("Insurance");
  await clickButton("Add policy");
  await fill("Policy name", opts.name);
  await fill("Policy number", opts.policyNumber);
  await fill("Insurance company", "Acme Insurance");
  await fill("Blanket coverage limit", opts.blanketLimitDollars);
  await fill("Coverage starts", opts.startDate);
  await fill("Coverage ends", opts.endDate);
  await clickButton("Add policy");
  await $(`article.hd-policy*=${opts.name}`).waitForExist();
  await goTo("Collection");
}

async function addFirearmWithValue(opts: {
  make: string;
  model: string;
  serial: string;
  valueDollars: string;
}) {
  await addFirearm({ ...opts, caliber: "9mm", type: "Handgun" });
}

/** Assigns coverage from the open record's Insurance panel. */
async function assignCoverage(opts: {
  policyName: string;
  kind: "Scheduled individually" | "Blanket";
  amountDollars?: string;
}) {
  await clickButton((await $("button=Change").isExisting()) ? "Change" : "Assign");
  await selectOption("Policy", opts.policyName);
  await choose(opts.kind);
  if (opts.amountDollars) await fill("Scheduled amount", opts.amountDollars);
  await clickButton("Save coverage");
  await $('[role="dialog"]').waitForExist({ reverse: true });
  await browser.pause(400);
}

/** The collection's total estimated value, in dollars, from the page header. */
async function collectionTotal(): Promise<number> {
  await goTo("Collection");
  const text = await $(".hd-page-sub").getText();
  const match = /\$([\d,]+(?:\.\d\d)?)/.exec(text);
  return Number(match![1].replace(/,/g, ""));
}

describe("User Story 3 - Track Value and Insurance Coverage", () => {
  before(async () => {
    const farFuture = isoDaysFromNow(365);
    await addPolicy({
      name: "InsE2E Policy A",
      policyNumber: "A-1",
      blanketLimitDollars: "100,000.00",
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

    expect(await titleBlock("Coverage")).toBe("Uninsured");
    await expect($(".hd-coverage*=No policy assigned")).toExist();

    await goTo("Insurance");
    const gaps = await $("section*=Not covered").getText();
    expect(gaps).toContain("InsE2EGlock 19");
  });

  it("flags under-insured then clears once sufficiently scheduled (Scenarios 2-3)", async () => {
    await goTo("Collection");
    await openFirearm("InsE2EGlock 19");
    await assignCoverage({
      policyName: "InsE2E Policy A",
      kind: "Scheduled individually",
      amountDollars: "300.00",
    });

    expect(await titleBlock("Coverage")).toBe("Under-insured");
    await expect($(".hd-coverage*=$200 short of its value")).toExist();
    await goTo("Insurance");
    expect(await policyCardText("InsE2E Policy A")).toContain("$200.00 short");

    await goTo("Collection");
    await openFirearm("InsE2EGlock 19");
    await assignCoverage({
      policyName: "InsE2E Policy A",
      kind: "Scheduled individually",
      amountDollars: "600.00",
    });

    expect(await titleBlock("Coverage")).toBe("Covered");
    await goTo("Insurance");
    expect(await policyCardText("InsE2E Policy A")).not.toContain("short");
  });

  it("flags a group blanket-limit-exceeded warning (Scenario 4)", async () => {
    await goTo("Collection");
    await addFirearmWithValue({
      make: "InsE2ESig",
      model: "P226",
      serial: "INS-B-1",
      valueDollars: "500.00",
    });
    await assignCoverage({ policyName: "InsE2E Policy B", kind: "Blanket" });
    expect(await titleBlock("Coverage")).toBe("Covered");

    await back();
    await addFirearmWithValue({
      make: "InsE2EMossberg",
      model: "500",
      serial: "INS-B-2",
      valueDollars: "500.00",
    });
    await assignCoverage({ policyName: "InsE2E Policy B", kind: "Blanket" });
    expect(await titleBlock("Coverage")).toBe("Under-insured");

    await goTo("Insurance");
    const card = await policyCardText("InsE2E Policy B");
    expect(card).toContain("$1,000 of $800 limit");
    expect(card).toContain("Over by $200");
  });

  it("updates the collection total immediately after add/edit/dispose (Scenario 5)", async () => {
    const before = await collectionTotal();

    await addFirearmWithValue({
      make: "InsE2ERuger",
      model: "10-22",
      serial: "INS-T-1",
      valueDollars: "200.00",
    });
    expect(await collectionTotal()).toBeCloseTo(before + 200, 2);

    await openFirearm("InsE2ERuger 10-22");
    await clickButton("Edit");
    await fill("Estimated replacement value", "300.00");
    await clickButton("Save changes");
    expect(await collectionTotal()).toBeCloseTo(before + 300, 2);

    await openFirearm("InsE2ERuger 10-22");
    await clickButton("Mark disposed");
    await choose("Sold");
    await fill("Transferred to", "Jane Doe");
    await fill("Date", "2025-01-01");
    await fill("Price received", "250.00");
    await clickButton("Mark as disposed");
    expect(await collectionTotal()).toBeCloseTo(before, 2);
  });

  it("breaks down the value summary by policy and unassigned group (Scenario 6)", async () => {
    await addFirearmWithValue({
      make: "InsE2EUnassigned",
      model: "1",
      serial: "INS-UNASSIGNED-1",
      valueDollars: "50.00",
    });

    await goTo("Insurance");
    await expect($(".hd-page-sub*=estimated replacement value")).toExist();
    await expect($("article.hd-policy*=InsE2E Policy A")).toExist();
    await expect($("article.hd-policy*=InsE2E Policy B")).toExist();
    const gaps = await $("section*=Not covered").getText();
    expect(gaps).toContain("InsE2EUnassigned 1");
  });

  it("flags a policy expiring within 30 days (Scenario 7)", async () => {
    await addPolicy({
      name: "InsE2E Policy C",
      policyNumber: "C-1",
      blanketLimitDollars: "100,000.00",
      startDate: "2020-01-01",
      endDate: isoDaysFromNow(15),
    });

    await goTo("Insurance");
    expect(await policyCardText("InsE2E Policy C")).toContain("Expires in 15 days");
    await goTo("Collection");
    await expect($(".hd-attention*=InsE2E Policy C")).toExist();
  });

  it("flags firearms on an expired policy as uninsured (Scenario 8)", async () => {
    await addPolicy({
      name: "InsE2E Policy D",
      policyNumber: "D-1",
      blanketLimitDollars: "100,000.00",
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
      kind: "Scheduled individually",
      amountDollars: "100.00",
    });

    expect(await titleBlock("Coverage")).toBe("Uninsured");
    await expect($(".hd-coverage*=expired")).toExist();

    await goTo("Insurance");
    const card = await policyCardText("InsE2E Policy D");
    expect(card).toContain("Expired");
    expect(card).toContain("Uninsured");
  });

  it("treats a renewed policy's firearms as insured again (Edge Case: renewal)", async () => {
    await goTo("Insurance");
    await clickInPolicyCard("InsE2E Policy D", "Edit");
    await fill("Coverage ends", isoDaysFromNow(200));
    await clickButton("Save changes");
    await browser.pause(400);

    const card = await policyCardText("InsE2E Policy D");
    expect(card).toContain("In force");
    expect(card).not.toContain("Uninsured");
    expect(card).toContain("Covered");
  });

  it("won't delete a policy that still covers firearms (Edge Case)", async () => {
    await goTo("Insurance");
    await clickInPolicyCard("InsE2E Policy D", "Delete InsE2E Policy D");
    await expect($('[role="dialog"]*=still covers firearms')).toExist();
    await expect($('[role="dialog"]*=InsE2EWinchester 94')).toExist();
    await clickButton("OK");
    await expect($("article.hd-policy*=InsE2E Policy D")).toExist();

    // A policy with nothing assigned can be deleted, after confirmation.
    await clickInPolicyCard("InsE2E Policy C", "Delete InsE2E Policy C");
    await clickButton("Delete policy");
    await browser.waitUntil(
      async () => !(await $("article.hd-policy*=InsE2E Policy C").isExisting()),
      {
        timeoutMsg: "the deleted policy is still listed",
      },
    );
  });
});

/** Clicks a button — by visible text or accessible name — inside the
 * policy card titled `policyName`. */
async function clickInPolicyCard(policyName: string, button: string) {
  const clicked = await browser.execute(
    (name: string, wanted: string) => {
      const card = [...document.querySelectorAll("article.hd-policy")].find(
        (a) => a.querySelector(".hd-policy__name")?.textContent?.trim() === name,
      );
      const target = [...(card?.querySelectorAll<HTMLButtonElement>("button") ?? [])].find(
        (b) => b.textContent?.trim() === wanted || b.getAttribute("aria-label") === wanted,
      );
      target?.click();
      return Boolean(target);
    },
    policyName,
    button,
  );
  if (!clicked) throw new Error(`no "${button}" button on the ${policyName} card`);
  await browser.pause(300);
}
