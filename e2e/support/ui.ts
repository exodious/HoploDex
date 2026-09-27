import { $, $$, browser } from "@wdio/globals";

/**
 * Shared helpers for driving the HoploDex UI through tauri-driver /
 * WebKitWebDriver.
 *
 * Clicks and value changes go through plain JS in the page rather than
 * WebDriver's native pointer/keyboard actions: WebKitWebDriver's native
 * click pipeline in this environment has a driver-level "element click
 * intercepted" / "did not become interactable" quirk even when the element
 * is independently verified (via elementFromPoint at the same
 * coordinates) to be on top and clickable, and its keystroke-based
 * setValue() can't clear a field or reliably fire React's change events.
 * None of the app's interactions depend on real pointer coordinates, so JS
 * clicks and native value setters are behaviorally equivalent for React's
 * handlers.
 */

const SETTLE_MS = 200;

// Runs in the page: the innermost open dialog, else the whole document —
// so a field label shared with the page underneath resolves to the one
// the user is actually looking at.
const SCOPE_JS = `
  const dialogs = document.querySelectorAll('[role="dialog"], [role="alertdialog"]');
  const scope = dialogs.length ? dialogs[dialogs.length - 1] : document;
`;

export async function clickEl(selector: string) {
  const el = await $(selector);
  await el.waitForExist();
  await browser.execute((element: HTMLElement) => element.click(), el);
  await browser.pause(SETTLE_MS);
}

/** Clicks the button whose visible text is exactly `text`, inside the
 * innermost open dialog when there is one. */
export async function clickButton(text: string) {
  await browser.waitUntil(
    () =>
      browser.execute(
        new Function(
          "text",
          `${SCOPE_JS}
          const button = [...scope.querySelectorAll("button")].find(
            (b) => b.textContent.trim() === text && !b.disabled,
          );
          if (button) button.click();
          return Boolean(button);`,
        ) as (text: string) => boolean,
        text,
      ),
    { timeout: 5000, timeoutMsg: `no enabled button "${text}"` },
  );
  await browser.pause(SETTLE_MS);
}

/** Opens the firearm form's disclosure group with this title if it is
 * closed (each starts closed on a record with none of its fields recorded).
 * Its button's text also carries a summary, so match on the title. */
async function openFormGroup(title: string) {
  await browser.waitUntil(
    () =>
      browser.execute(
        new Function(
          `${SCOPE_JS}
          const button = [...scope.querySelectorAll("button[aria-expanded]")].find((b) =>
            b.textContent.trim().startsWith(${JSON.stringify(title)}),
          );
          if (button && button.getAttribute("aria-expanded") === "false") button.click();
          return Boolean(button);`,
        ) as () => boolean,
      ),
    { timeout: 5000, timeoutMsg: `no "${title}" group` },
  );
  await browser.pause(SETTLE_MS);
}

/** Opens the "Origin and year of manufacture" group. */
export async function openOriginGroup() {
  await openFormGroup("Origin and year of manufacture");
}

/** Opens the "Physical details" group. */
export async function openPhysicalGroup() {
  await openFormGroup("Physical details");
}

/** Whether the button with this visible text, in the innermost open dialog,
 * is disabled. (An attribute selector can't express "button with this text"
 * after a descendant combinator, so this reads it from the page.) */
export async function isButtonDisabled(text: string): Promise<boolean> {
  return browser.execute(
    new Function(
      "text",
      `${SCOPE_JS}
      const button = [...scope.querySelectorAll("button")].find((b) => b.textContent.trim() === text);
      if (!button) throw new Error("no button " + text);
      return button.disabled;`,
    ) as (text: string) => boolean,
    text,
  );
}

/** Clicks one of the app's top-bar section tabs ("Collection", "Insurance"). */
export async function goTo(section: "Collection" | "Insurance") {
  await browser.execute((name: string) => {
    const tab = [...document.querySelectorAll<HTMLElement>(".hd-tab")].find((t) =>
      t.textContent?.trim().startsWith(name),
    );
    tab?.click();
  }, section);
  await browser.pause(400);
}

/** Sets a text, amount, or date field by its visible label. */
export async function fill(label: string, value: string) {
  const found = await browser.execute(
    new Function(
      "label",
      "value",
      `${SCOPE_JS}
      const labelEl = [...scope.querySelectorAll("label")].find(
        (l) => l.textContent.trim() === label && l.htmlFor,
      );
      const field = labelEl && document.getElementById(labelEl.htmlFor);
      if (!field) return false;
      const proto = field.tagName === "TEXTAREA" ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
      Object.getOwnPropertyDescriptor(proto, "value").set.call(field, value);
      field.dispatchEvent(new Event("input", { bubbles: true }));
      field.dispatchEvent(new Event("change", { bubbles: true }));
      return true;`,
    ) as (label: string, value: string) => boolean,
    label,
    value,
  );
  if (!found) throw new Error(`no field labelled "${label}"`);
  await browser.pause(100);
}

/** The current text of a labelled field in the open dialog. */
export async function fieldValue(label: string): Promise<string> {
  const value = await browser.execute(
    new Function(
      "label",
      `${SCOPE_JS}
      const labelEl = [...scope.querySelectorAll("label")].find(
        (l) => l.textContent.trim() === label && l.htmlFor,
      );
      const field = labelEl && document.getElementById(labelEl.htmlFor);
      return field ? field.value : null;`,
    ) as (label: string) => string | null,
    label,
  );
  if (value === null) throw new Error(`no field labelled "${label}"`);
  return value;
}

/** Pastes `text` into a labelled field, as the clipboard would: a real
 * `paste` event carrying the text, so the field can refuse it before it
 * changes anything (`fill` sets a value directly and never fires one). */
export async function pasteInto(label: string, text: string) {
  const found = await browser.execute(
    new Function(
      "label",
      "text",
      `${SCOPE_JS}
      const labelEl = [...scope.querySelectorAll("label")].find(
        (l) => l.textContent.trim() === label && l.htmlFor,
      );
      const field = labelEl && document.getElementById(labelEl.htmlFor);
      if (!field) return false;
      field.focus();
      const data = new DataTransfer();
      data.setData("text/plain", text);
      const paste = new ClipboardEvent("paste", { clipboardData: data, bubbles: true, cancelable: true });
      const proceed = field.dispatchEvent(paste);
      if (proceed) {
        // Nobody refused it: insert what the browser would have.
        const proto = HTMLInputElement.prototype;
        Object.getOwnPropertyDescriptor(proto, "value").set.call(field, field.value + text);
        field.dispatchEvent(new Event("input", { bubbles: true }));
      }
      return true;`,
    ) as (label: string, text: string) => boolean,
    label,
    text,
  );
  if (!found) throw new Error(`no field labelled "${label}"`);
  await browser.pause(150);
}

/** The visible label of the field that has keyboard focus, or null. */
export async function focusedFieldLabel(): Promise<string | null> {
  return browser.execute(() => {
    const active = document.activeElement as HTMLElement | null;
    if (!active?.id) return null;
    const label = document.querySelector(`label[for="${active.id}"]`);
    return label ? (label.textContent ?? "").trim() : null;
  });
}

/** Whether the labelled field is fully inside the visible window, i.e. was
 * scrolled into view rather than left below a dialog's fold. */
export async function isFieldInView(label: string): Promise<boolean> {
  return browser.execute(
    new Function(
      "label",
      `${SCOPE_JS}
      const labelEl = [...scope.querySelectorAll("label")].find(
        (l) => l.textContent.trim() === label && l.htmlFor,
      );
      const field = labelEl && document.getElementById(labelEl.htmlFor);
      if (!field) return false;
      const box = field.getBoundingClientRect();
      return box.top >= 0 && box.bottom <= window.innerHeight;`,
    ) as (label: string) => boolean,
    label,
  );
}

/** Follows the "Add" link beside an empty-state line such as
 * "No notes recorded." on a firearm's record. */
export async function followAddLink(emptyText: string) {
  const found = await browser.execute((text: string) => {
    const line = [...document.querySelectorAll(".hd-panel__empty")].find((p) =>
      p.textContent?.includes(text),
    );
    const link = line?.querySelector<HTMLElement>("button");
    link?.click();
    return Boolean(link);
  }, emptyText);
  if (!found) throw new Error(`no "Add" link beside "${emptyText}"`);
  await browser.pause(SETTLE_MS);
}

/** Whether a form section in the open dialog is currently highlighted (FR-038). */
export async function hasHighlightedSection(): Promise<boolean> {
  return browser.execute(() => Boolean(document.querySelector('[role="dialog"] [data-highlight]')));
}

/** Whether the labelled text field in the open dialog is disabled. */
export async function isFieldDisabled(label: string): Promise<boolean> {
  const state = await browser.execute(
    new Function(
      "label",
      `${SCOPE_JS}
      const labelEl = [...scope.querySelectorAll("label")].find(
        (l) => l.textContent.trim() === label && l.htmlFor,
      );
      const field = labelEl && document.getElementById(labelEl.htmlFor);
      return field ? field.disabled : null;`,
    ) as (label: string) => boolean | null,
    label,
  );
  if (state === null) throw new Error(`no field labelled "${label}"`);
  return state;
}

/** Picks a radio choice (type cards, segmented controls, coverage kind…)
 * by its visible label. */
export async function choose(text: string) {
  await browser.waitUntil(
    () =>
      browser.execute(
        new Function(
          "text",
          `${SCOPE_JS}
          const input = [...scope.querySelectorAll('input[type="radio"]')].find((r) => {
            const face = r.closest("label")?.querySelector(".hd-choice__label, .hd-segmented__face");
            return face && face.textContent.trim() === text;
          });
          if (input) input.click();
          return Boolean(input);`,
        ) as (text: string) => boolean,
        text,
      ),
    { timeout: 5000, timeoutMsg: `no choice "${text}"` },
  );
  await browser.pause(SETTLE_MS);
}

/** Toggles a checkbox by its visible label. */
export async function toggle(label: string) {
  const found = await browser.execute(
    new Function(
      "label",
      `${SCOPE_JS}
      const labelEl = [...scope.querySelectorAll("label")].find((l) => l.textContent.trim() === label);
      const box = labelEl && document.getElementById(labelEl.htmlFor);
      if (box) box.click();
      return Boolean(box);`,
    ) as (label: string) => boolean,
    label,
  );
  if (!found) throw new Error(`no checkbox labelled "${label}"`);
  await browser.pause(SETTLE_MS);
}

/** Opens a labelled dropdown (e.g. "Policy") and picks an option by name. */
export async function selectOption(label: string, option: string) {
  const opened = await browser.execute(
    new Function(
      "label",
      `${SCOPE_JS}
      const labelEl = [...scope.querySelectorAll("label")].find((l) => l.textContent.trim() === label);
      const trigger = labelEl && document.getElementById(labelEl.htmlFor);
      if (trigger) trigger.click();
      return Boolean(trigger);`,
    ) as (label: string) => boolean,
    label,
  );
  if (!opened) throw new Error(`no dropdown labelled "${label}"`);
  // The option list is fetched and portaled asynchronously; wait for it.
  await browser.waitUntil(
    () =>
      browser.execute((name: string) => {
        const item = [...document.querySelectorAll<HTMLElement>('[role="option"]')].find(
          (o) => o.querySelector(".hd-select__item-text > span")?.textContent?.trim() === name,
        );
        item?.click();
        return Boolean(item);
      }, option),
    { timeout: 5000, timeoutMsg: `no option "${option}" in "${label}"` },
  );
  await browser.pause(300);
}

/** Types into the collection search box and waits out its debounce. */
export async function search(term: string) {
  await browser.execute((value: string) => {
    const input = document.querySelector<HTMLInputElement>('input[type="search"]')!;
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(input, value);
    input.dispatchEvent(new Event("input", { bubbles: true }));
  }, term);
  await browser.pause(600);
}

/** Opens a firearm's record from the collection list by its "Make Model"
 * name (the first match, if several share it). */
export async function openFirearm(name: string) {
  await browser.waitUntil(
    () =>
      browser.execute((wanted: string) => {
        const button = [...document.querySelectorAll<HTMLElement>(".hd-row__name")].find(
          (b) => b.textContent?.trim() === wanted,
        );
        button?.click();
        return Boolean(button);
      }, name),
    { timeout: 5000, timeoutMsg: `no firearm "${name}" in the list` },
  );
  await $("#record-name").waitForExist();
  await browser.pause(300);
}

/** Leaves a record for the page it was opened from. */
export async function back() {
  await clickEl(".hd-backlink");
  await browser.pause(300);
}

/** Presses Escape as the user would, on whatever has focus. */
export async function pressEscape() {
  await browser.keys(["Escape"]);
  await browser.pause(300);
}

/** Scrolls a long record or policy page to the bottom and waits for the
 * pinned strip that keeps its heading's controls in reach (FR-041). */
export async function scrollToPinnedStrip() {
  await browser.execute(() => window.scrollTo(0, document.documentElement.scrollHeight));
  await $(".hd-runhead").waitForExist({ timeoutMsg: "the pinned strip never showed" });
  await browser.pause(200);
}

// Runs in the page: `reachable(el)` is true when `el` is on screen below the
// top bar and is what a click at its center would land on, so nothing covers
// it. JS clicks (see above) would succeed on an element that fails this, so
// tests that claim something is within reach check it here first.
const REACHABLE_JS = `
  const topbar = document.querySelector(".hd-topbar")?.getBoundingClientRect().bottom ?? 0;
  const reachable = (el) => {
    if (!el) return false;
    const box = el.getBoundingClientRect();
    if (box.width <= 0 || box.height <= 0) return false;
    if (box.top < topbar - 1 || box.bottom > window.innerHeight) return false;
    if (box.left < 0 || box.right > window.innerWidth) return false;
    const hit = document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2);
    return Boolean(hit) && (hit === el || el.contains(hit));
  };
  const text = (el) => (reachable(el) ? el.textContent.trim() : "");
`;

/** What the pinned strip shows within reach: the back link's label and key,
 * the record's name and stamp, and its actions (visible text, or accessible
 * name for icon buttons). A part that is off screen, covered, or collapsed
 * reads as "" (or is left out of `actions`). Reads text from the DOM because
 * WebDriver's getText() drops the strip's ellipsis-truncated spans. */
export async function pinnedStrip(): Promise<{
  back: string;
  backKey: string;
  name: string;
  stamp: string;
  actions: string[];
}> {
  return browser.execute(
    new Function(
      `${REACHABLE_JS}
      const strip = document.querySelector(".hd-runhead");
      const part = (selector) => text(strip?.querySelector(selector));
      return {
        back: part(".hd-backlink__label"),
        backKey: part(".hd-backlink__kbd"),
        name: part(".hd-runhead__name"),
        stamp: part(".hd-runhead__stamp"),
        actions: [...(strip?.querySelectorAll(".hd-runhead__actions button") ?? [])]
          .filter(reachable)
          .map((b) => b.getAttribute("aria-label") ?? b.textContent.trim()),
      };`,
    ) as () => { back: string; backKey: string; name: string; stamp: string; actions: string[] },
  );
}

/** The page's own back link (not the pinned strip's), as shown within reach:
 * its label and the key it shows (FR-040). */
export async function backLinkShown(): Promise<{ label: string; key: string }> {
  return browser.execute(
    new Function(
      `${REACHABLE_JS}
      const link = [...document.querySelectorAll(".hd-backlink")].find(
        (l) => !l.closest(".hd-runhead"),
      );
      return {
        label: text(link?.querySelector(".hd-backlink__label")),
        key: text(link?.querySelector(".hd-backlink__kbd")),
      };`,
    ) as () => { label: string; key: string },
  );
}

/** Clicks a button in the pinned strip by its visible text or accessible
 * name. */
export async function clickPinned(name: string) {
  const clicked = await browser.execute((wanted: string) => {
    const button = [...document.querySelectorAll<HTMLElement>(".hd-runhead button")].find(
      (b) => b.textContent?.trim() === wanted || b.getAttribute("aria-label") === wanted,
    );
    button?.click();
    return Boolean(button);
  }, name);
  if (!clicked) throw new Error(`no "${name}" button in the pinned strip`);
  await browser.pause(SETTLE_MS);
}

/** Names ("Make Model") of every firearm currently listed. */
export async function listedNames(): Promise<string[]> {
  return browser.execute(() =>
    [...document.querySelectorAll(".hd-row__name, .hd-tile__name")].map(
      (el) => el.textContent?.trim() ?? "",
    ),
  );
}

/** What a listed firearm's row thumbnail shows: its photo, its type's
 * generic drawing, or nothing yet. */
export async function rowThumbnail(name: string): Promise<"photo" | "drawing" | "empty"> {
  return browser.execute((wanted: string) => {
    const row = [...document.querySelectorAll(".hd-row")].find(
      (r) => r.querySelector(".hd-row__name")?.textContent?.trim() === wanted,
    );
    const thumb = row?.querySelector(".hd-row__thumb");
    if (thumb?.querySelector("img")) return "photo";
    if (thumb?.querySelector("svg.hd-drawing")) return "drawing";
    return "empty";
  }, name);
}

/** The record page's title-block value for a label ("Status", "Coverage"…). */
export async function titleBlock(label: string): Promise<string> {
  return browser.execute((wanted: string) => {
    const cell = [...document.querySelectorAll(".hd-titleblock__cell")].find(
      (c) => c.querySelector("dt")?.textContent?.trim().toLowerCase() === wanted.toLowerCase(),
    );
    return cell?.querySelector("dd")?.textContent?.trim() ?? "";
  }, label);
}

/** The text of the policy card titled `name` on the Insurance page, with
 * whitespace collapsed (flex layouts put line breaks between figures). */
export async function policyCardText(name: string): Promise<string> {
  const card = await $(`article.hd-policy*=${name}`);
  await card.waitForExist();
  return (await card.getText()).replace(/\s+/g, " ");
}

/** Puts a real file into a file input the way a file picker would, so the
 * app's own reading/upload path runs (WebDriver's file upload can't reach
 * a visually hidden input). */
export async function attachFile(
  inputSelector: string,
  file: { name: string; type: string; base64: string },
) {
  await browser.execute(
    (selector: string, name: string, type: string, base64: string) => {
      const bytes = Uint8Array.from(atob(base64), (c) => c.charCodeAt(0));
      const transfer = new DataTransfer();
      transfer.items.add(new File([bytes], name, { type }));
      const input = document.querySelector<HTMLInputElement>(selector)!;
      input.files = transfer.files;
      input.dispatchEvent(new Event("change", { bubbles: true }));
    },
    inputSelector,
    file.name,
    file.type,
    file.base64,
  );
  await browser.pause(800);
}

export interface NewFirearm {
  make: string;
  model: string;
  caliber: string;
  type: "Handgun" | "Rifle" | "Shotgun" | "Other";
  serial?: string;
  noSerial?: boolean;
  valueDollars?: string;
  notes?: string;
  nickname?: string;
  acquisitionDate?: string;
  /** FR-039: free text, searchable. */
  finish?: string;
  /** specs/002-firearm-identification FR-001. */
  origin?: "Domestic" | "Imported" | "Re-imported";
  /** specs/002-firearm-identification FR-003. */
  yearOfManufacture?: string;
  /** specs/002-firearm-identification FR-002 (Imported only). */
  countryOfManufacture?: string;
  /** specs/002-firearm-identification FR-002 (Imported/Re-imported only). */
  importerName?: string;
}

/** A firearm's display name as the app shows it: "Make Model", plus its
 * nickname in curly quotes when it has one (FR-031). */
export function displayName(firearm: { make: string; model: string; nickname?: string }) {
  const name = `${firearm.make} ${firearm.model}`;
  return firearm.nickname ? `${name} “${firearm.nickname}”` : name;
}

/** Fills the open Add firearm dialog without saving it. */
export async function fillFirearmForm(firearm: NewFirearm) {
  await fill("Make", firearm.make);
  await fill("Model", firearm.model);
  if (firearm.nickname) await fill("Nickname", firearm.nickname);
  await choose(firearm.type);
  await fill("Caliber", firearm.caliber);
  if (firearm.serial) await fill("Serial number", firearm.serial);
  if (firearm.noSerial) await toggle("This firearm has no serial number");
  if (firearm.valueDollars) await fill("Estimated replacement value", firearm.valueDollars);
  if (firearm.acquisitionDate) await fill("Date acquired", firearm.acquisitionDate);
  if (firearm.notes) await fill("Notes", firearm.notes);
  if (firearm.finish) {
    await openPhysicalGroup();
    await fill("Finish", firearm.finish);
  }
  if (
    firearm.origin ||
    firearm.countryOfManufacture ||
    firearm.importerName ||
    firearm.yearOfManufacture
  ) {
    await openOriginGroup();
  }
  if (firearm.origin) await choose(firearm.origin);
  if (firearm.countryOfManufacture)
    await fill("Country of manufacture", firearm.countryOfManufacture);
  if (firearm.importerName) await fill("Importer", firearm.importerName);
  if (firearm.yearOfManufacture) await fill("Year of manufacture", firearm.yearOfManufacture);
}

/** Adds a firearm through the Add firearm dialog, leaving the app on its
 * new record. */
export async function addFirearm(firearm: NewFirearm) {
  await clickButton("Add firearm");
  await $('[role="dialog"]').waitForExist();
  await fillFirearmForm(firearm);
  await clickButton("Add firearm");
  const expected = displayName(firearm);
  await browser.waitUntil(
    async () =>
      (await $("#record-name").isExisting()) &&
      // The nickname sits on its own line, so compare with whitespace collapsed.
      (await $("#record-name").getText()).replace(/\s+/g, " ") === expected,
    { timeout: 8000, timeoutMsg: `record for ${expected} never opened` },
  );
  await browser.pause(300);
}

/** The passphrase of every database a spec creates. */
export const E2E_PASSPHRASE = "end to end test passphrase";

/** The FR-004 acknowledgement in the create dialog. */
const ACKNOWLEDGEMENT =
  "I have stored this passphrase somewhere safe. If it is forgotten, nobody, including HoploDex, can open this database or recover the collection.";

/** The sandbox's documents folder, where the harness points the app's
 * suggested location (wdio.conf.ts). Specs type locations under it rather
 * than use the native pickers, which WebDriver can't drive. */
export function scratchDocuments(): string {
  const documents = process.env.HOPLODEX_E2E_DOCUMENTS;
  if (!documents) throw new Error("HOPLODEX_E2E_DOCUMENTS is not set (see wdio.conf.ts)");
  return documents;
}

/** Waits for the chooser, the screen shown whenever no database is open. */
export async function waitForChooser() {
  await $(".hd-chooser__title").waitForExist({ timeout: 10000 });
  await browser.pause(SETTLE_MS);
}

/** Waits for an open database's collection. */
export async function waitForCollection() {
  await $('nav[aria-label="Sections"]').waitForExist({ timeout: 10000 });
  await browser.pause(SETTLE_MS);
}

/** Creates a database from the chooser by typing its location, and waits
 * for its (empty) collection. Defaults: "Test", in the sandbox's suggested
 * folder, with {@link E2E_PASSPHRASE}. */
export async function createDatabase({
  folder = `${scratchDocuments()}/HoploDex`,
  name = "Test",
  passphrase = E2E_PASSPHRASE,
}: { folder?: string; name?: string; passphrase?: string } = {}) {
  await waitForChooser();
  await clickButton("Create a new database…");
  await $('[role="dialog"]').waitForExist();
  await fill("Name", name);
  await fill("Folder", folder);
  await fill("Passphrase", passphrase);
  await fill("Confirm passphrase", passphrase);
  await toggle(ACKNOWLEDGEMENT);
  await clickButton("Create database");
  await waitForCollection();
}

/** Types `passphrase` into the chooser's selected database and presses
 * **Open**, without waiting for the outcome. */
export async function submitPassphrase(passphrase: string) {
  await waitForChooser();
  const found = await browser.execute((value: string) => {
    const field = document.querySelector<HTMLInputElement>(".hd-db-row--selected input");
    if (!field) return false;
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(field, value);
    field.dispatchEvent(new Event("input", { bubbles: true }));
    field.form?.requestSubmit();
    return true;
  }, passphrase);
  if (!found) throw new Error("no database is selected in the chooser");
  await browser.pause(SETTLE_MS);
}

/** Opens the chooser's selected database with `passphrase` and waits for
 * its collection. */
export async function unlock(passphrase: string) {
  await submitPassphrase(passphrase);
  await waitForCollection();
}

/** Opens the Radix menu behind `triggerSelector` (it opens on pointer
 * down, which a plain click doesn't send) and picks the item whose label
 * starts with `item`. */
export async function chooseMenuItem(triggerSelector: string, item: string) {
  const trigger = await $(triggerSelector);
  await trigger.waitForExist();
  await browser.execute((element: HTMLElement) => {
    element.dispatchEvent(
      new PointerEvent("pointerdown", { bubbles: true, button: 0, pointerType: "mouse" }),
    );
  }, trigger);
  await $('[role="menu"]').waitForExist({ timeout: 5000 });
  await browser.waitUntil(
    () =>
      browser.execute((label: string) => {
        const found = [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')].find((m) =>
          (m.textContent ?? "").trim().startsWith(label),
        );
        found?.click();
        return Boolean(found);
      }, item),
    { timeout: 5000, timeoutMsg: `no menu item "${item}"` },
  );
  await browser.pause(SETTLE_MS);
}

/** Leaves the open database the way the database menu does, by locking it,
 * and waits for the chooser, where another can be opened. */
export async function switchDatabase() {
  await chooseMenuItem("button.hd-db-menu", "Lock now");
  await waitForChooser();
}

/** The chooser's rows, in order. */
export async function chooserNames(): Promise<string[]> {
  return browser.execute(() =>
    [...document.querySelectorAll(".hd-chooser__list .hd-db-row__name")].map(
      (name) => name.textContent ?? "",
    ),
  );
}

/** The name of the chooser's selected row. */
export async function selectedChooserRow(): Promise<string | null> {
  return browser.execute(
    () => document.querySelector(".hd-db-row--selected .hd-db-row__name")?.textContent ?? null,
  );
}

/** Selects the chooser row for `name`. */
export async function selectChooserRow(name: string) {
  await clickEl(`button.hd-db-row__select[aria-label^="${name}, "]`);
}

/** Asks the app to quit as the window's close button does: the backend's
 * `app:quit-requested` event, sent from the page. */
export async function requestQuit() {
  await browser.execute(() => {
    const internals = (
      window as unknown as {
        __TAURI_INTERNALS__: { invoke: (cmd: string, args: unknown) => Promise<unknown> };
      }
    ).__TAURI_INTERNALS__;
    void internals.invoke("plugin:event|emit", { event: "app:quit-requested", payload: {} });
  });
  await browser.pause(SETTLE_MS);
}

export { $, $$, browser };
export { expect } from "@wdio/globals";
