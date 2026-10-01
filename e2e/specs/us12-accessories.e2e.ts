import {
  $,
  $$,
  addFirearm,
  back,
  browser,
  createDatabase,
  expect,
  focusedFieldLabel,
} from "../support/ui";
import type { NewFirearm } from "../support/ui";
import { realClick, realKey } from "../support/realInput";

/**
 * End-to-end coverage of specs/006-accessory-links' User Story 2 (SC-001,
 * US2-1, US2-3a, US2-4): from the keyboard alone, as real key events through
 * the X server, an accessory is added and mounted on a firearm from that
 * firearm's page, an existing firearm is mounted on it, and that firearm is
 * moved to another from the other's page after the move is confirmed. The
 * suggestion list's, menu's and dialogs' focus behaviour is WebKitGTK's to get
 * right, and WebDriver's own keys don't pass through GTK. See "Real keyboard
 * and mouse input" in DEVELOPMENT.md.
 *
 * The records are made first, through the Add firearm dialog (not timed); a
 * real click on a page's heading then puts real focus on the page, and from
 * there the keys alone do the work.
 */

/** The whole path must take well under a minute (SC-001 asks for the first
 * part, an accessory recorded and mounted, in under one); this is a guard
 * against a flow that became slow or needs the mouse, not a benchmark. */
const TASK_LIMIT_MS = 60_000;

/** Types one character with a real key press. */
async function pressChar(char: string) {
  if (char === " ") await realKey("space");
  else if (char === "-") await realKey("minus");
  else if (char === ".") await realKey("period");
  else if (/[A-Z]/.test(char)) await realKey(`Shift_L+${char.toLowerCase()}`);
  else if (/[a-z0-9]/.test(char)) await realKey(char);
  else throw new Error(`typeReal can't type "${char}"`);
}

/** The focused field's text. */
const focusedValue = () =>
  browser.execute(() => (document.activeElement as HTMLInputElement | null)?.value ?? "");

/** Types `text` with real key presses. Letters, digits, space, "-" and "."
 * only. A real key press now and then does not land in a field with a
 * suggestion list, so each character is checked and pressed again if it did
 * not. */
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

/** Presses Tab until the field whose label starts with `label` has focus. */
async function tabTo(label: string) {
  for (let step = 0; step < 20; step++) {
    const current = await focusedFieldLabel();
    if (current !== null && current.startsWith(label)) return;
    await realKey("Tab");
  }
  throw new Error(`Tab never reached "${label}"`);
}

/** The visible text of the focused button, link or menu item, or null. */
const focusedControlText = () =>
  browser.execute(() => {
    const active = document.activeElement as HTMLElement | null;
    return active?.matches('button, a, [role="menuitem"]')
      ? (active.textContent ?? "").trim()
      : null;
  });

/** Presses Tab until a button, link or menu item whose text starts with
 * `text` has focus. */
async function tabToControl(text: string) {
  for (let step = 0; step < 30; step++) {
    if ((await focusedControlText())?.startsWith(text)) return;
    await realKey("Tab");
  }
  throw new Error(`Tab never reached "${text}"`);
}

/** The text of the row `aria-activedescendant` points at, or null. */
async function highlightedOption(): Promise<string | null> {
  return browser.execute(() => {
    const active = document.activeElement as HTMLElement | null;
    const id = active?.getAttribute("aria-activedescendant");
    return id ? (document.getElementById(id)?.textContent ?? null) : null;
  });
}

/** Presses ↓ until the highlighted suggestion contains `text`, then Enter. */
async function chooseSuggestion(text: string) {
  await $('[role="listbox"]').waitForDisplayed({ timeout: 5000 });
  for (let step = 0; step < 4; step++) {
    await realKey("Down");
    if ((await highlightedOption())?.includes(text)) {
      await realKey("Return");
      return;
    }
  }
  throw new Error(`no suggestion containing "${text}" was highlighted`);
}

/** The text of the Mounted section, spaces collapsed. */
const mountedSectionText = () =>
  browser.execute(() => {
    const heading = [...document.querySelectorAll("h2, h3")].find(
      (h) => h.textContent?.trim() === "Mounted",
    );
    return (heading?.closest("section")?.textContent ?? "").replace(/\s+/g, " ");
  });

/** Waits until the Mounted section mentions `text`. */
async function mountedSectionShows(text: string) {
  await browser.waitUntil(async () => (await mountedSectionText()).includes(text), {
    timeout: 8000,
    timeoutMsg: `the Mounted section never showed "${text}"`,
  });
}

/** A real click on the record's heading puts real focus on the page; the keys
 * take over from there. Then Tab to Mount ▾, open its menu and choose the
 * entry `entry` places down the menu (0 = New accessory…, 1 = Existing
 * accessory or firearm…). */
async function openMountMenu(entry: 0 | 1) {
  await realClick("#record-name");
  await tabToControl("Mount");
  await realKey("Return");
  await $('[role="menu"]').waitForDisplayed({ timeout: 5000 });
  for (let step = 0; step < entry; step++) await realKey("Down");
  await realKey("Return");
}

/** The "Mount on {name}" dialog's search is focused when it opens; make sure,
 * then type. */
async function searchMountDialog(text: string) {
  await $('[role="dialog"]').waitForDisplayed({ timeout: 5000 });
  for (let step = 0; step < 4; step++) {
    const onCombobox = await browser.execute(
      () => document.activeElement?.getAttribute("role") === "combobox",
    );
    if (onCombobox) break;
    await realKey("Tab");
  }
  await typeReal(text);
}

/** Opens a firearm's record from the collection list with keys: Tab to its
 * name and press Enter. */
async function openFirearmByKeys(name: string) {
  await realClick("h1");
  for (let step = 0; step < 40; step++) {
    await realKey("Tab");
    const text = await browser.execute(() => {
      const active = document.activeElement as HTMLElement | null;
      return active?.classList.contains("hd-row__name")
        ? (active.textContent ?? "").replace(/\s+/g, " ").trim()
        : null;
    });
    if (text === name) {
      await realKey("Return");
      await browser.waitUntil(
        async () =>
          (await $("#record-name").isExisting()) &&
          (await $("#record-name").getText()).replace(/\s+/g, " ") === name,
        { timeout: 8000, timeoutMsg: `${name}'s record never opened` },
      );
      return;
    }
  }
  throw new Error(`Tab never reached "${name}" in the list`);
}

const RIFLE: NewFirearm = {
  make: "LaRue",
  model: "PredatAR",
  type: "Rifle",
  caliber: "5.56mm",
  serial: "E2E-RIFLE-1",
};
const AR: NewFirearm = {
  make: "Daniel Defense",
  model: "DDM4",
  type: "Rifle",
  caliber: "5.56mm",
  serial: "E2E-AR-1",
};
const SUPPRESSOR: NewFirearm = {
  make: "SilencerCo",
  model: "Omega 300",
  type: "Suppressor" as NewFirearm["type"],
  caliber: ".30",
  serial: "E2E-SUP-1",
};

describe("User Story 2 - Mounting (specs/006-accessory-links)", () => {
  // Each spec's session starts at the chooser with no databases (wdio.conf.ts).
  before(async () => {
    await createDatabase();
    for (const firearm of [RIFLE, AR, SUPPRESSOR]) {
      await addFirearm(firearm);
      await back();
    }
    expect(await $$(".hd-row__name").length).toBe(3);
  });

  it("adds an Optic from a Rifle's page, mounts a Suppressor on it, then moves the Suppressor to an AR, from the keyboard alone (SC-001, US2-1, US2-3a, US2-4)", async () => {
    const started = Date.now();

    // Open the seeded Rifle and add an Optic mounted on it.
    await openFirearmByKeys("LaRue PredatAR");
    await openMountMenu(0);
    await $('[role="dialog"]').waitForDisplayed({ timeout: 5000 });
    // Still the Add accessory form, with Mounted on already set to the Rifle.
    expect(await $('[role="dialog"]').getText()).toContain("Add accessory");

    await tabTo("Kind");
    await realKey("Return");
    await $('[role="listbox"]').waitForDisplayed({ timeout: 5000 });
    for (let step = 0; step < 14; step++) {
      const option = await browser.execute(() => {
        const active = document.activeElement as HTMLElement | null;
        return active?.getAttribute("role") === "option" ? (active.textContent ?? "").trim() : null;
      });
      if (option === "Optic") break;
      await realKey("Down");
    }
    await realKey("Return");

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

    // Mount the Suppressor, an existing firearm, on the Rifle.
    await openMountMenu(1);
    await searchMountDialog("omeg");
    await chooseSuggestion("SilencerCo Omega 300");
    await mountedSectionShows("SilencerCo Omega 300");
    expect(await mountedSectionText()).toContain("Nikon P3 · Optic");

    // Back to the collection by key: Shift+Tab from the heading to Back.
    await realClick("#record-name");
    for (let step = 0; step < 12; step++) {
      await realKey("Shift_L+Tab");
      const onBack = await browser.execute(() =>
        document.activeElement?.classList.contains("hd-backlink"),
      );
      if (onBack) break;
    }
    await realKey("Return");
    await $(".hd-row__name").waitForExist({ timeout: 8000 });

    // Open the AR and mount the Suppressor on it. It is mounted on the Rifle,
    // so the dialog lists it with where it is, and moving it asks first.
    await openFirearmByKeys("Daniel Defense DDM4");
    await openMountMenu(1);
    await searchMountDialog("omeg");
    await $('[role="listbox"]').waitForDisplayed({ timeout: 5000 });
    expect(await $('[role="listbox"]').getText()).toContain("Mounted on LaRue PredatAR");
    await chooseSuggestion("SilencerCo Omega 300");

    const ask = await $('[role="alertdialog"]');
    await ask.waitForDisplayed({ timeout: 5000 });
    const asked = (await ask.getText()).replace(/\s+/g, " ");
    expect(asked).toContain("Move SilencerCo Omega 300?");
    expect(asked).toContain(
      "It is mounted on LaRue PredatAR. Moving it takes everything mounted on it along.",
    );
    await tabToControl("Move");
    await realKey("Return");

    await mountedSectionShows("SilencerCo Omega 300");
    expect(Date.now() - started).toBeLessThan(TASK_LIMIT_MS);
  });
});
