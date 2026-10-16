import { $, $$, browser, expect, toggle } from "../support/ui";
import {
  TASK_LIMIT_MS,
  backToCollectionByKeys,
  chooseSuggestion,
  createAccessory,
  mountedSectionShows,
  mountedSectionText,
  openFirearmByKeys,
  openMountMenu,
  searchMountDialog,
  seedMountedRecords,
  tabTo,
  tabToControl,
  typeReal,
} from "../support/accessories";
import { realClick, realKey, skipWithoutRealInput } from "../support/realInput";

/**
 * End-to-end coverage of specs/006-accessory-links' User Story 3 (US3-1,
 * US3-2): the Rifle, carrying an Optic and the AR (which carries the
 * Suppressor), is marked disposed from the keyboard with the Optic disposed
 * with it. The Rifle, the Optic on it and the Suppressor on the AR are what the
 * Mounting story (`us12-accessories-mounting`) left, and the unmounted Sling is
 * what the Browsing story (`us12-accessories-browse`) added; here the
 * `before` hook makes them through the backend (see `support/accessories.ts`).
 * Real keys, as in those stories.
 */

/** The name of the radio group the focused radio button is in (a group's
 * label, or what labels it), or null when focus is elsewhere. */
const focusedRadioGroup = () =>
  browser.execute(() => {
    const active = document.activeElement as HTMLElement | null;
    if (active?.getAttribute("type") !== "radio") return null;
    const group = active.closest('[role="radiogroup"]');
    const id = group?.getAttribute("aria-labelledby");
    return id ? (document.getElementById(id)?.textContent ?? "").trim() : null;
  });

/** Presses Tab until a radio button in the group named `group` has focus. */
async function tabToRadioGroup(group: string) {
  for (let step = 0; step < 30; step++) {
    if ((await focusedRadioGroup())?.startsWith(group)) return;
    await realKey("Tab");
  }
  throw new Error(`Tab never reached the "${group}" choice`);
}

/** The text of the page's title block, spaces collapsed. */
const titleBlockText = () =>
  browser.execute(() =>
    (document.querySelector(".hd-titleblock")?.textContent ?? "").replace(/\s+/g, " "),
  );

describe("006 US3 - Dispose and Delete With Items Mounted (was us12-accessories-dispose)", () => {
  before(function () {
    skipWithoutRealInput(this);
  });
  before(async () => {
    await seedMountedRecords();
    // The Sling the Browsing story added (the only accessory not mounted).
    await createAccessory({
      kind: "Sling",
      make: "Vickers",
      model: "Two Point",
      estimatedValue: 40,
    });
  });

  it("marks the Rifle disposed with its Optic, from the keyboard, and leaves the rest unmounted (US3-1, US3-2)", async () => {
    // Setup, not timed: the Rifle carries the Optic already; mount the AR on
    // it too, so that the AR (with the Suppressor on it) can be kept.
    await backToCollectionByKeys();
    await openFirearmByKeys("LaRue PredatAR");
    await openMountMenu(1);
    await searchMountDialog("ddm");
    await chooseSuggestion("Daniel Defense DDM4");
    await mountedSectionShows("Daniel Defense DDM4");
    await mountedSectionShows("SilencerCo Omega 300");
    expect(await mountedSectionText()).toContain("Nikon P3 · Optic");

    const started = Date.now();
    await realClick("#record-name");
    await tabToControl("Mark disposed");
    await realKey("Return");
    await $('[role="dialog"]').waitForDisplayed({ timeout: 5000 });

    // The Rifle is not mounted, so no unmount note; the Mounted group lists
    // all three, each defaulting to Keep.
    const dialogText = (await $('[role="dialog"]').getText()).replace(/\s+/g, " ");
    expect(dialogText).not.toContain("will be unmounted from");
    expect(dialogText).toContain("Nikon P3 · Optic");
    expect(dialogText).toContain(
      "Kept firearms and accessories mounted on LaRue PredatAR will be unmounted.",
    );

    await tabToRadioGroup("What happened");
    await realKey("space");
    await tabTo("Transferred to");
    await typeReal("Jane Doe");
    await tabTo("Price received");
    await typeReal("2500");

    // The Optic's row: arrow to Dispose with it, then its price.
    await tabToRadioGroup("Nikon P3 · Optic");
    await realKey("Right");
    await tabTo("Price for Nikon P3 · Optic");
    await typeReal("150");
    // Enter in the last field confirms.
    await realKey("Return");

    await browser.waitUntil(async () => (await titleBlockText()).includes("Sold"), {
      timeout: 8000,
      timeoutMsg: "the Rifle never showed as sold",
    });
    expect(Date.now() - started).toBeLessThan(TASK_LIMIT_MS);

    // The Rifle's page has no Mounted section any more.
    expect(await mountedSectionText()).toBe("");

    // The Optic is disposed, with the Rifle's recipient and its own price.
    await browser.execute(() => {
      [...document.querySelectorAll<HTMLElement>(".hd-tab")]
        .find((t) => t.textContent?.trim().startsWith("Accessories"))
        ?.click();
    });
    // The Optic is disposed, so the page lists only the Sling until disposed
    // accessories are included.
    await browser.waitUntil(async () => (await $$(".hd-row").length) === 1, {
      timeout: 8000,
      timeoutMsg: "the Accessories page never listed only the Sling",
    });
    expect(await $$(".hd-row--disposed").length).toBe(0);
    await toggle("Show disposed");
    await browser.waitUntil(
      async () =>
        (await browser.execute(() => document.querySelectorAll(".hd-row--disposed").length)) === 1,
      { timeout: 8000, timeoutMsg: "the Optic never showed as disposed" },
    );
    expect(await $(".hd-row--disposed").getText()).toContain("Nikon P3");

    // The AR was kept: unmounted from the Rifle, still carrying the Suppressor.
    await browser.execute(() => {
      [...document.querySelectorAll<HTMLElement>(".hd-tab")]
        .find((t) => t.textContent?.trim().startsWith("Collection"))
        ?.click();
    });
    await $$(".hd-row__name")[0].waitForExist({ timeout: 8000 });
    await openFirearmByKeys("Daniel Defense DDM4");
    expect(await $(".hd-plate__mounted").isExisting()).toBe(false);
    expect(await mountedSectionText()).toContain("SilencerCo Omega 300");
    expect(await mountedSectionText()).not.toContain("Nikon P3");
  });
});
