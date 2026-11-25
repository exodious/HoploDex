import {
  $,
  $$,
  back,
  browser,
  choose,
  clickButton,
  goTo,
  groupBy,
  pressEscape,
  settle,
  settleChooserPlate,
  unlock,
  waitForChooser,
} from "../support/ui";
import { SCREENSHOT_WINDOW, chooseTheme, screenshotsEnabled, shot } from "../support/screenshots";

/**
 * The showcase's shots, light and dark: the GitHub Pages site's (site/) and
 * the README's, retaken on purpose with `npm run screenshots:showcase`
 * (docs/development/screenshots.md, "The showcase"). The setting is the
 * store shots' (store.e2e.ts): the sandbox at one fixed path, the seed's times
 * stamped, 1200 x 800. The seed is `--showcase`'s collection
 * (src-tauri/examples/support/showcase_collection.rs), so:
 *
 * - every firearm and accessory in view shows a real photo, never its type's
 *   drawing or a generated picture, which this checks before each shot that
 *   lists them;
 * - the collection mixes handguns, rifles and a shotgun, shown ungrouped so
 *   they're side by side;
 * - no text in view depends on the day: the policies' dates are fixed and in
 *   force until the end of 2028, so no card says "Expires" (this fails
 *   rather than shoot one that does: move the seed's dates on).
 */

/** The record shot's firearm, and the one with the scanned bill of sale. */
const RECORD = "Glock 19 Gen5";
const SCANNED = "SilencerCo Osprey 45";
const SCAN = "Bill of sale (scan).tif";
/** Typed in Make: the makes on record that it matches, firearms' and
 * accessories', each with how many records have it. */
const MAKE_TYPED = "S";

/** Opens the listed firearm whose name (with its nickname) starts `name`. */
async function openFirearm(name: string) {
  await browser.waitUntil(
    () =>
      browser.execute((wanted: string) => {
        const button = [...document.querySelectorAll<HTMLElement>(".hd-row__name")].find((b) =>
          b.textContent?.trim().startsWith(wanted),
        );
        button?.click();
        return Boolean(button);
      }, name),
    { timeout: 5000, timeoutMsg: `no firearm "${name}" in the list` },
  );
  await $("#record-name").waitForExist();
  await settle();
}

/** Fails unless `ok`, with `problem`: no shot is taken of a view that is off. */
function check(ok: boolean, problem: string) {
  if (!ok) throw new Error(problem);
}

function pageText(): Promise<string> {
  return browser.execute(() => (document.body.textContent ?? "").replace(/\s+/g, " "));
}

/** Fails unless the window is the shots' size and in `theme`, with no toast. */
async function assertSetting(name: string, theme: "light" | "dark") {
  const view = await browser.execute(() => ({
    width: window.innerWidth,
    height: window.innerHeight,
    theme: document.documentElement.getAttribute("data-theme"),
  }));
  check(
    view.width === SCREENSHOT_WINDOW.width && view.height === SCREENSHOT_WINDOW.height,
    `${name}: the window is ${view.width} x ${view.height}`,
  );
  check(view.theme === theme, `${name}: the theme is ${view.theme}, not ${theme}`);
  check(!(await $(".hd-toast").isExisting()), `${name}: a toast is on the page`);
}

/** Fails unless every listed record in view (rows and tiles) shows a photo. */
async function assertPhotosOnly(name: string) {
  const listed = await browser.execute(() => {
    const top = document.querySelector(".hd-topbar")!.getBoundingClientRect().bottom;
    return [...document.querySelectorAll(".hd-row, .hd-tile")]
      .filter((item) => {
        const box = item.getBoundingClientRect();
        return box.bottom > top && box.top < window.innerHeight;
      })
      .map((item) => ({
        name: item.querySelector(".hd-row__name, .hd-tile__name")?.textContent?.trim() ?? "",
        photo: Boolean(item.querySelector(".hd-row__thumb img, .hd-tile__image img")),
      }));
  });
  check(listed.length > 0, `${name}: no record is in view`);
  for (const item of listed) check(item.photo, `${name}: "${item.name}" shows no photo`);
}

/** Types `text` into the Add firearm dialog's `field` and waits for its list. */
async function typeInto(field: string, text: string) {
  await browser.execute(
    (selector: string, value: string) => {
      const input = document.querySelector<HTMLInputElement>(selector)!;
      input.scrollIntoView({ block: "center" });
      input.focus();
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(input, value);
      input.dispatchEvent(new Event("input", { bubbles: true }));
    },
    `[role="dialog"] [data-field="${field}"] input`,
    text,
  );
  await $('[role="listbox"]').waitForDisplayed({ timeout: 5000 });
  await settle();
}

async function openAddFirearm() {
  await clickButton("Add firearm");
  await $('[role="dialog"]').waitForExist();
  await settle();
}

/** Closes the dialog: Escape closes the list, then the dialog. */
async function closeDialog() {
  await pressEscape();
  if (await $('[role="listbox"]').isExisting()) await pressEscape();
  if (await $('[role="dialog"]').isExisting()) await pressEscape();
  await $('[role="dialog"]').waitForExist({ reverse: true });
}

async function toTop() {
  await browser.execute(() => window.scrollTo(0, 0));
  await settle();
}

describe("The showcase shots", () => {
  before(async () => {
    if (!screenshotsEnabled()) {
      throw new Error(
        "Run this with `npm run screenshots:showcase` (docs/development/screenshots.md)",
      );
    }
  });

  it("chooser", async () => {
    await waitForChooser();
    for (const theme of ["Light", "Dark"] as const) {
      const t = theme === "Light" ? "light" : "dark";
      await chooseTheme(theme);
      await settleChooserPlate();
      await assertSetting(`chooser-${t}`, t);
      const text = await pageText();
      check(text.includes("Today at 10:30 AM"), "chooser: the last open isn't the fixed time");
      await shot(`chooser-${t}`);
    }
  });

  for (const theme of ["Light", "Dark"] as const) {
    const t = theme === "Light" ? "light" : "dark";

    it(`the collection, ${t}`, async () => {
      if (theme === "Light") await unlock(process.env.HOPLODEX_E2E_SEED_PASSPHRASE!);
      await chooseTheme(theme);
      await goTo("Collection");
      await groupBy("None");
      await choose("Tiles");
      await $$(".hd-tile__name")[0].waitForExist();
      await toTop();
      await assertSetting(`tiles-${t}`, t);
      await assertPhotosOnly(`tiles-${t}`);
      await shot(`tiles-${t}`);
      await choose("List");
      await $$(".hd-row__name")[0].waitForExist();
    });

    it(`a record, ${t}`, async () => {
      await openFirearm(RECORD);
      await assertSetting(`record-${t}`, t);
      check((await $$(".hd-photo")).length > 1, `record-${t}: the record has one photo or none`);
      await shot(`record-${t}`);
      await back();
    });

    it(`insurance, ${t}`, async () => {
      await goTo("Insurance");
      await $("h1=Insurance").waitForExist();
      await toTop();
      await assertSetting(`insurance-${t}`, t);
      const text = await pageText();
      check(
        !/Expires (in|today)|Starts /.test(text),
        `insurance-${t}: a policy's status depends on the day; move the seed's policy dates on`,
      );
      await shot(`insurance-${t}`);
    });

    it(`accessories, ${t}`, async () => {
      await goTo("Accessories");
      await $$(".hd-row__name")[0].waitForExist();
      await groupBy("Kind");
      await $$("h2.hd-group__title")[0].waitForExist();
      await toTop();
      await assertSetting(`accessories-${t}`, t);
      await assertPhotosOnly(`accessories-${t}`);
      await shot(`accessories-${t}`);
    });

    it(`suggestions, ${t}`, async () => {
      await goTo("Collection");
      await assertPhotosOnly(`cartridge-${t}`);
      await openAddFirearm();
      await typeInto("cartridge", "9mm");
      await assertSetting(`cartridge-${t}`, t);
      await shot(`cartridge-${t}`);
      await closeDialog();

      await openAddFirearm();
      await typeInto("make", MAKE_TYPED);
      await assertSetting(`make-${t}`, t);
      await shot(`make-${t}`);
      await closeDialog();
    });

    it(`a document, ${t}`, async () => {
      await openFirearm(SCANNED);
      await browser.waitUntil(
        () =>
          browser.execute((wanted: string) => {
            const button = [...document.querySelectorAll<HTMLElement>("button.hd-doc__name")].find(
              (b) => b.textContent?.trim() === wanted,
            );
            button?.click();
            return Boolean(button);
          }, SCAN),
        { timeout: 5000, timeoutMsg: `no document "${SCAN}"` },
      );
      const viewer = ".hd-dialog__content--xl";
      await $(`${viewer} img[alt="${SCAN}, page 1 of 3"]`).waitForExist({ timeout: 10000 });
      await settle();
      await assertSetting(`document-preview-${t}`, t);
      await shot(`document-preview-${t}`);
      await pressEscape();
      await $(viewer).waitForExist({ reverse: true });
      await back();
    });
  }
});
