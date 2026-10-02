import fs from "node:fs";
import {
  $,
  E2E_PASSPHRASE,
  addFirearm,
  browser,
  chooseMenuItem,
  chooserNames,
  clickButton,
  clickEl,
  createDatabase,
  expect,
  fieldValue,
  fill,
  goTo,
  isButtonDisabled,
  listedNames,
  requestQuit,
  scratchDocuments,
  selectChooserRow,
  selectedChooserRow,
  settleChooserPlate,
  submitPassphrase,
  switchDatabase,
  toggle,
  unlock,
  waitForChooser,
  waitForCollection,
} from "../support/ui";

/**
 * End-to-end coverage of specs/003-database-protection-management's
 * databases stories, against the real built app: the session starts in a
 * sandbox with no databases (wdio.conf.ts), so the first screen is a first
 * run.
 */

const ACKNOWLEDGEMENT =
  "I have stored this passphrase somewhere safe. If it is forgotten, nobody, including HoploDex, can open this database or recover the collection.";

/** Ends the app and starts it again against the same sandbox, as quitting
 * and relaunching would. */
async function relaunch() {
  await browser.reloadSession();
  await browser.waitUntil(
    async () => (await browser.execute(() => document.readyState)) === "complete",
    { timeout: 10000, timeoutMsg: "the relaunched app never finished loading" },
  );
  await waitForChooser();
}

async function diskEncryptionNoteShown(): Promise<boolean> {
  return $('section[aria-label="Disk encryption"]').isExisting();
}

describe("User Story 1 (003) - Protect My Collection With My Own Passphrase", () => {
  const folder = `${scratchDocuments()}/Typed folder`;

  it("welcomes a first run", async () => {
    await waitForChooser();
    await expect($(".hd-chooser__welcome")).toHaveText(
      "HoploDex keeps your collection in an encrypted database file that only your passphrase opens.",
    );
    await expect($("button=Create a new database…")).toExist();
    await expect($("button=Open another database file…")).toExist();
  });

  it("fits the first-run chooser in the window, with no scrollbar", async () => {
    // The plate is as tall as the window less the top bar; a 1px miss made
    // the page always scroll.
    const overflow = await browser.execute(
      () => document.documentElement.scrollHeight - document.documentElement.clientHeight,
    );
    expect(overflow).toBeLessThanOrEqual(0);
  });

  it("draws the catalogue plate in, then cycles its firearm drawings (#23)", async () => {
    // In the app's own engine: the draw-in's CSS animations, and the cycle's
    // Web Animations, which repeat until a database is opened.
    const plate = await browser.execute(() => {
      const svg = document.querySelector(".hd-catalogue");
      const all = svg?.getAnimations({ subtree: true }) ?? [];
      const forever = all.filter((a) => a.effect?.getComputedTiming().iterations === Infinity);
      return {
        hidden: svg?.getAttribute("aria-hidden"),
        drawing: all.length - forever.length,
        cycling: forever.length,
        drawings: [...(svg?.querySelectorAll(".layer") ?? [])].map((l) =>
          l.getAttribute("data-drawing"),
        ),
        numbers: [...(svg?.querySelectorAll(".layer .no") ?? [])].map((n) => n.textContent),
      };
    });
    expect(plate.hidden).toBe("true");
    expect(plate.drawing).toBeGreaterThan(500);
    expect(plate.cycling).toBeGreaterThan(0);
    // shuffled at startup, each keeping its own number
    expect([...plate.drawings].sort()).toEqual([
      "handgun",
      "other",
      "rifle",
      "shotgun",
      "suppressor",
    ]);
    expect(plate.numbers).toEqual(
      plate.drawings.map(
        (d) => ({ rifle: "2", handgun: "3", shotgun: "4", suppressor: "5", other: "6" })[d!],
      ),
    );
  });

  it("blinks the owl when its beak is clicked, once its pupils are in", async () => {
    await settleChooserPlate();
    await $(".hd-catalogue .device .beak").click();
    const blinks = await browser.execute(() =>
      [...document.querySelectorAll(".hd-catalogue .device .pf")].map(
        (pupil) => pupil.getAnimations().filter((a) => !("animationName" in a)).length,
      ),
    );
    expect(blinks).toEqual([1, 1]);
  });

  it("creates a database only with a long enough passphrase and the acknowledgement (FR-003, FR-004)", async () => {
    await clickButton("Create a new database…");
    await $('[role="dialog"]').waitForExist();
    await fill("Name", "Test");
    await fill("Folder", folder);
    await expect($(`p=Saved as ${folder}/Test.hoplodex`)).toExist();

    // Create waits for a long enough passphrase, confirmed, and for the
    // no-recovery acknowledgement.
    await fill("Passphrase", "too short");
    await fill("Confirm passphrase", "too short");
    await toggle(ACKNOWLEDGEMENT);
    await expect($("p=Use at least 12 characters.")).toExist();
    expect(await isButtonDisabled("Create database")).toBe(true);
    expect(fs.existsSync(`${folder}/Test.hoplodex`)).toBe(false);

    await fill("Passphrase", E2E_PASSPHRASE);
    await fill("Confirm passphrase", E2E_PASSPHRASE);
    await toggle(ACKNOWLEDGEMENT);
    expect(await isButtonDisabled("Create database")).toBe(true);
    await toggle(ACKNOWLEDGEMENT);
    expect(await isButtonDisabled("Create database")).toBe(false);
    await clickButton("Create database");
    await waitForCollection();
    expect(fs.existsSync(`${folder}/Test.hoplodex`)).toBe(true);
  });

  it("shows the disk-encryption note until it is dismissed (FR-008)", async () => {
    expect(await diskEncryptionNoteShown()).toBe(true);
    await expect($('section[aria-label="Disk encryption"]')).toHaveText(
      expect.stringContaining("BitLocker on Windows, FileVault on macOS, or LUKS on Linux"),
    );

    await clickEl('section[aria-label="Disk encryption"] button[aria-label="Dismiss"]');
    await $('section[aria-label="Disk encryption"]').waitForExist({ reverse: true });
  });

  it("keeps a firearm that only the right passphrase shows again (FR-005, FR-006)", async () => {
    await addFirearm({
      make: "LockedE2E",
      model: "Model 1",
      caliber: ".38 Special",
      type: "Handgun",
      serial: "LK-001",
    });

    await relaunch();
    // Nothing from the collection shows before the passphrase.
    expect(await $('nav[aria-label="Sections"]').isExisting()).toBe(false);
    expect(await $("body").getText()).not.toContain("LockedE2E");

    await submitPassphrase("not the right passphrase");
    await expect(
      $(
        "p=That passphrase didn't open “Test”. Either the passphrase is wrong, or the file isn't a HoploDex database or is damaged.",
      ),
    ).toExist();
    expect(await $('nav[aria-label="Sections"]').isExisting()).toBe(false);

    await unlock(E2E_PASSPHRASE);
    await goTo("Collection");
    await browser.waitUntil(
      async () => (await listedNames()).some((n) => n.includes("LockedE2E")),
      {
        timeout: 5000,
        timeoutMsg: "the firearm isn't listed after reopening",
      },
    );
    expect(await diskEncryptionNoteShown()).toBe(false);
  });
});

const club = {
  folder: `${scratchDocuments()}/Club`,
  name: "Club",
  passphrase: "club armory passphrase",
};
const home = {
  folder: `${scratchDocuments()}/Home`,
  name: "Home",
  passphrase: "home safe passphrase",
};

async function showsFirearm(make: string): Promise<boolean> {
  await goTo("Collection");
  return (await listedNames()).some((name) => name.includes(make));
}

/** The selected chooser row asks for a passphrase, rather than opening with
 * the saved one. */
async function asksForPassphrase(): Promise<boolean> {
  return $(".hd-db-row--selected input").isExisting();
}

describe("User Story 2 (003) - Keep Several Databases, Anywhere", () => {
  it("keeps two databases with their own passphrases in two folders", async () => {
    await switchDatabase();
    await createDatabase(club);
    await addFirearm({
      make: "ClubGun",
      model: "M1",
      caliber: ".30-06",
      type: "Rifle",
      serial: "CL-1",
    });

    await switchDatabase();
    await createDatabase(home);
    await addFirearm({
      make: "HomeGun",
      model: "P1",
      caliber: "9mm",
      type: "Handgun",
      serial: "HM-1",
    });

    expect(fs.existsSync(`${club.folder}/Club.hoplodex`)).toBe(true);
    expect(fs.existsSync(`${home.folder}/Home.hoplodex`)).toBe(true);
  });

  it("switches through the database menu, listing the most recent first", async () => {
    await switchDatabase();

    expect(await chooserNames()).toEqual(["Home", "Club", "Test"]);
    expect(await selectedChooserRow()).toBe("Home");

    await selectChooserRow("Club");
    await submitPassphrase(home.passphrase);
    await expect(
      $(
        "p=That passphrase didn't open “Club”. Either the passphrase is wrong, or the file isn't a HoploDex database or is damaged.",
      ),
    ).toExist();
    await unlock(club.passphrase);
    expect(await showsFirearm("ClubGun")).toBe(true);
    expect(await showsFirearm("HomeGun")).toBe(false);
  });

  it("removes a database from the list, leaving its file where it is (US2-5)", async () => {
    const file = `${scratchDocuments()}/Typed folder/Test.hoplodex`;
    await switchDatabase();

    await chooseMenuItem('button[aria-label="More actions for Test"]', "Remove from list");

    await browser.waitUntil(async () => !(await chooserNames()).includes("Test"), {
      timeout: 5000,
      timeoutMsg: "Test is still listed",
    });
    expect(await chooserNames()).toEqual(["Club", "Home"]);
    expect(fs.existsSync(file)).toBe(true);
  });

  it("asks save, discard or cancel when quitting with an unsaved firearm form (FR-010)", async () => {
    // "Club" was closed last, so it is selected.
    expect(await selectedChooserRow()).toBe("Club");
    await unlock(club.passphrase);
    await clickButton("Add firearm");
    await $('[role="dialog"]').waitForExist();
    await fill("Make", "Unsaved make");

    const prompt = '[role="alertdialog"]';
    await requestQuit();
    await $(prompt).waitForExist();
    await expect($(prompt)).toHaveText(expect.stringContaining("Save changes to New firearm?"));
    await clickButton("Cancel");
    await $(prompt).waitForExist({ reverse: true });
    expect(await fieldValue("Make")).toBe("Unsaved make");

    // The form isn't complete, so saving fails there and nothing closes.
    await requestQuit();
    await $(prompt).waitForExist();
    await clickButton("Save changes");
    await $(prompt).waitForExist({ reverse: true });
    expect(await fieldValue("Make")).toBe("Unsaved make");
    await expect($("p=Enter the model.")).toExist();

    // Discarding quits; the relaunched app has nothing of the draft.
    await requestQuit();
    await $(prompt).waitForExist();
    // The click quits the app, so it is scheduled for after this script has
    // answered: a click made inside it would take the session down unanswered.
    await browser.execute(() => {
      const discard = [...document.querySelectorAll('[role="alertdialog"] button')].find(
        (button) => button.textContent?.trim() === "Discard changes",
      ) as HTMLElement | undefined;
      setTimeout(() => discard?.click(), 100);
    });
    // The app is gone once WebDriver can no longer reach it, and its close
    // (with the automatic backup) is done.
    await browser.waitUntil(
      async () => {
        try {
          await browser.execute(() => true);
          return false;
        } catch {
          return true;
        }
      },
      { timeout: 15000, timeoutMsg: "the app never quit after discarding" },
    );
    await relaunch();
    expect(await selectedChooserRow()).toBe("Club");
    await unlock(club.passphrase);
    expect(await showsFirearm("Unsaved make")).toBe(false);
    expect(await showsFirearm("ClubGun")).toBe(true);
  });
});

describe("User Story 5 (003) - Optionally Let This Computer Remember My Passphrase", () => {
  const SAVED_NOTE = "Opens without a passphrase on this computer";

  it("remembers a passphrase at open, after the confirmation (FR-017)", async () => {
    await switchDatabase();
    await selectChooserRow("Home");
    expect(await asksForPassphrase()).toBe(true);

    await toggle("Remember on this computer");
    const confirm = '[role="alertdialog"]';
    await $(confirm).waitForExist();
    await expect($(confirm)).toHaveText(
      expect.stringContaining("will be able to open “Home” without knowing the passphrase"),
    );
    await clickButton("Remember passphrase");
    await $(confirm).waitForExist({ reverse: true });

    await unlock(home.passphrase);
    expect(await showsFirearm("HomeGun")).toBe(true);
  });

  it("opens it with Open alone after a relaunch, while another database still asks", async () => {
    await relaunch();
    expect(await selectedChooserRow()).toBe("Home");
    await expect($(".hd-db-row--selected .hd-db-row__saved")).toHaveText(SAVED_NOTE);
    expect(await asksForPassphrase()).toBe(false);

    await selectChooserRow("Club");
    expect(await asksForPassphrase()).toBe(true);

    await selectChooserRow("Home");
    await clickButton("Open");
    await waitForCollection();
    expect(await showsFirearm("HomeGun")).toBe(true);
  });

  it("forgets it from the database settings, and the prompt returns (FR-018)", async () => {
    await chooseMenuItem("button.hd-db-menu", "Database settings…");
    await $('[role="dialog"]').waitForExist();
    await expect($("fieldset*=This computer")).toHaveText(
      expect.stringContaining("The passphrase is remembered on this computer"),
    );
    await clickButton("Forget saved passphrase");
    await expect($("fieldset*=This computer")).toHaveText(
      expect.stringContaining(
        "Not remembered: the database asks for its passphrase each time it opens.",
      ),
    );
    await clickButton("Cancel");
    await $('[role="dialog"]').waitForExist({ reverse: true });

    await relaunch();
    expect(await selectedChooserRow()).toBe("Home");
    expect(await asksForPassphrase()).toBe(true);
    expect(await $(".hd-db-row__saved").isExisting()).toBe(false);
    await unlock(home.passphrase);
  });
});
