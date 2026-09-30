import {
  back,
  browser,
  choose,
  clickButton,
  createDatabase,
  expect,
  fieldValue,
  fill,
  focusedFieldLabel,
} from "../support/ui";
import { realClick, realKey } from "../support/realInput";

/**
 * End-to-end coverage of specs/004-cartridges-action-types' User Story 2
 * (US2-3, US2-4, US2-10): the Cartridge suggestion list is driven with the
 * keyboard alone, as real key events through the X server, since the
 * list's focus and highlight behaviour is WebKitGTK's to get right and
 * WebDriver's own keys don't pass through GTK. See "Real keyboard and mouse
 * input" in DEVELOPMENT.md.
 */
describe("User Story 2 - Suggestions while typing (specs/004-cartridges-action-types)", () => {
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

  it("picks a cartridge from the list with the keyboard alone and fills the caliber (US2-3, US2-10)", async () => {
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

    for (let step = 0; step < 8; step++) {
      await realKey("Down");
      if ((await highlightedOption())?.startsWith("9x19mm Parabellum")) break;
    }
    expect(await highlightedOption()).toMatch(/^9x19mm Parabellum/);
    // The row says where it comes from (FR-016).
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
    await browser.pause(300);
  });

  it("finds the firearm under its cartridge when grouped by Cartridge", async () => {
    await back();
    await choose("Cartridge");
    await $("h2.hd-group__title*=9x19mm Parabellum").waitForExist({ timeout: 5000 });
  });

  it("offers the value now on record, marked with its count (US2-4)", async () => {
    await clickButton("Add firearm");
    await $('[role="dialog"]').waitForExist();
    await realClick('[data-field="cartridge"] input');
    for (const key of ["9", "x", "1"]) await realKey(key);

    await $('[role="option"]*=9x19mm Parabellum').waitForDisplayed({ timeout: 5000 });
    const row = (await $('[role="option"]*=9x19mm Parabellum').getText()).replace(/\s+/g, " ");
    expect(row).toContain("Built-in · 9mm · 1 in collection");
    // Once, however many places it comes from.
    expect((await $$('[role="option"]*=9x19mm Parabellum')).length).toBe(1);

    await realKey("Escape");
    await $('[role="listbox"]').waitForExist({ reverse: true });
    // Escape closed the list, not the dialog, and kept the text.
    await expect($('[role="dialog"]')).toExist();
    expect(await fieldValue("Cartridge")).toBe("9x1");
    await clickButton("Cancel");
  });
});
