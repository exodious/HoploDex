import {
  $,
  addFirearm,
  back,
  browser,
  clickButton,
  createDatabase,
  expect,
  focusedFieldLabel,
} from "../support/ui";
import { realClick, realKey } from "../support/realInput";

/**
 * End-to-end coverage of specs/005-regulated-item-types' User Story 2
 * (SC-001, US2-1, US2-2, US2-5, US2-9): a Suppressor is added and registered
 * from the keyboard alone, as real key events through the X server, since the
 * suggestion list's and menu's focus behaviour is WebKitGTK's to get right and
 * WebDriver's own keys don't pass through GTK. See "Real keyboard and mouse
 * input" in DEVELOPMENT.md.
 */

/** The whole task must take well under two minutes (SC-001); this is a guard
 * against a flow that became slow or needs the mouse, not a benchmark. */
const TASK_LIMIT_MS = 120_000;

/** Types one character with a real key press. */
async function pressChar(char: string) {
  if (char === " ") await realKey("space");
  else if (char === "-") await realKey("minus");
  else if (/[A-Z]/.test(char)) await realKey(`Shift_L+${char.toLowerCase()}`);
  else if (/[a-z0-9]/.test(char)) await realKey(char);
  else throw new Error(`typeReal can't type "${char}"`);
}

/** The focused field's text. */
const focusedValue = () =>
  browser.execute(() => (document.activeElement as HTMLInputElement | null)?.value ?? "");

/** Types `text` with real key presses. Letters, digits, space and "-" only.
 * A real key press now and then does not land in a field with a suggestion list,
 * so each character is checked and pressed again if it did not. */
async function typeReal(text: string) {
  const before = await focusedValue();
  for (const [index, char] of [...text].entries()) {
    const expected = before + text.slice(0, index + 1);
    for (let attempt = 0; attempt < 3; attempt++) {
      await pressChar(char);
      if ((await focusedValue()) === expected) break;
    }
  }
}

/** Presses Tab until the field labelled `label` has focus. */
async function tabTo(label: string) {
  for (let step = 0; step < 20; step++) {
    if ((await focusedFieldLabel()) === label) return;
    await realKey("Tab");
  }
  throw new Error(`Tab never reached "${label}"`);
}

/** The text of the row `aria-activedescendant` points at, or null. */
async function highlightedOption(): Promise<string | null> {
  return browser.execute(() => {
    const active = document.activeElement as HTMLElement | null;
    const id = active?.getAttribute("aria-activedescendant");
    return id ? (document.getElementById(id)?.textContent ?? null) : null;
  });
}

describe("User Story 2 - Registration (specs/005-regulated-item-types)", () => {
  // Each spec's session starts at the chooser with no databases (wdio.conf.ts).
  before(async () => {
    await createDatabase();
  });

  it("adds and registers a Suppressor from the keyboard alone (SC-001)", async () => {
    const started = Date.now();
    await clickButton("Add firearm");
    await $('[role="dialog"]').waitForExist();

    // A real click puts real focus in Make; from here on, keys only.
    await realClick('[data-field="make"] input');
    await typeReal("SilencerCo");
    await tabTo("Model");
    await typeReal("Omega 300");

    // Type is a group of radio cards: the arrow keys choose, Suppressor last.
    for (let step = 0; step < 6; step++) {
      await realKey("Tab");
      const onRadio = await browser.execute(
        () => document.activeElement?.closest('[data-field="firearmTypeId"]') !== null,
      );
      if (onRadio) break;
    }
    for (let step = 0; step < 5; step++) await realKey("Down");
    expect(
      await browser.execute(
        () =>
          (
            document.querySelector(
              '[data-field="firearmTypeId"] input[type="radio"]:checked',
            ) as HTMLInputElement
          )?.value,
      ),
    ).toBe("5");

    // Suppressor omits the action, and its caliber is a rating.
    await tabTo("Caliber rating");
    await typeReal("30");
    await tabTo("Serial number");
    await typeReal("e2e-sup-1");

    // The Registration section: open it, choose the classification.
    for (let step = 0; step < 6; step++) {
      await realKey("Tab");
      const onRegistration = await browser.execute(
        () =>
          (document.activeElement?.textContent ?? "").startsWith("Registration") &&
          document.activeElement?.getAttribute("aria-expanded") !== null,
      );
      if (onRegistration) break;
    }
    await realKey("Return");
    await tabTo("Registered as");
    await realKey("Return");
    await $('[role="listbox"]').waitForDisplayed({ timeout: 5000 });
    await realKey("Down");
    await realKey("Return");

    // Form: the list offers the built-in names; Form 4 is first.
    await tabTo("Form");
    await typeReal("Form");
    await $('[role="listbox"]').waitForDisplayed({ timeout: 5000 });
    await realKey("Down");
    expect(await highlightedOption()).toMatch(/^Form 4/);
    await realKey("Return");

    await tabTo("Approved");
    await typeReal("2026-02-10");
    await tabTo("Registered to");
    await typeReal("Smith Family Trust");

    // Enter in the last field saves.
    await realKey("Return");
    await $("#record-name").waitForExist({ timeout: 8000 });
    expect(Date.now() - started).toBeLessThan(TASK_LIMIT_MS);

    // Reopened: the registration is on the record, and the fields a suppressor
    // omits were never offered.
    const panel = await $('section[aria-labelledby="registration-title"]');
    await panel.waitForExist();
    const facts = await browser.execute(
      (element: HTMLElement) =>
        [...element.querySelectorAll(".hd-facts__row")].map((row) => [
          row.querySelector("dt")?.textContent,
          row.querySelector("dd")?.textContent,
        ]),
      panel,
    );
    expect(facts).toEqual([
      ["Registered as", "Suppressor"],
      ["Form", "Form 4"],
      ["Approved", "Feb 10, 2026"],
      ["Registered to", "Smith Family Trust"],
    ]);
    const page = (await $("main").getText()).replace(/\s+/g, " ");
    expect(page).not.toMatch(/Barrel length|Capacity|\bAction\b/i);
    expect(page).toMatch(/Caliber rating/i);
  });

  it("groups by Registered as from the keyboard, with Unspecified last (US2-9)", async () => {
    await back();
    await back().catch(() => undefined);
    await addFirearm({
      make: "Ruger",
      model: "10/22",
      type: "Rifle",
      caliber: ".22 LR",
      serial: "E2E-PLAIN",
    });
    await back();

    // Keys only from the page: Tab to the grouping button, open the menu, jump
    // to "Registered as" by typing, choose it.
    await realClick("h1");
    for (let step = 0; step < 12; step++) {
      await realKey("Tab");
      const onGroupBy = await browser.execute(
        () =>
          document.activeElement?.getAttribute("aria-haspopup") === "menu" &&
          (document.activeElement?.getAttribute("aria-label") ?? "").startsWith("Group by"),
      );
      if (onGroupBy) break;
    }
    await realKey("Return");
    await $('[role="menu"]').waitForDisplayed({ timeout: 5000 });
    await realKey("r");
    await realKey("Return");

    await browser.waitUntil(async () => (await $$("h2.hd-group__title").length) >= 2, {
      timeoutMsg: "the collection never regrouped",
    });
    const titles = await $$("h2.hd-group__title").map((title) => title.getText());
    expect(titles.map((title) => title.split("\n")[0].trim())[0]).toContain("Suppressor");
    expect(titles[titles.length - 1]).toContain("Unspecified");
    await expect($('button[aria-label="Group by, Registered as"]')).toExist();
  });
});
