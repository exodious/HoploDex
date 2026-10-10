import {
  $,
  $$,
  addFirearm,
  attachFile,
  back,
  browser,
  clickButton,
  createDatabase,
  expect,
  fill,
  focusedFieldLabel,
  goTo,
  titleBlock,
  toggle,
} from "../support/ui";
import type { NewFirearm } from "../support/ui";
import { realClick, realKey, skipWithoutRealInput } from "../support/realInput";

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
 * User Story 3 follows in the same session: the Rifle, carrying an Optic and
 * the AR (which carries the Suppressor), is marked disposed from the keyboard
 * with the Optic disposed with it (US3-1, US3-2).
 *
 * User Story 4 is in between: the Accessories page, with an unmounted Sling
 * added from the keyboard, is grouped by Mounted on through the grouping menu,
 * and shows the Rifle's group and "Not mounted" last (US4-2).
 *
 * User Story 1 ends the session: on the Accessories page, an Optic is added
 * from the keyboard, scheduled under a policy for less than its value (the
 * under-insured warning shows), given a photo and edited (SC-001, US1-1,
 * US1/AC8). These go through the real IPC paths the unit tests mock.
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
  const where = await browser.execute(
    () => document.activeElement?.outerHTML.slice(0, 160) ?? "nothing",
  );
  throw new Error(`Tab never reached "${text}"; focus is on ${where}`);
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

/** Presses Shift+Tab until a button, link or menu item whose text starts with
 * `text` has focus. */
async function shiftTabToControl(text: string) {
  for (let step = 0; step < 30; step++) {
    if ((await focusedControlText())?.startsWith(text)) return;
    await realKey("Shift_L+Tab");
  }
  throw new Error(`Shift+Tab never reached "${text}"`);
}

/** Opens the "Kind" field's list (focus is on it) and chooses `kind`. */
async function chooseKind(kind: string) {
  await realKey("Return");
  await $('[role="listbox"]').waitForDisplayed({ timeout: 5000 });
  for (let step = 0; step < 14; step++) {
    const option = await browser.execute(() => {
      const active = document.activeElement as HTMLElement | null;
      return active?.getAttribute("role") === "option" ? (active.textContent ?? "").trim() : null;
    });
    if (option === kind) break;
    await realKey("Down");
  }
  await realKey("Return");
}

/** Opens the focused select's list and chooses the option whose text starts
 * with `text` (an option's text can carry a detail line after its name). */
async function chooseOptionStartingWith(text: string) {
  await realKey("Return");
  await $('[role="listbox"]').waitForDisplayed({ timeout: 5000 });
  for (let step = 0; step < 14; step++) {
    const option = await browser.execute(() => {
      const active = document.activeElement as HTMLElement | null;
      return active?.getAttribute("role") === "option" ? (active.textContent ?? "").trim() : null;
    });
    if (option?.startsWith(text)) break;
    await realKey("Down");
  }
  await realKey("Return");
}

/** A local calendar date `days` from today, as YYYY-MM-DD. */
function isoDaysFromNow(days: number): string {
  const d = new Date();
  d.setDate(d.getDate() + days);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

// A tiny but genuinely valid PNG, so the backend's real image decoding and
// thumbnail generation run (as in us4-photos-documents.e2e.ts).
const SAMPLE_PNG_BASE64 =
  "iVBORw0KGgoAAAANSUhEUgAAABQAAAAUCAIAAAAC64paAAAAGUlEQVR42mNgaPhPPhrVPKp5VPOo5oHVDADApFaPDOtbFgAAAABJRU5ErkJggg==";

/** The group headings of the Accessories page, spaces collapsed. */
const groupHeadings = () =>
  browser.execute(() =>
    [...document.querySelectorAll<HTMLElement>(".hd-group__title")].map((h) =>
      (h.textContent ?? "").replace(/\s+/g, " ").trim(),
    ),
  );

/** Back to the collection by key: Shift+Tab from the heading to Back. */
async function backToCollectionByKeys() {
  await realClick("#record-name");
  for (let step = 0; step < 12; step++) {
    await realKey("Shift_L+Tab");
    const onBack = await browser.execute(() =>
      document.activeElement?.classList.contains("hd-backlink"),
    );
    if (onBack) break;
  }
  await realKey("Return");
  await $$(".hd-row__name")[0].waitForExist({ timeout: 8000 });
}

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

/** The text of the page's title block, spaces collapsed. */
const titleBlockText = () =>
  browser.execute(() =>
    (document.querySelector(".hd-titleblock")?.textContent ?? "").replace(/\s+/g, " "),
  );

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
  before(function () {
    skipWithoutRealInput(this);
  });
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

    // Mount the Suppressor, an existing firearm, on the Rifle.
    await openMountMenu(1);
    await searchMountDialog("omeg");
    await chooseSuggestion("SilencerCo Omega 300");
    await mountedSectionShows("SilencerCo Omega 300");
    expect(await mountedSectionText()).toContain("Nikon P3 · Optic");

    await backToCollectionByKeys();

    // Open the AR and mount the Suppressor on it. It is mounted on the Rifle,
    // so the dialog lists it with where it is, and moving it asks first.
    await openFirearmByKeys("Daniel Defense DDM4");
    await openMountMenu(1);
    await searchMountDialog("omeg");
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

describe("User Story 4 - Browsing accessories by what they are mounted on (specs/006-accessory-links)", () => {
  before(function () {
    skipWithoutRealInput(this);
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

describe("User Story 3 - Disposing with what is mounted (specs/006-accessory-links)", () => {
  before(function () {
    skipWithoutRealInput(this);
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

describe("User Story 1 - Recording accessories (specs/006-accessory-links)", () => {
  before(function () {
    skipWithoutRealInput(this);
  });
  it("adds an Optic, schedules it under a policy below its value, adds a photo and edits it, from the keyboard (SC-001, US1-1, US1/AC8)", async () => {
    // Setup, not timed: a schedule-only policy, from the Insurance page.
    await goTo("Insurance");
    await clickButton("Add policy");
    await fill("Policy name", "E2E Optic Rider");
    await fill("Policy number", "AC-1");
    await fill("Insurance company", "Acme Insurance");
    await fill("Coverage starts", "2020-01-01");
    await fill("Coverage ends", isoDaysFromNow(365));
    await clickButton("Add policy");
    await $("article.hd-policy*=E2E Optic Rider").waitForExist({ timeout: 8000 });

    const started = Date.now();
    // Real focus on the page, then Shift+Tab back to the Accessories tab.
    await realClick("h1");
    await shiftTabToControl("Accessories");
    await realKey("Return");
    await $("h1=Accessories").waitForExist({ timeout: 8000 });
    await tabToControl("Add accessory");
    await realKey("Return");
    await $('[role="dialog"]').waitForDisplayed({ timeout: 5000 });

    await tabTo("Kind");
    await chooseKind("Optic");
    await tabTo("Make");
    await typeReal("Leupold");
    await tabTo("Model");
    await typeReal("VX3");
    await tabTo("Serial number");
    await typeReal("SC1");
    await tabTo("Estimated value");
    await typeReal("800");
    await realKey("Return");

    // Saving opens the Optic's record.
    await browser.waitUntil(
      async () =>
        (await $("#record-name").isExisting()) &&
        (await $("#record-name").getText()).includes("Leupold VX3"),
      { timeout: 8000, timeoutMsg: "the new Optic's record never opened" },
    );
    expect(await titleBlock("Replacement value")).toBe("$800");

    // Schedule it under the policy for $300, below its $800 value. This is
    // the real assign_accessory_coverage call (T125).
    await realClick("#record-name");
    await tabToControl("Assign");
    await realKey("Return");
    await $('[role="dialog"]').waitForDisplayed({ timeout: 5000 });
    await tabTo("Policy");
    await chooseOptionStartingWith("E2E Optic Rider");
    await tabTo("Scheduled amount");
    await typeReal("300");
    await realKey("Return");

    await browser.waitUntil(async () => (await titleBlock("Coverage")) === "Under-insured", {
      timeout: 8000,
      timeoutMsg: "the Optic never showed as under-insured",
    });
    await expect($(".hd-coverage*=$500 short of its value")).toExist();
    await expect($(".hd-facts--compact*=E2E Optic Rider")).toExist();
    await expect($(".hd-facts--compact*=Scheduled, $300")).toExist();

    // Add a photo. A file picker can't be driven by keys, so this puts the
    // file into the gallery's input as the picker would.
    await attachFile('input[aria-label="Add photos"]', {
      name: "scope.png",
      type: "image/png",
      base64: SAMPLE_PNG_BASE64,
    });
    await $(".hd-photo").waitForExist({ timeout: 8000 });
    await expect($(".hd-photo__tag*=Thumbnail")).toExist();

    // Edit it: a lower value, which the $300 schedule now covers.
    await realClick("#record-name");
    await tabToControl("Edit");
    await realKey("Return");
    await $('[role="dialog"]').waitForDisplayed({ timeout: 5000 });
    await tabTo("Estimated value");
    await realKey("Control_L+a");
    await realKey("BackSpace");
    await typeReal("250");
    await realKey("Return");

    await browser.waitUntil(async () => (await titleBlock("Replacement value")) === "$250", {
      timeout: 8000,
      timeoutMsg: "the edited value never showed",
    });
    expect(await titleBlock("Coverage")).toBe("Covered");
    // The photo and the schedule survive the edit.
    expect(await $$(".hd-photo").length).toBe(1);
    await expect($(".hd-facts--compact*=Scheduled, $300")).toExist();
    expect(Date.now() - started).toBeLessThan(TASK_LIMIT_MS);
  });
});
