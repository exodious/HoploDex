import {
  $,
  $$,
  addFirearm,
  back,
  browser,
  createDatabase,
  expect,
  focusedFieldLabel,
  invokeCommand,
  openFirearm,
} from "./ui";
import type { NewFirearm } from "./ui";
import { realClick, realKey } from "./realInput";

/**
 * What the accessory E2E specs share (specs/006-accessory-links; the
 * `us12-accessories-*.e2e.ts` files, one per user story): real-key helpers for
 * the keyboard-only flows, the three firearms they start from, and the two
 * seeds a story that needs earlier stories' records starts with. Each spec
 * file is a fresh app and sandbox, so a story that used to build on the one
 * before it recreates that state here instead.
 */

/** The whole path must take well under a minute (SC-001 asks for the first
 * part, an accessory recorded and mounted, in under one); this is a guard
 * against a flow that became slow or needs the mouse, not a benchmark. */
export const TASK_LIMIT_MS = 60_000;

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
export const focusedValue = () =>
  browser.execute(() => (document.activeElement as HTMLInputElement | null)?.value ?? "");

/** Types `text` with real key presses. Letters, digits, space, "-" and "."
 * only. A real key press now and then does not land in a field with a
 * suggestion list, so each character is checked and pressed again if it did
 * not. */
export async function typeReal(text: string) {
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
export async function tabTo(label: string) {
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
export async function tabToControl(text: string) {
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
export async function chooseSuggestion(text: string) {
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
export const mountedSectionText = () =>
  browser.execute(() => {
    const heading = [...document.querySelectorAll("h2, h3")].find(
      (h) => h.textContent?.trim() === "Mounted",
    );
    return (heading?.closest("section")?.textContent ?? "").replace(/\s+/g, " ");
  });

/** Waits until the Mounted section mentions `text`. */
export async function mountedSectionShows(text: string) {
  await browser.waitUntil(async () => (await mountedSectionText()).includes(text), {
    timeout: 8000,
    timeoutMsg: `the Mounted section never showed "${text}"`,
  });
}

/** A real click on the record's heading puts real focus on the page; the keys
 * take over from there. Then Tab to Mount ▾, open its menu and choose the
 * entry `entry` places down the menu (0 = New accessory…, 1 = Existing
 * accessory or firearm…). */
export async function openMountMenu(entry: 0 | 1) {
  await realClick("#record-name");
  await tabToControl("Mount");
  await realKey("Return");
  await $('[role="menu"]').waitForDisplayed({ timeout: 5000 });
  for (let step = 0; step < entry; step++) await realKey("Down");
  await realKey("Return");
}

/** The "Mount on {name}" dialog's search is focused when it opens; make sure,
 * then type. */
export async function searchMountDialog(text: string) {
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
export async function shiftTabToControl(text: string) {
  for (let step = 0; step < 30; step++) {
    if ((await focusedControlText())?.startsWith(text)) return;
    await realKey("Shift_L+Tab");
  }
  throw new Error(`Shift+Tab never reached "${text}"`);
}

/** Opens the "Kind" field's list (focus is on it) and chooses `kind`. */
export async function chooseKind(kind: string) {
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

/** Back to the collection by key: Shift+Tab from the heading to Back. */
export async function backToCollectionByKeys() {
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

/** Opens a firearm's record from the collection list with keys: Tab to its
 * name and press Enter. */
export async function openFirearmByKeys(name: string) {
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

export const RIFLE: NewFirearm = {
  make: "LaRue",
  model: "PredatAR",
  type: "Rifle",
  caliber: "5.56mm",
  serial: "E2E-RIFLE-1",
};
export const AR: NewFirearm = {
  make: "Daniel Defense",
  model: "DDM4",
  type: "Rifle",
  caliber: "5.56mm",
  serial: "E2E-AR-1",
};
export const SUPPRESSOR: NewFirearm = {
  make: "SilencerCo",
  model: "Omega 300",
  type: "Suppressor" as NewFirearm["type"],
  caliber: ".30",
  serial: "E2E-SUP-1",
};

/** A fresh database holding the Rifle, the AR and the Suppressor (in that
 * order, so their ids are 1, 2 and 3), made through the Add firearm dialog
 * (not timed). */
export async function seedFirearms() {
  // Each spec's session starts at the chooser with no databases (wdio.conf.ts).
  await createDatabase();
  for (const firearm of [RIFLE, AR, SUPPRESSOR]) {
    await addFirearm(firearm);
    await back();
  }
  expect(await $$(".hd-row__name").length).toBe(3);
}

/** The id of the accessory kind named `kind`, from the backend's list. */
async function accessoryKindId(kind: string): Promise<number> {
  const listed = (await invokeCommand("list_accessory_kinds", {})) as {
    kinds: { id: number; name: string }[];
  };
  const found = listed.kinds.find((k) => k.name === kind);
  if (!found) throw new Error(`no accessory kind "${kind}"`);
  return found.id;
}

/** Records an accessory through the backend, as the Add accessory dialog
 * would save it, for a story whose own subject is a later step. Returns its
 * id. */
export async function createAccessory(input: {
  kind: string;
  make: string;
  model: string;
  serialNumber?: string;
  estimatedValue?: number;
  mountedOn?: { kind: "firearm" | "accessory"; id: number };
}): Promise<number> {
  const created = (await invokeCommand("create_accessory", {
    input: {
      accessoryKindId: await accessoryKindId(input.kind),
      make: input.make,
      model: input.model,
      serialNumber: input.serialNumber ?? null,
      status: "active",
      estimatedValue: input.estimatedValue ?? null,
      mountedOn: input.mountedOn ?? null,
    },
  })) as { id: number };
  return created.id;
}

/** What the Mounting story leaves behind, for the stories after it: the
 * three firearms, the Nikon P3 Optic mounted on the Rifle (US2-1) and the
 * Suppressor moved from the Rifle to the AR (US2-3a, US2-4). The records are
 * made through the backend (setup, not timed), and the app is left on the
 * AR's record, where the Mounting story ended. */
export async function seedMountedRecords() {
  await seedFirearms();
  // The ids are the order seedFirearms added them in.
  const [rifle, ar, suppressor] = [1, 2, 3];
  await createAccessory({
    kind: "Optic",
    make: "Nikon",
    model: "P3",
    serialNumber: "N1",
    estimatedValue: 300,
    mountedOn: { kind: "firearm", id: rifle },
  });
  await invokeCommand("mount_record", {
    input: { item: { kind: "firearm", id: suppressor }, host: { kind: "firearm", id: ar } },
  });
  // A record loads when it opens, so the AR opens with the Suppressor on it.
  await openFirearm("Daniel Defense DDM4");
}
