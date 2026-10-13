import { $, $$, browser, expect } from "../support/ui";
import {
  TASK_LIMIT_MS,
  chooseKind,
  openFirearmByKeys,
  seedMountedRecords,
  shiftTabToControl,
  tabTo,
  tabToControl,
  typeReal,
} from "../support/accessories";
import { realClick, realKey, skipWithoutRealInput } from "../support/realInput";

/**
 * End-to-end coverage of specs/006-accessory-links' User Story 4 (US4-2):
 * the Accessories page, with an unmounted Sling added from the keyboard, is
 * grouped by Mounted on through the grouping menu, and shows the Rifle's group
 * and "Not mounted" last. The Rifle's group is the Optic mounted on it, which
 * the Mounting story (`us12-accessories-mounting`) made from the keyboard; here
 * `seedMountedRecords` makes it through the backend, with the rest of what that
 * story left (see `support/accessories.ts`). Real keys, as in that story.
 */

/** The group headings of the Accessories page, spaces collapsed. */
const groupHeadings = () =>
  browser.execute(() =>
    [...document.querySelectorAll<HTMLElement>(".hd-group__title")].map((h) =>
      (h.textContent ?? "").replace(/\s+/g, " ").trim(),
    ),
  );

describe("User Story 4 - Browsing accessories by what they are mounted on (specs/006-accessory-links)", () => {
  before(function () {
    skipWithoutRealInput(this);
  });
  before(async () => {
    await seedMountedRecords();
  });
  it("groups the Accessories page by Mounted on from the keyboard: the Rifle's group, then Not mounted (US4-2)", async () => {
    // Setup, not timed: an unmounted Sling, added from the Accessories page.
    await realClick("#record-name");
    await shiftTabToControl("Accessories");
    await realKey("Return");
    await $("h1=Accessories").waitForExist({ timeout: 8000 });
    await tabToControl("Add accessory");
    await realKey("Return");
    await $('[role="dialog"]').waitForDisplayed({ timeout: 5000 });
    await tabTo("Kind");
    await chooseKind("Sling");
    await tabTo("Make");
    await typeReal("Vickers");
    await tabTo("Model");
    await typeReal("Two Point");
    await tabTo("Estimated value");
    await typeReal("40");
    await realKey("Return");
    await browser.waitUntil(async () => !(await $('[role="dialog"]').isExisting()), {
      timeout: 8000,
      timeoutMsg: "the Add accessory dialog never closed",
    });
    // Saving opens the Sling's record; its Back link returns to the list.
    await $("#record-name").waitForExist({ timeout: 8000 });
    await realClick("#record-name");
    await shiftTabToControl("Accessories");
    await realKey("Return");
    await $("h1=Accessories").waitForExist({ timeout: 8000 });

    const started = Date.now();
    // Real focus is on the page already (the dialog's keys); Tab to the
    // grouping menu and choose Mounted on.
    await tabToControl("Group by");
    await realKey("Return");
    await $('[role="menu"]').waitForDisplayed({ timeout: 5000 });
    for (let step = 0; step < 8; step++) {
      const item = await browser.execute(() => {
        const active = document.activeElement as HTMLElement | null;
        return active?.getAttribute("role") === "menuitemradio"
          ? (active.textContent ?? "").trim()
          : null;
      });
      if (item === "Mounted on") break;
      await realKey("Down");
    }
    await realKey("Return");

    await browser.waitUntil(async () => (await groupHeadings()).length === 2, {
      timeout: 8000,
      timeoutMsg: "the Accessories page never showed its two groups",
    });
    const headings = await groupHeadings();
    expect(headings[0]).toContain("LaRue PredatAR");
    expect(headings[1]).toContain("Not mounted");
    expect(Date.now() - started).toBeLessThan(TASK_LIMIT_MS);

    // Back to the AR's page for the next story, which starts on a record.
    await shiftTabToControl("Collection");
    await realKey("Return");
    await $$(".hd-row__name")[0].waitForExist({ timeout: 8000 });
    await openFirearmByKeys("Daniel Defense DDM4");
  });
});
