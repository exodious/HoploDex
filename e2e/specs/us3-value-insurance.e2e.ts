import { $, addFirearm, back, browser, choose, clickButton, expect, fill } from "../support/ui";
import { goTo, isButtonDisabled, openFirearm, policyCardText, selectOption } from "../support/ui";
import { titleBlock, toggle } from "../support/ui";

/**
 * End-to-end coverage of User Story 3's acceptance scenarios (spec.md),
 * driven against the real built app via tauri-driver / WebKitWebDriver.
 * Coverage is implicit: a firearm is scheduled on a policy with its own
 * amount, or left unscheduled and covered by the one blanket policy in force
 * (FR-036). Seeds its own uniquely-named policies and firearms rather than
 * assuming a clean slate; the scenarios build on each other in order. See
 * e2e/support/ui.ts for why interactions go through page JS.
 */

/** A local calendar date `days` from today, as YYYY-MM-DD. */
function isoDaysFromNow(days: number): string {
  const d = new Date();
  d.setDate(d.getDate() + days);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

interface NewPolicy {
  name: string;
  policyNumber: string;
  /** Blank for a schedule-only policy; a limit makes it a blanket policy. */
  blanketLimitDollars?: string;
  startDate: string;
  endDate: string;
}

/** Fills and submits the Add policy dialog. */
async function submitPolicy(opts: NewPolicy) {
  await goTo("Insurance");
  await clickButton("Add policy");
  await fill("Policy name", opts.name);
  await fill("Policy number", opts.policyNumber);
  await fill("Insurance company", "Acme Insurance");
  if (opts.blanketLimitDollars) await fill("Blanket coverage limit", opts.blanketLimitDollars);
  await fill("Coverage starts", opts.startDate);
  await fill("Coverage ends", opts.endDate);
  await clickButton("Add policy");
}

async function addPolicy(opts: NewPolicy) {
  await submitPolicy(opts);
  await $(`article.hd-policy*=${opts.name}`).waitForExist();
  await goTo("Collection");
}

/** Edits the named policy's fields from its card on the Insurance page. */
async function editPolicy(name: string, fields: Record<string, string>) {
  await goTo("Insurance");
  await clickInPolicyCard(name, "Edit");
  for (const [label, value] of Object.entries(fields)) await fill(label, value);
  await clickButton("Save changes");
  await $('[role="dialog"]').waitForExist({ reverse: true });
  await browser.pause(400);
}

async function addFirearmWithValue(opts: {
  make: string;
  model: string;
  serial: string;
  valueDollars: string;
}) {
  await addFirearm({ ...opts, caliber: "9mm", type: "Handgun" });
}

/** Schedules the open record's firearm on a policy, or (with `null`) leaves it
 * unscheduled, from the record's Insurance panel. */
async function assignCoverage(opts: { policyName: string | null; amountDollars?: string }) {
  await clickButton((await $("button=Change").isExisting()) ? "Change" : "Assign");
  await selectOption("Policy", opts.policyName ?? "Not scheduled");
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

async function openCoverage(name: string) {
  await goTo("Collection");
  await openFirearm(name);
}

describe("User Story 3 - Track Value and Insurance Coverage", () => {
  it("flags an unscheduled firearm as uninsured while no blanket policy is in force (Scenarios 1, 13)", async () => {
    await addFirearmWithValue({
      make: "InsE2EGlock",
      model: "19",
      serial: "INS-U-1",
      valueDollars: "500.00",
    });

    expect(await titleBlock("Coverage")).toBe("Uninsured");
    await expect($(".hd-coverage*=No blanket policy is in force")).toExist();

    await goTo("Insurance");
    const gaps = await $("section*=Not covered").getText();
    expect(gaps).toContain("InsE2EGlock 19");
  });

  it("covers it as soon as a blanket policy is entered, without editing the firearm (Scenarios 11, 13)", async () => {
    await addPolicy({
      name: "InsE2E Blanket A",
      policyNumber: "A-1",
      blanketLimitDollars: "100,000.00",
      startDate: "2020-01-01",
      endDate: isoDaysFromNow(365),
    });

    await openCoverage("InsE2EGlock 19");
    expect(await titleBlock("Coverage")).toBe("Covered");
    await expect($(".hd-coverage*=Covered by InsE2E Blanket A")).toExist();

    // A firearm added afterwards needs no assignment step either.
    await back();
    await addFirearmWithValue({
      make: "InsE2ESig",
      model: "P226",
      serial: "INS-B-1",
      valueDollars: "500.00",
    });
    expect(await titleBlock("Coverage")).toBe("Covered");
  });

  it("flags a scheduled firearm under-insured, then clears once it's scheduled sufficiently (Scenarios 2-3)", async () => {
    await addPolicy({
      name: "InsE2E Rider",
      policyNumber: "R-1",
      startDate: "2020-01-01",
      endDate: isoDaysFromNow(365),
    });
    await openCoverage("InsE2EGlock 19");
    await assignCoverage({ policyName: "InsE2E Rider", amountDollars: "300.00" });

    expect(await titleBlock("Coverage")).toBe("Under-insured");
    await expect($(".hd-coverage*=$200 short of its value")).toExist();
    await goTo("Insurance");
    expect(await policyCardText("InsE2E Rider")).toContain("$200 short");

    await openCoverage("InsE2EGlock 19");
    await assignCoverage({ policyName: "InsE2E Rider", amountDollars: "600.00" });

    expect(await titleBlock("Coverage")).toBe("Covered");
    await goTo("Insurance");
    expect(await policyCardText("InsE2E Rider")).not.toContain("short");
  });

  it("flags every unscheduled firearm when their combined value exceeds the blanket limit (Scenario 4)", async () => {
    await goTo("Collection");
    await addFirearmWithValue({
      make: "InsE2EMossberg",
      model: "500",
      serial: "INS-B-2",
      valueDollars: "500.00",
    });
    expect(await titleBlock("Coverage")).toBe("Covered");

    // Sig and Mossberg are unscheduled and worth $1,000 together.
    await editPolicy("InsE2E Blanket A", { "Blanket coverage limit": "800.00" });

    await openCoverage("InsE2EMossberg 500");
    expect(await titleBlock("Coverage")).toBe("Under-insured");
    await goTo("Insurance");
    const card = await policyCardText("InsE2E Blanket A");
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

  it("breaks the value summary down into the blanket policy, scheduled firearms and uninsured (Scenario 6)", async () => {
    await goTo("Insurance");
    await expect($(".hd-page-sub*=estimated replacement value")).toExist();

    // Section labels are upper-cased by CSS, and getText() returns what's rendered.
    const blanket = (await policyCardText("InsE2E Blanket A")).toLowerCase();
    expect(blanket).toContain("blanket coverage");
    expect(blanket).toContain("covers every firearm not scheduled individually");

    const rider = (await policyCardText("InsE2E Rider")).toLowerCase();
    expect(rider).toContain("scheduled individually");
    expect(rider).toContain("inse2eglock 19");
  });

  it("blocks a blanket policy overlapping another, naming it, but accepts a shared boundary day (Scenario 12)", async () => {
    await submitPolicy({
      name: "InsE2E Blanket Overlap",
      policyNumber: "O-1",
      blanketLimitDollars: "50,000.00",
      startDate: isoDaysFromNow(300),
      endDate: isoDaysFromNow(700),
    });
    await expect($('[role="dialog"]*=InsE2E Blanket A')).toExist();
    await clickButton("Cancel");
    await expect($("article.hd-policy*=InsE2E Blanket Overlap")).not.toExist();

    // Starting on the day the other ends is fine; the later start is in force.
    await addPolicy({
      name: "InsE2E Blanket Next",
      policyNumber: "N-1",
      blanketLimitDollars: "50,000.00",
      startDate: isoDaysFromNow(365),
      endDate: isoDaysFromNow(730),
    });
    await goTo("Insurance");
    await expect($("article.hd-policy*=InsE2E Blanket Next")).toExist();
  });

  it("flags a policy expiring within 30 days, unless a successor blanket policy takes over (Scenarios 7, 14)", async () => {
    await editPolicy("InsE2E Blanket A", { "Coverage ends": isoDaysFromNow(15) });

    await goTo("Insurance");
    expect(await policyCardText("InsE2E Blanket A")).toContain("Expires in 15 days");
    await goTo("Collection");
    await expect($(".hd-attention*=InsE2E Blanket A")).toExist();

    // The successor now starts the day this one ends: no gap, no warning.
    await editPolicy("InsE2E Blanket Next", { "Coverage starts": isoDaysFromNow(15) });

    await goTo("Insurance");
    expect(await policyCardText("InsE2E Blanket A")).not.toContain("Expires in");
    await goTo("Collection");
    const banner = (await $(".hd-attention").isExisting())
      ? await $(".hd-attention").getText()
      : "";
    expect(banner).not.toContain("InsE2E Blanket A");
  });

  it("flags firearms on an expired policy as uninsured (Scenario 8)", async () => {
    await addPolicy({
      name: "InsE2E Rider Old",
      policyNumber: "RO-1",
      startDate: "2020-01-01",
      endDate: isoDaysFromNow(-5),
    });
    await addFirearmWithValue({
      make: "InsE2EWinchester",
      model: "94",
      serial: "INS-EXP-2",
      valueDollars: "100.00",
    });
    await assignCoverage({ policyName: "InsE2E Rider Old", amountDollars: "100.00" });

    expect(await titleBlock("Coverage")).toBe("Uninsured");
    await expect($(".hd-coverage*=expired")).toExist();

    await goTo("Insurance");
    const card = await policyCardText("InsE2E Rider Old");
    expect(card).toContain("Expired");
    expect(card).toContain("Uninsured");
  });

  it("treats a renewed policy's firearms as insured again (Edge Case: renewal)", async () => {
    await editPolicy("InsE2E Rider Old", { "Coverage ends": isoDaysFromNow(200) });

    const card = await policyCardText("InsE2E Rider Old");
    expect(card).toContain("In force");
    expect(card).not.toContain("Uninsured");
    expect(card).toContain("Covered");
  });

  it("moves the scheduled firearms to another policy when their policy is deleted (Scenario 9)", async () => {
    await goTo("Insurance");
    await clickInPolicyCard("InsE2E Rider", "Delete InsE2E Rider");
    await expect($('[role="alertdialog"]*=InsE2EGlock 19')).toExist();
    // Nothing can be deleted until the firearms have been dealt with.
    expect(await isButtonDisabled("Delete policy")).toBe(true);

    await choose("Move to another policy");
    await selectOption("Move to", "InsE2E Rider Old");
    await expect($('[role="alertdialog"]*=actually covers these firearms')).toExist();
    await clickButton("Delete policy");
    await waitForPolicyGone("InsE2E Rider");

    // The firearm kept its $600 scheduled amount, now on the other policy.
    expect(await policyCardText("InsE2E Rider Old")).toContain("InsE2EGlock 19");
    await openCoverage("InsE2EGlock 19");
    expect(await titleBlock("Coverage")).toBe("Covered");
  });

  it("lets an expired policy's firearms be left unscheduled with only a warning (Scenario 10)", async () => {
    await addPolicy({
      name: "InsE2E Rider Lapsed",
      policyNumber: "RL-1",
      startDate: "2020-01-01",
      endDate: isoDaysFromNow(-10),
    });
    await addFirearmWithValue({
      make: "InsE2ERemington",
      model: "870",
      serial: "INS-LAPSED-1",
      valueDollars: "100.00",
    });
    await assignCoverage({ policyName: "InsE2E Rider Lapsed", amountDollars: "100.00" });
    expect(await titleBlock("Coverage")).toBe("Uninsured");

    await goTo("Insurance");
    await clickInPolicyCard("InsE2E Rider Lapsed", "Delete InsE2E Rider Lapsed");
    await expect($('[role="alertdialog"]*=already treated as uninsured')).toExist();
    await choose("Leave unscheduled");
    // No stronger confirmation: they were uninsured already.
    await expect($('[role="alertdialog"] [role="checkbox"]')).not.toExist();
    await clickButton("Delete policy");
    await waitForPolicyGone("InsE2E Rider Lapsed");

    // Unscheduled, so the blanket policy in force covers it. Its $800 limit is
    // already exceeded by the other unscheduled firearms, which is why this
    // reads under-insured rather than covered.
    await openCoverage("InsE2ERemington 870");
    await expect($(".hd-coverage*=InsE2E Blanket A")).toExist();
    expect(await titleBlock("Coverage")).toBe("Under-insured");
  });

  it("needs the stronger confirmation to leave a current policy's firearms unscheduled (Scenario 9)", async () => {
    await goTo("Insurance");
    await clickInPolicyCard("InsE2E Rider Old", "Delete InsE2E Rider Old");
    await choose("Leave unscheduled");
    expect(await isButtonDisabled("Delete policy")).toBe(true);

    await toggle("2 firearms will lose their scheduled coverage");
    await clickButton("Delete policy");
    await waitForPolicyGone("InsE2E Rider Old");
  });

  it("warns how many firearms lose coverage when the blanket policy in force is deleted (Scenario 15)", async () => {
    await goTo("Insurance");
    await clickInPolicyCard("InsE2E Blanket A", "Delete InsE2E Blanket A");
    await expect($('[role="alertdialog"]*=lose its blanket coverage')).toExist();
    await clickButton("Delete policy");
    await waitForPolicyGone("InsE2E Blanket A");

    // Nothing else is in force today (the successor starts later), so an
    // unscheduled firearm is uninsured again.
    await openCoverage("InsE2ESig P226");
    expect(await titleBlock("Coverage")).toBe("Uninsured");
  });
});

/** Waits for the policy card with exactly this name to leave the page.
 * (Substring selectors can't tell "InsE2E Rider" from "InsE2E Rider Old".) */
async function waitForPolicyGone(name: string) {
  await browser.waitUntil(
    () =>
      browser.execute(
        (wanted: string) =>
          ![...document.querySelectorAll(".hd-policy__name")].some(
            (n) => n.textContent?.trim() === wanted,
          ),
        name,
      ),
    { timeoutMsg: `the deleted policy "${name}" is still listed` },
  );
}

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
