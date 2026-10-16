import {
  $,
  addFirearm,
  back,
  browser,
  choose,
  clickButton,
  expect,
  fill,
  settle,
} from "../support/ui";
import { goTo, isButtonDisabled, openFirearm, policyCardText, selectOption } from "../support/ui";
import { titleBlock } from "../support/ui";
import { createDatabase } from "../support/ui";

/**
 * End-to-end coverage of 001 US3 (001 spec.md), driven against the real
 * built app through its embedded WebDriver server: one journey through
 * the story's scenarios, plus the policy form's field error. Coverage is
 * implicit: a firearm is scheduled on a policy with its own amount, or left
 * unscheduled and covered by the one blanket policy in force (001 FR-036). The
 * rules each scenario states (the over-limit total, expiry warnings, the
 * deletion choices, notes, Escape) are tested in Rust and Vitest, citing
 * their scenario; this spec keeps what needs the real app. See
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
  await settle();
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

/** The error shown for the field labelled `label` in the open dialog: the
 * text of the element its `aria-describedby` names when it is `aria-invalid`,
 * or `null` when the field shows none. */
async function dialogFieldError(label: string): Promise<string | null> {
  return browser.execute((label: string) => {
    const labelEl = [...document.querySelectorAll('[role="dialog"] label')].find(
      (l) => l.textContent?.trim() === label,
    ) as HTMLLabelElement | undefined;
    const field = labelEl && document.getElementById(labelEl.htmlFor);
    if (!field || field.getAttribute("aria-invalid") !== "true") return null;
    for (const id of (field.getAttribute("aria-describedby") ?? "").split(" ")) {
      const el = document.getElementById(id);
      if (el?.classList.contains("hd-field__error")) return el.textContent?.trim() ?? "";
    }
    return null;
  }, label);
}

describe("001 US3 - Track Value and Insurance Coverage (was us3-value-insurance)", () => {
  // Each spec's session starts at the chooser with no databases (wdio.conf.ts).
  before(async () => {
    await createDatabase();
  });

  it("flags, covers and schedules firearms, keeps the total current, and moves them when their policy is deleted (001 US3-1 to US3-3, US3-5, US3-9, US3-11 and US3-13)", async () => {
    // 001 US3-1, US3-13: no blanket policy in force, so an unscheduled firearm is uninsured.
    await addFirearmWithValue({
      make: "InsE2EGlock",
      model: "19",
      serial: "INS-U-1",
      valueDollars: "500",
    });

    expect(await titleBlock("Coverage")).toBe("Uninsured");
    await expect($(".hd-coverage*=No blanket policy is in force")).toExist();

    await goTo("Insurance");
    const gaps = await $("section*=Not covered").getText();
    expect(gaps).toContain("InsE2EGlock 19");

    // 001 US3-11, US3-13: a blanket policy covers it at once, without editing the firearm.
    await addPolicy({
      name: "InsE2E Blanket A",
      policyNumber: "A-1",
      blanketLimitDollars: "100000",
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
      valueDollars: "500",
    });
    expect(await titleBlock("Coverage")).toBe("Covered");

    // 001 US3-2 to US3-3: scheduled below its value it is under-insured, then clears.
    await addPolicy({
      name: "InsE2E Rider",
      policyNumber: "R-1",
      startDate: "2020-01-01",
      endDate: isoDaysFromNow(365),
    });
    await openCoverage("InsE2EGlock 19");
    await assignCoverage({ policyName: "InsE2E Rider", amountDollars: "300" });

    expect(await titleBlock("Coverage")).toBe("Under-insured");
    await expect($(".hd-coverage*=$200 short of its value")).toExist();
    await goTo("Insurance");
    expect(await policyCardText("InsE2E Rider")).toContain("$200 short");

    await openCoverage("InsE2EGlock 19");
    await assignCoverage({ policyName: "InsE2E Rider", amountDollars: "600" });

    expect(await titleBlock("Coverage")).toBe("Covered");
    await goTo("Insurance");
    expect(await policyCardText("InsE2E Rider")).not.toContain("short");

    // 001 US3-5: the collection total follows an add, an edit and a disposal.
    const before = await collectionTotal();

    await addFirearmWithValue({
      make: "InsE2ERuger",
      model: "10-22",
      serial: "INS-T-1",
      valueDollars: "200",
    });
    expect(await collectionTotal()).toBeCloseTo(before + 200, 2);

    await openFirearm("InsE2ERuger 10-22");
    await clickButton("Edit");
    await fill("Estimated replacement value", "300");
    await clickButton("Save changes");
    expect(await collectionTotal()).toBeCloseTo(before + 300, 2);

    await openFirearm("InsE2ERuger 10-22");
    await clickButton("Mark disposed");
    await choose("Sold");
    await fill("Transferred to", "Jane Doe");
    await fill("Date", "2025-01-01");
    await fill("Price received", "250");
    await clickButton("Mark as disposed");
    expect(await collectionTotal()).toBeCloseTo(before, 2);

    // 001 US3-9: deleting the Rider moves its firearm to another policy, which
    // the Insurance page and the firearm's record both show afterwards.
    await addPolicy({
      name: "InsE2E Rider Two",
      policyNumber: "R-2",
      startDate: "2020-01-01",
      endDate: isoDaysFromNow(200),
    });
    await goTo("Insurance");
    await expect($(".hd-page-sub*=estimated replacement value")).toExist();
    await clickInPolicyCard("InsE2E Rider", "Delete InsE2E Rider");
    await expect($('[role="alertdialog"]*=InsE2EGlock 19')).toExist();
    // Nothing can be deleted until the firearm has been dealt with.
    expect(await isButtonDisabled("Delete policy")).toBe(true);

    await choose("Move to another policy");
    await selectOption("Move to", "InsE2E Rider Two");
    await expect($('[role="alertdialog"]*=actually covers this firearm.')).toExist();
    await clickButton("Delete policy");
    await waitForPolicyGone("InsE2E Rider");

    // The firearm kept its $600 scheduled amount, now on the other policy.
    expect(await policyCardText("InsE2E Rider Two")).toContain("InsE2EGlock 19");
    await openCoverage("InsE2EGlock 19");
    expect(await titleBlock("Coverage")).toBe("Covered");
    await expect($(".hd-coverage*=$600")).toExist();
  });

  // InsurancePolicyForm's field error: the backend refuses an overlap and the
  // message lands on the Coverage starts field.
  it("blocks a blanket policy overlapping another, naming it on the Coverage starts field (001 US3-12)", async () => {
    // Years away from the journey's policies, so the two never overlap.
    await addPolicy({
      name: "InsE2E Overlap Base",
      policyNumber: "OB-1",
      blanketLimitDollars: "50000",
      startDate: "2010-01-01",
      endDate: "2011-01-01",
    });

    await submitPolicy({
      name: "InsE2E Overlap New",
      policyNumber: "ON-1",
      blanketLimitDollars: "50000",
      startDate: "2010-06-01",
      endDate: "2012-01-01",
    });
    await $('[role="dialog"]*=InsE2E Overlap Base').waitForExist();
    expect(await dialogFieldError("Coverage starts")).toContain("InsE2E Overlap Base");
    await clickButton("Cancel");
    await $('[role="dialog"]').waitForExist({ reverse: true });
    await expect($("article.hd-policy*=InsE2E Overlap New")).not.toExist();
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
  await settle();
}
