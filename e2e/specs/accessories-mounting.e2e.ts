import { budgetsChecked } from "../support/budgets";
import { $, browser, expect } from "../support/ui";
import {
  TASK_LIMIT_MS,
  chooseKind,
  chooseSuggestion,
  focusedValue,
  mountedSectionShows,
  mountedSectionText,
  openFirearmByKeys,
  openMountMenu,
  formSays,
  openMountedList,
  saveForm,
  seedFirearms,
  tabTo,
  typeReal,
  backToCollectionByKeys,
} from "../support/accessories";
import { realKey, skipWithoutRealInput } from "../support/realInput";

/**
 * End-to-end coverage of 006 SC-001, US2-1, US2-3a and US2-4, as 014 US5
 * changes them: from the keyboard alone, as real key events through the X
 * server, an accessory is added and mounted on a firearm from that firearm's
 * page, an existing firearm is added to the firearm's Mounted list in its edit
 * form and saved with it, and that firearm is moved to another the same way,
 * its row saying where it moves from. The suggestion list's, menu's and
 * dialogs' focus behaviour is WebKitGTK's to get right, and WebDriver's own
 * keys don't pass through GTK. See
 * docs/development/real-input.md.
 *
 * The records are made first, through the Add firearm dialog (not timed); a
 * real click on a page's heading then puts real focus on the page, and from
 * there the keys alone do the work. The other stories are in
 * `accessories-record`, `-browse` and `-dispose`; the helpers and the
 * seeds are in `support/accessories.ts`.
 */

describe("006 US2 - Mount an Item on a Firearm (was us12-accessories-mounting)", () => {
  before(function () {
    skipWithoutRealInput(this);
  });
  before(async () => {
    await seedFirearms();
  });

  it("adds an Optic from a Rifle's page, mounts a Suppressor on it, then moves the Suppressor to an AR, from the keyboard alone (006 SC-001, US2-1, US2-3a, US2-4; 014 US5-1 to US5-4, US5-7)", async () => {
    const started = Date.now();

    // Open the seeded Rifle and add an Optic mounted on it.
    await openFirearmByKeys("LaRue PredatAR");
    await openMountMenu(0);
    await $('[role="dialog"]').waitForDisplayed({ timeout: 5000 });
    // Still the Add accessory form, with Mounted on already set to the Rifle.
    expect(await $('[role="dialog"]').getText()).toContain("Add accessory");

    await tabTo("Kind");
    await chooseKind("Optic");

    await tabTo("Make");
    await typeReal("Nikon");
    await tabTo("Model");
    await typeReal("P3");
    await tabTo("Serial number");
    await typeReal("N1");
    await tabTo("Mounted on");
    expect(await focusedValue()).toContain("LaRue PredatAR");
    await tabTo("Estimated value");
    await typeReal("300");
    // Enter in the last field saves.
    await realKey("Return");

    await mountedSectionShows("Nikon P3 · Optic");

    // Mount the Suppressor, an existing firearm, on the Rifle: in the Rifle's
    // edit form, held until Save (014 US5).
    await openMountedList();
    await typeReal("omeg");
    await chooseSuggestion("SilencerCo Omega 300");
    await formSays("Will be mounted here, with everything mounted on it.");
    expect(await mountedSectionText()).not.toContain("SilencerCo Omega 300");
    await saveForm();
    await mountedSectionShows("SilencerCo Omega 300");
    expect(await mountedSectionText()).toContain("Nikon P3 · Optic");

    await backToCollectionByKeys();

    // Open the AR and mount the Suppressor on it. It is mounted on the Rifle,
    // so the list offers it with where it is, and its row says it moves.
    await openFirearmByKeys("Daniel Defense DDM4");
    await openMountedList();
    await typeReal("omeg");
    await $('[role="listbox"]').waitForDisplayed({ timeout: 5000 });
    // The text, not WebDriver's getText: WebKitGTK's leaves out the name and
    // the "Mounted on" line of a row (both ellipsised) that are plainly shown.
    await browser.waitUntil(
      async () =>
        (
          await browser.execute(() => document.querySelector('[role="listbox"]')?.textContent ?? "")
        ).includes("Mounted on LaRue PredatAR"),
      { timeout: 5000, timeoutMsg: "the Suppressor's row never said where it is mounted" },
    );
    await chooseSuggestion("SilencerCo Omega 300");
    await formSays(
      "Will be mounted here, moving from LaRue PredatAR with everything mounted on it.",
    );
    await saveForm();

    await mountedSectionShows("SilencerCo Omega 300");
    if (budgetsChecked()) expect(Date.now() - started).toBeLessThan(TASK_LIMIT_MS);
  });
});
