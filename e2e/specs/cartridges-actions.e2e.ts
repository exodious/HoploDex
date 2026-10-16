import {
  browser,
  choose,
  clickButton,
  createDatabase,
  expect,
  fieldValue,
  fill,
  focusedFieldLabel,
  settle,
} from "../support/ui";
import { realClick, realKey, skipWithoutRealInput } from "../support/realInput";

/**
 * End-to-end coverage of 004 US2-3 and US2-10: the Cartridge suggestion list
 * is driven with the keyboard alone, as real key events through the X server,
 * since the list's focus and highlight behaviour is WebKitGTK's to get right
 * and WebDriver's own keys don't pass through GTK. See "Real keyboard and
 * mouse input" in DEVELOPMENT.md.
 *
 * The list's contents and markers (004 US2-4) and grouping by Cartridge
 * (004 US1-6) are tested in Rust and Vitest (see the citations in those tests).
 */
describe("004 US2 - Pick From a Narrowing List While Typing (was us10-cartridges-actions)", () => {
  before(function () {
    skipWithoutRealInput(this);
  });
  // Each spec's session starts at the chooser with no databases (wdio.conf.ts).
  before(async () => {
    await createDatabase();
  });

  /** The text of the row `aria-activedescendant` points at, or null. */
  async function highlightedOption(): Promise<string | null> {
    return browser.execute(() => {
      const active = document.activeElement as HTMLElement | null;
      const id = active?.getAttribute("aria-activedescendant");
      return id ? (document.getElementById(id)?.textContent ?? null) : null;
    });
  }

  it("picks a cartridge from the list with the keyboard alone and fills the caliber (004 US2-3, US2-10)", async () => {
    await clickButton("Add firearm");
    await $('[role="dialog"]').waitForExist();
    await fill("Make", "Glock");
    await fill("Model", "17");
    await fill("Serial number", "E2E-9X19");
    await choose("Handgun");

    // A real click puts real focus in Make; from here on, keys only.
    await realClick('[data-field="make"] input');
    for (let step = 0; step < 8 && (await focusedFieldLabel()) !== "Cartridge"; step++) {
      await realKey("Tab");
    }
    expect(await focusedFieldLabel()).toBe("Cartridge");

    for (const key of ["9", "x", "1"]) await realKey(key);
    await $('[role="listbox"]').waitForDisplayed({ timeout: 5000 });

    // Escape closes the list, not the dialog, and keeps the typed text (004 US2-10).
    await realKey("Escape");
    await $('[role="listbox"]').waitForExist({ reverse: true });
    await expect($('[role="dialog"]')).toExist();
    expect(await fieldValue("Cartridge")).toBe("9x1");

    // Down opens the list again, and Down moves through it.
    for (let step = 0; step < 8; step++) {
      await realKey("Down");
      if ((await highlightedOption())?.startsWith("9x19mm Parabellum")) break;
    }
    expect(await highlightedOption()).toMatch(/^9x19mm Parabellum/);
    // The row says where it comes from (004 FR-016).
    expect(await highlightedOption()).toContain("Built-in · 9mm");

    await realKey("Return");

    expect(await fieldValue("Cartridge")).toBe("9x19mm Parabellum");
    await browser.waitUntil(async () => (await fieldValue("Caliber")) === "9mm", {
      timeoutMsg: "the caliber was never filled in from the cartridge",
    });
    expect(await focusedFieldLabel()).toBe("Cartridge");
    // Enter picked the row: the form is still open.
    await expect($('[role="dialog"]')).toExist();

    await clickButton("Add firearm");
    await $("#record-name").waitForExist({ timeout: 8000 });
    await settle();
  });
});
