import {
  $,
  $$,
  browser,
  chooseMenuItem,
  clickButton,
  clickEl,
  confirmTakeOver,
  expect,
  fill,
  invokeCommand,
  selectChooserRow,
  settle,
  submitPassphrase,
  switchDatabase,
  unlock,
  waitForChooser,
  waitForCollection,
} from "../support/ui";

/**
 * End-to-end coverage of specs/008-hardening-batch's User Story 1 against
 * the real built app: nothing of one database's session reaches the next
 * (FR-001, FR-002, FR-005, SC-001). The two databases the human-testing seed
 * makes, "Main collection" and "Shared collection", each have a firearm whose
 * thumbnail is the photo with id 1, with different bytes
 * (src-tauri/examples/human_seed.rs), so a thumbnail kept from the first
 * session and shown in the second is a different picture and is noticed.
 *
 * Needs the seeded collection (wdio.conf.ts); the one passphrase opens both.
 */

const MAIN = "Main collection";
const SHARED = "Shared collection";
const GLOCK = "Glock 19 Gen5";
const BERETTA = "Beretta 92FS";

const passphrase = () => process.env.HOPLODEX_E2E_SEED_PASSPHRASE!;

/** Presses Ctrl+L on whatever has focus, as the user would. */
async function pressLockShortcut() {
  await browser.execute(() => {
    const target = document.activeElement ?? document.body;
    target.dispatchEvent(
      new KeyboardEvent("keydown", { key: "l", ctrlKey: true, bubbles: true, cancelable: true }),
    );
  });
  await waitForChooser();
}

/** Selects the chooser row for `name` unless it is the selected one (a row's
 * select button exists only for the others). */
async function chooseRow(name: string) {
  const select = await $(`button.hd-db-row__select[aria-label^="${name}, "]`);
  if (await select.isExisting()) await selectChooserRow(name);
}

/** Opens "Shared collection". The first time, the seed has it marked open on
 * another computer and holding pending changes: this takes it over and
 * discards them, which is what leaves its collection to look at. After that
 * it is an ordinary open. */
let sharedFreed = false;
async function openShared() {
  await chooseRow(SHARED);
  await submitPassphrase(passphrase());
  if (!sharedFreed) {
    await confirmTakeOver(SHARED, passphrase());
    await $("button=Discard changes").waitForExist({ timeout: 20000 });
    await clickButton("Discard changes");
    await $('[role="alertdialog"]').waitForExist();
    await clickButton("Discard changes");
    sharedFreed = true;
  }
  await waitForCollection();
}

async function openMain() {
  await chooseRow(MAIN);
  await unlock(passphrase());
}

/** The picture a listed firearm's row shows (its data URL), once it has
 * loaded. The row is scrolled into view first, because the thumbnails of a
 * long list load only near the viewport. */
async function rowPicture(name: string): Promise<string> {
  let src = "";
  await browser.waitUntil(
    async () => {
      src = await browser.execute((wanted: string) => {
        const row = [...document.querySelectorAll<HTMLElement>(".hd-row")].find((r) =>
          r.querySelector(".hd-row__name")?.textContent?.trim().startsWith(wanted),
        );
        row?.scrollIntoView({ block: "center" });
        return row?.querySelector<HTMLImageElement>(".hd-row__thumb img")?.src ?? "";
      }, name);
      return src.startsWith("data:image/");
    },
    { timeout: 10000, timeoutMsg: `"${name}" never showed a thumbnail` },
  );
  return src;
}

/** Every picture the listed rows show. */
async function everyRowPicture(): Promise<string[]> {
  return browser.execute(() =>
    [...document.querySelectorAll<HTMLImageElement>(".hd-row__thumb img")].map((img) => img.src),
  );
}

/** What the open database holds as photo 1's thumbnail, straight from the
 * backend. */
async function photoOneThumbnail(): Promise<string> {
  const bytes = (await invokeCommand("get_photo_thumbnail", { photoId: 1 })) as number[];
  return JSON.stringify(bytes);
}

async function toastCount(): Promise<number> {
  return (await $$(".hd-toast")).length;
}

describe("User Story 1 (008) - Nothing of one session reaches the next", () => {
  let mainPicture = "";
  let mainPhotoOne = "";
  let sharedPicture = "";

  it("shows Main's own thumbnails when it is opened", async () => {
    await openMain();
    mainPicture = await rowPicture(GLOCK);
    mainPhotoOne = await photoOneThumbnail();
    expect(mainPicture).toMatch(/^data:image\//);
  });

  it("shows Shared's own thumbnail, not Main's, after locking Main and opening Shared", async () => {
    await pressLockShortcut();
    await openShared();

    // The two databases really do give photo 1 to different pictures.
    expect(await photoOneThumbnail()).not.toBe(mainPhotoOne);
    sharedPicture = await rowPicture(BERETTA);
    expect(sharedPicture).not.toBe(mainPicture);
    expect(await everyRowPicture()).not.toContain(mainPicture);
    // Nothing of Main's session says anything in Shared's.
    await settle();
    expect(await toastCount()).toBe(0);
  });

  it("shows Main's own thumbnail again after switching back through the database menu", async () => {
    await switchDatabase();
    await openMain();

    expect(await photoOneThumbnail()).toBe(mainPhotoOne);
    const picture = await rowPicture(GLOCK);
    expect(picture).toBe(mainPicture);
    expect(picture).not.toBe(sharedPicture);
    await settle();
    expect(await toastCount()).toBe(0);
  });

  it("keeps showing only Main's after restoring a backup of it", async () => {
    await chooseMenuItem("button.hd-db-menu", "Restore from a backup…");
    await $$('[role="dialog"] input[type="radio"]')[0].waitForExist();
    await settle();
    await fill("Passphrase for this backup", passphrase());
    await clickEl('[role="dialog"] button[type="submit"]');
    await $('[role="alertdialog"]').waitForExist();
    await clickEl('[role="alertdialog"] .hd-button--danger');
    await $('[role="dialog"]').waitForExist({ reverse: true, timeout: 20000 });
    await waitForCollection();

    // The restore starts a new session on the same file: what it shows is
    // that file's, and nothing the session before it (or Shared's) kept.
    expect(await photoOneThumbnail()).toBe(mainPhotoOne);
    const picture = await rowPicture(GLOCK);
    expect(picture).toBe(mainPicture);
    expect(picture).not.toBe(sharedPicture);
    expect(await toastCount()).toBe(0);
  });

  it("shows Shared's own thumbnail after locking the restored Main", async () => {
    await pressLockShortcut();
    await openShared();

    expect(await photoOneThumbnail()).not.toBe(mainPhotoOne);
    const picture = await rowPicture(BERETTA);
    expect(picture).toBe(sharedPicture);
    expect(await everyRowPicture()).not.toContain(mainPicture);
  });
});
