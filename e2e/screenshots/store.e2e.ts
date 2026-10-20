import {
  $,
  $$,
  back,
  browser,
  clickButton,
  fill,
  goTo,
  groupBy,
  settle,
  unlock,
  waitForChooser,
  pressEscape,
  settleChooserPlate,
} from "../support/ui";
import { SCREENSHOT_WINDOW, chooseTheme, screenshotsEnabled, shot } from "../support/screenshots";
import { readStoreShots, type StoreShot } from "./storeShots";

/**
 * The Flatpak's five store shots (011 FR-017, research.md §13), retaken on
 * purpose with `npm run screenshots:store`, never by a build or by
 * `npm run screenshots`, and not part of `npm run test:e2e`. Where they differ
 * from the walk's (screens.e2e.ts):
 *
 * - The sandbox is at one fixed path and the seed stamps its times with
 *   `--fixed-times` (wdio.conf.ts), so no text in view depends on the day or
 *   the time the script runs: the chooser says "Today at 10:30 AM", the viewer
 *   "added Sep 15, 2025".
 * - The seed's one $99,999,999 firearm (#67, which the seed keeps) has its
 *   estimated replacement value set to $2,500 through the form, so the
 *   collection's total reads $51,300, and the list is grouped by Type so the
 *   Handgun group opens it and no photo that needs a credit line is in view.
 * - What each shot shows, by record and photo, is store-shots.json; this
 *   checks the view against it before every shot, and fails rather than take
 *   one that shows something else.
 *
 * Light theme, 1200 x 800, the walk's own setting. Names and order are
 * store-shots.json's, which the metainfo lists.
 */

const SHOTS = readStoreShots().shots;
const shotNamed = (name: string): StoreShot => {
  const found = SHOTS.find((s) => s.name === name);
  if (!found) throw new Error(`store-shots.json has no shot "${name}"`);
  return found;
};

/** Stops the run, with `problem`, unless `ok`: no shot is taken of a view the
 * recorded list doesn't describe. */
function check(ok: boolean, problem: string) {
  if (!ok) throw new Error(problem);
}

const COMMEMORATIVE = "Smith & Wesson Model 1911 A1 Government Commemorative";
const RECORD = "Glock 19 Gen5";

async function openRecord(name: string) {
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

/** The page's text, with every run of white space (a no-break space between a
 * time and "AM" included) one space. */
function pageText(): Promise<string> {
  return browser.execute(() => (document.body.textContent ?? "").replace(/\s+/g, " "));
}

/** Fails unless the page is the size and theme the shots are taken in and
 * shows the text `store-shots.json` says the shot has. */
async function assertSetting(shotName: string) {
  const view = await browser.execute(() => ({
    width: window.innerWidth,
    height: window.innerHeight,
    theme: document.documentElement.getAttribute("data-theme"),
  }));
  check(
    view.width === SCREENSHOT_WINDOW.width && view.height === SCREENSHOT_WINDOW.height,
    `${shotName}: the window is ${view.width} x ${view.height}`,
  );
  check(view.theme === "light", `${shotName}: the theme is ${view.theme}, not light`);
  check(!(await $(".hd-toast").isExisting()), `${shotName}: a toast is on the page`);
  const text = await pageText();
  for (const wanted of shotNamed(shotName).shows) {
    check(text.includes(wanted), `${shotName}: the page does not show "${wanted}"`);
  }
}

/** Fails unless the list is grouped by `grouping` (the menu's button says so). */
async function assertGroupedBy(shotName: string, grouping: string) {
  const label = await browser.execute(
    () =>
      document
        .querySelector('button[aria-haspopup="menu"][aria-label^="Group by"]')
        ?.getAttribute("aria-label") ?? "",
  );
  check(label === `Group by, ${grouping}`, `${shotName}: the menu says "${label}"`);
}

/** The listed rows in view (not above or under the top bar, not below the
 * window) and what each one's thumbnail is. */
async function viewportRows(): Promise<{ name: string; thumb: "photo" | "drawing" | "empty" }[]> {
  return browser.execute(() => {
    const top = document.querySelector(".hd-topbar")!.getBoundingClientRect().bottom;
    return [...document.querySelectorAll(".hd-row")]
      .filter((row) => {
        const box = row.getBoundingClientRect();
        return box.bottom > top && box.top < window.innerHeight;
      })
      .map((row) => {
        const thumb = row.querySelector(".hd-row__thumb");
        return {
          name: row.querySelector(".hd-row__name")?.textContent?.trim() ?? "",
          thumb: thumb?.querySelector("img")
            ? ("photo" as const)
            : thumb?.querySelector("svg.hd-drawing")
              ? ("drawing" as const)
              : ("empty" as const),
        };
      });
  });
}

/** Fails unless the rows in view are those `store-shots.json` records for the
 * shot: a photo only on a record it lists (or one whose picture is generated),
 * a drawing on the rest, and every record it names in view. This is what
 * keeps a photo that needs a credit line out of the shot (research.md §13). */
async function assertRows(shotName: string) {
  const shot = shotNamed(shotName);
  const withPhoto = [...shot.photos.map((p) => p.record), ...shot.generated];
  const rows = await viewportRows();
  const startsWith = (row: { name: string }, wanted: string) => row.name.startsWith(wanted);
  for (const row of rows) {
    const photo = withPhoto.some((wanted) => startsWith(row, wanted));
    check(
      photo ? row.thumb === "photo" : row.thumb === "drawing",
      `${shotName}: "${row.name}" shows ${row.thumb}, but ${photo ? "a photo" : "a drawing"} is recorded`,
    );
  }
  for (const wanted of [...withPhoto, ...shot.drawings]) {
    check(
      rows.some((row) => startsWith(row, wanted)),
      `${shotName}: "${wanted}" is not in view`,
    );
  }
}

describe("011 US4 - Flatpak store shots", () => {
  before(async () => {
    if (!screenshotsEnabled()) {
      throw new Error("Run this with `npm run screenshots:store` (DEVELOPMENT.md, Screenshots)");
    }
  });

  it("chooser", async () => {
    await waitForChooser();
    await chooseTheme("Light");
    await settleChooserPlate();
    await assertSetting("chooser");
    await shot("chooser");
  });

  it("collection", async () => {
    await unlock(process.env.HOPLODEX_E2E_SEED_PASSPHRASE!);
    await chooseTheme("Light");
    await goTo("Collection");

    // The $99,999,999 firearm's estimated replacement value, set to $2,500.
    await openRecord(COMMEMORATIVE);
    await clickButton("Edit");
    await $('[role="dialog"]').waitForExist();
    await settle();
    await fill("Estimated replacement value", "2500");
    await clickButton("Save changes");
    await $('[role="dialog"]').waitForExist({ reverse: true });
    await settle();
    check((await pageText()).includes("$2,500"), "the edited value is not $2,500");
    // The "Saved changes" toast leaves by itself (4 s); no shot has it.
    await $(".hd-toast").waitForExist({ reverse: true, timeout: 15000 });
    await back();

    await groupBy("Type");
    await $$("h2.hd-group__title")[0].waitForExist();
    await browser.execute(() => window.scrollTo(0, 0));
    await settle();
    await assertSetting("collection");
    await assertGroupedBy("collection", "Type");
    await assertRows("collection");
    await shot("collection");
  });

  it("record", async () => {
    await openRecord(RECORD);
    await assertSetting("record");
    const photos = await $$(".hd-photo");
    check(
      photos.length === shotNamed("record").photos.length,
      `record: ${photos.length} photos in the strip, ${shotNamed("record").photos.length} recorded`,
    );
    await shot("record");
    await back();
  });

  it("accessories", async () => {
    await goTo("Accessories");
    await $("h1=Accessories").waitForExist();
    await $$(".hd-row__name")[0].waitForExist();
    await groupBy("Kind");
    await $$("h2.hd-group__title")[0].waitForExist();
    await assertSetting("accessories");
    await assertGroupedBy("accessories", "Kind");
    await assertRows("accessories");
    await shot("accessories");
  });

  it("document-preview", async () => {
    await goTo("Collection");
    await openRecord("Dead Air Sandman-K");
    check((await $$(".hd-photo")).length === 0, "document-preview: the record has a photo");
    await browser.waitUntil(
      () =>
        browser.execute(() => {
          const button = [...document.querySelectorAll<HTMLElement>("button.hd-doc__name")].find(
            (b) => b.textContent?.trim() === "Bill of sale (scan).tif",
          );
          button?.click();
          return Boolean(button);
        }),
      { timeout: 5000, timeoutMsg: 'no document "Bill of sale (scan).tif" in the list' },
    );
    const viewer = ".hd-dialog__content--xl";
    await $(`${viewer} .hd-dialog__title`).waitForExist();
    await $(`${viewer} img[alt="Bill of sale (scan).tif, page 1 of 3"]`).waitForExist({
      timeout: 10000,
    });
    await settle();
    await assertSetting("document-preview");
    await shot("document-preview");
    await pressEscape();
    await $(viewer).waitForExist({ reverse: true });
  });
});
