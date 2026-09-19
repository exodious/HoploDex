import {
  $,
  addFirearm,
  back,
  browser,
  choose,
  clickButton,
  clickEl,
  expect,
  fill,
} from "../support/ui";
import { goTo, openFirearm, selectOption } from "../support/ui";

/**
 * Regression coverage for the review of the UI redesign (PR #1): what the
 * under-insured figure means, the Insurance tab's count, linking between a
 * firearm and its policy, and the color-mode control. Seeds its own
 * uniquely-named policy and firearms. See e2e/support/ui.ts for why
 * interactions go through page JS.
 */

function isoDaysFromNow(days: number): string {
  const d = new Date();
  d.setDate(d.getDate() + days);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

async function assignCoverage(
  policyName: string,
  kind: "Scheduled individually" | "Blanket",
  amountDollars?: string,
) {
  await clickButton((await $("button=Change").isExisting()) ? "Change" : "Assign");
  await selectOption("Policy", policyName);
  await choose(kind);
  if (amountDollars) await fill("Scheduled amount", amountDollars);
  await clickButton("Save coverage");
  await $('[role="dialog"]').waitForExist({ reverse: true });
  await browser.pause(400);
}

/** The number on the Insurance tab, or 0 when it shows none. */
async function insuranceTabCount(): Promise<number> {
  const badge = await $(".hd-tab__alert");
  if (!(await badge.isExisting())) return 0;
  return Number((await badge.getText()).replace(/\D/g, ""));
}

async function themeAttribute(): Promise<string | null> {
  return browser.execute(() => document.documentElement.getAttribute("data-theme"));
}

async function chooseTheme(label: string) {
  await browser.execute((title: string) => {
    document.querySelector<HTMLElement>(`.hd-topbar label[title="${title}"]`)?.click();
  }, label);
  await browser.pause(300);
}

describe("UI review follow-ups", () => {
  before(async () => {
    await goTo("Insurance");
    await clickButton("Add policy");
    await fill("Policy name", "InsE2E Review Policy");
    await fill("Policy number", "REV-1");
    await fill("Insurance company", "Acme Insurance");
    await fill("Blanket coverage limit", "1,000.00");
    await fill("Coverage starts", "2020-01-01");
    await fill("Coverage ends", isoDaysFromNow(365));
    await clickButton("Add policy");
    await $("article.hd-policy*=InsE2E Review Policy").waitForExist();
    await goTo("Collection");

    // Blanket-covered, $4,000 over the policy's limit.
    await addFirearm({
      make: "InsE2EReview",
      model: "Rifle",
      caliber: ".308",
      type: "Rifle",
      serial: "REV-R",
      valueDollars: "5,000.00",
    });
    await assignCoverage("InsE2E Review Policy", "Blanket");
    await back();

    // Scheduled for $1,500 of a $2,000 value: $500 short.
    await addFirearm({
      make: "InsE2EReview",
      model: "Pistol",
      caliber: "9mm",
      type: "Handgun",
      serial: "REV-P",
      valueDollars: "2,000.00",
    });
    await assignCoverage("InsE2E Review Policy", "Scheduled individually", "1,500.00");
    await back();

    // No policy at all: uninsured.
    await addFirearm({
      make: "InsE2EReview",
      model: "Pump",
      caliber: "12 gauge",
      type: "Shotgun",
      serial: "REV-S",
      valueDollars: "600.00",
    });
    await back();
  });

  it("shows how much coverage is missing for under-insured firearms, not their value", async () => {
    await goTo("Insurance");
    // The legend's parts are separate elements, so getText() runs them together.
    const legend = (await $(".hd-overview__legend").getText()).replace(/\s+/g, "");
    expect(legend).toContain("Under-insured$4,500short");
    expect(legend).toContain("Uninsured$600");
  });

  it("counts only firearms on the Insurance tab, and recounts after a delete", async () => {
    // Two under-insured and one uninsured firearm — the policy isn't counted.
    expect(await insuranceTabCount()).toBe(3);

    await goTo("Collection");
    await openFirearm("InsE2EReview Pump");
    await clickButton("Delete");
    await clickButton("Delete firearm");
    await $("#record-name").waitForExist({ reverse: true });
    await browser.pause(400);

    expect(await insuranceTabCount()).toBe(2);
  });

  it("links from a firearm to its own policy page and back, and from the policy to a firearm and back", async () => {
    await goTo("Collection");
    await openFirearm("InsE2EReview Rifle");

    // The policy link opens that one policy, not the Insurance listing.
    await clickEl(".hd-facts .hd-link");
    await $(".hd-policy").waitForExist({ timeout: 4000 });
    await expect($(".hd-policy__name")).toHaveText("InsE2E Review Policy");
    expect(await $$(".hd-policy").length).toBe(1);
    await expect($(".hd-page-title")).not.toExist();
    await expect($(".hd-backlink")).toHaveText(expect.stringContaining("InsE2EReview Rifle"));

    await back();
    await $("#record-name").waitForExist();
    expect(await $("#record-name").getText()).toBe("InsE2EReview Rifle");

    // And the other way: policy -> firearm -> back lands on the policy again.
    await clickEl(".hd-facts .hd-link");
    await $(".hd-policy").waitForExist({ timeout: 4000 });
    await browser.execute(() => {
      [...document.querySelectorAll<HTMLElement>(".hd-policy .hd-link")]
        .find((link) => link.textContent?.includes("InsE2EReview Pistol"))
        ?.click();
    });
    await $("#record-name").waitForExist();
    await expect($(".hd-backlink")).toHaveText(expect.stringContaining("InsE2E Review Policy"));

    await back();
    await $(".hd-policy").waitForExist({ timeout: 4000 });
    await expect($(".hd-policy__name")).toHaveText("InsE2E Review Policy");
    await expect($(".hd-page-title")).not.toExist();
  });

  it("opens a policy's own page from the Insurance list, and goes back to the list", async () => {
    await goTo("Insurance");
    await expect($(".hd-page-title")).toHaveText("Insurance");

    await clickEl(".hd-policy__open");
    await $(".hd-policy").waitForExist({ timeout: 4000 });
    await expect($(".hd-policy__name")).toHaveText("InsE2E Review Policy");
    expect(await $$(".hd-policy").length).toBe(1);
    // On its own page the name is the heading, not a link to itself.
    await expect($(".hd-policy__open")).not.toExist();
    await expect($(".hd-backlink")).toHaveText(expect.stringContaining("Insurance"));

    await back();
    await expect($(".hd-page-title")).toHaveText("Insurance");
  });

  it("switches between light, dark, and auto color modes", async () => {
    await goTo("Collection");
    expect(await themeAttribute()).toBeNull();

    await chooseTheme("Dark");
    expect(await themeAttribute()).toBe("dark");

    await chooseTheme("Light");
    expect(await themeAttribute()).toBe("light");

    await chooseTheme("Auto (match system)");
    expect(await themeAttribute()).toBeNull();
  });
});
