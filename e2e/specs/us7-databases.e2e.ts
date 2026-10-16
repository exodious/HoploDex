import fs from "node:fs";
import {
  $,
  E2E_PASSPHRASE,
  addFirearm,
  browser,
  chooseMenuItem,
  clickButton,
  createDatabase,
  expect,
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
import { appPid, relaunchApp, waitForAppToQuit } from "../support/app";
import {
  chooseMenuBarItem,
  pressCommandQ,
  quitMenuItemTitle,
  sendQuitEvent,
  skipWithoutMacInput,
} from "../support/macInput";
import { realClick, skipWithoutRealInput } from "../support/realInput";

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
  await relaunchApp();
  await browser.waitUntil(
    async () => (await browser.execute(() => document.readyState)) === "complete",
    { timeout: 10000, timeoutMsg: "the relaunched app never finished loading" },
  );
  await waitForChooser();
}

/** The error shown for the field labelled `label` in the open dialog: the
 * text of the element its `aria-describedby` names when it is `aria-invalid`,
 * or `null` when the field shows none. */
async function dialogFieldError(label: string): Promise<string | null> {
  return browser.execute((label: string) => {
    const labelEl = [...document.querySelectorAll('[role="dialog"] label')].find(
      (l) => l.textContent?.trim() === label,
    ) as HTMLLabelElement | undefined;
    const field = labelEl && document.getElementById(labelEl.htmlFor);
    if (!field || field.getAttribute("aria-invalid") !== "true") return null;
    for (const id of (field.getAttribute("aria-describedby") ?? "").split(" ")) {
      const el = document.getElementById(id);
      if (el?.classList.contains("hd-field__error")) return el.textContent?.trim() ?? "";
    }
    return null;
  }, label);
}

describe("User Story 1 (003) - Protect My Collection With My Own Passphrase", () => {
  const folder = `${scratchDocuments()}/Typed folder`;

  it("fits the first-run chooser in the window, with no scrollbar, and its plate animates", async () => {
    await waitForChooser();
    // The plate is as tall as the window less the top bar; a 1px miss made
    // the page always scroll.
    const overflow = await browser.execute(
      () => document.documentElement.scrollHeight - document.documentElement.clientHeight,
    );
    expect(overflow).toBeLessThanOrEqual(0);

    // Smoke check of the catalogue plate (#23) in the app's own engine: the
    // draw-in's CSS animations and the cycle's Web Animations, which repeat
    // until a database is opened. Its structure and timing are in
    // CataloguePlate.test.tsx.
    const plate = await browser.execute(() => {
      const all = document.querySelector(".hd-catalogue")?.getAnimations({ subtree: true }) ?? [];
      const forever = all.filter((a) => a.effect?.getComputedTiming().iterations === Infinity);
      return { drawing: all.length - forever.length, cycling: forever.length };
    });
    expect(plate.drawing).toBeGreaterThan(0);
    expect(plate.cycling).toBeGreaterThan(0);
  });

  it("blinks the owl when its beak is clicked, once its pupils are in", async function () {
    skipWithoutRealInput(this);
    await settleChooserPlate();
    // A real pointer click: it also shows the beak is what the pointer hits
    // there. (WebDriver's own click is `element.click()`, which an SVG
    // element doesn't have.)
    await realClick(".hd-catalogue .device .beak");
    const blinks = await browser.execute(() =>
      [...document.querySelectorAll(".hd-catalogue .device .pf")].map(
        (pupil) => pupil.getAnimations().filter((a) => !("animationName" in a)).length,
      ),
    );
    expect(blinks).toEqual([1, 1]);
  });

  it("keeps the acknowledgement and Create in view when a weak passphrase's warning shows (#41)", async () => {
    await clickButton("Create a new database…");
    await $('[role="dialog"]').waitForExist();
    await fill("Name", "Weak");
    await fill("Folder", folder);
    const inView = () =>
      browser.execute((acknowledgement: string) => {
        const dialog = document.querySelector<HTMLElement>('[role="dialog"]')!;
        const label = [...dialog.querySelectorAll("label")].find(
          (l) => l.textContent?.trim() === acknowledgement,
        );
        const checkbox = document.getElementById(label?.htmlFor ?? "");
        const create = [...dialog.querySelectorAll("button")].find(
          (b) => b.textContent?.trim() === "Create database",
        );
        // Visible without scrolling: inside the window, the dialog, and every
        // scrolling box between the element and the dialog.
        const visible = (el: Element | null | undefined) => {
          if (!el) return false;
          const box = el.getBoundingClientRect();
          const page = new DOMRect(0, 0, window.innerWidth, window.innerHeight);
          const within = (outer: DOMRect) =>
            box.top >= outer.top - 1 &&
            box.bottom <= outer.bottom + 1 &&
            box.left >= outer.left - 1 &&
            box.right <= outer.right + 1;
          if (!within(page) || !within(dialog.getBoundingClientRect())) return false;
          for (let up = el.parentElement; up && up !== dialog; up = up.parentElement) {
            const overflow = getComputedStyle(up).overflowY;
            if (
              (overflow === "auto" || overflow === "scroll") &&
              !within(up.getBoundingClientRect())
            )
              return false;
          }
          return true;
        };
        const scrolled = [dialog, ...dialog.querySelectorAll<HTMLElement>("*")].some(
          (el) => el.scrollTop > 0,
        );
        return {
          checkbox: visible(checkbox),
          create: visible(create),
          scrolled,
          suggestion:
            dialog.querySelector('[data-testid="strength-suggestion"]')?.textContent?.trim() ?? "",
        };
      }, ACKNOWLEDGEMENT);

    // Where the acknowledgement and Create are, to tell whether the warning
    // moves them: the dialog is tall enough here that they stay in view
    // either way, so what shows the bug is that they are pushed down.
    const tops = () =>
      browser.execute((acknowledgement: string) => {
        const dialog = document.querySelector<HTMLElement>('[role="dialog"]')!;
        const label = [...dialog.querySelectorAll("label")].find(
          (l) => l.textContent?.trim() === acknowledgement,
        );
        const create = [...dialog.querySelectorAll("button")].find(
          (b) => b.textContent?.trim() === "Create database",
        );
        return {
          checkbox: label?.getBoundingClientRect().top ?? -1,
          create: create?.getBoundingClientRect().top ?? -1,
        };
      }, ACKNOWLEDGEMENT);

    // Before any warning (and before anything is typed, when nothing is shown
    // under the field), then with a weak (but long enough) passphrase's.
    const empty = await tops();
    await fill("Passphrase", "p");
    await fill("Confirm passphrase", "p");
    const before = await inView();
    expect(before.checkbox).toBe(true);
    expect(before.create).toBe(true);

    await fill("Passphrase", "passwordpassword");
    await fill("Confirm passphrase", "passwordpassword");
    await browser.waitUntil(async () => (await inView()).suggestion !== "", {
      timeout: 10000,
      timeoutMsg: "the weak passphrase's warning never showed",
    });
    expect(await inView()).toEqual({
      checkbox: true,
      create: true,
      scrolled: false,
      suggestion: expect.stringMatching(/\S/),
    });
    // The warning took its place in the layout without moving anything.
    const after = await tops();
    expect(Math.abs(after.checkbox - empty.checkbox)).toBeLessThanOrEqual(1);
    expect(Math.abs(after.create - empty.create)).toBeLessThanOrEqual(1);
    await clickButton("Cancel");
    await $('[role="dialog"]').waitForExist({ reverse: true });
  });

  // The create dialog to the collection, M2 (the backend's refusal lands on the
  // name field) and the relaunch that shows nothing before the passphrase.
  // The dialog's own rules (Saved as, 12 characters, matching, the
  // acknowledgement) are in CreateDatabaseDialog.test.tsx.
  it("creates a database, keeps a firearm that only the right passphrase shows again (FR-003, FR-005, FR-006)", async () => {
    // A file already at the name asked for; the backend refuses the create.
    fs.mkdirSync(folder, { recursive: true });
    fs.writeFileSync(`${folder}/Taken.hoplodex`, "");

    await clickButton("Create a new database…");
    await $('[role="dialog"]').waitForExist();
    await fill("Name", "Taken");
    await fill("Folder", folder);
    await fill("Passphrase", E2E_PASSPHRASE);
    await fill("Confirm passphrase", E2E_PASSPHRASE);
    await toggle(ACKNOWLEDGEMENT);
    await clickButton("Create database");
    await browser.waitUntil(async () => (await dialogFieldError("Name")) !== null, {
      timeout: 10000,
      timeoutMsg: "the refusal never landed on the Name field",
    });
    expect(await dialogFieldError("Name")).toBe(
      "A file with this name already exists in that folder.",
    );
    expect(await dialogFieldError("Folder")).toBeNull();
    // Nothing was created, and the file that was there is untouched.
    expect(fs.readFileSync(`${folder}/Taken.hoplodex`, "utf8")).toBe("");
    expect(await $('nav[aria-label="Sections"]').isExisting()).toBe(false);

    // Another name, and the passphrases typed again (they are cleared on a
    // refusal), creates it.
    await fill("Name", "Test");
    expect(await dialogFieldError("Name")).toBeNull();
    await fill("Passphrase", E2E_PASSPHRASE);
    await fill("Confirm passphrase", E2E_PASSPHRASE);
    expect(await isButtonDisabled("Create database")).toBe(false);
    await clickButton("Create database");
    await waitForCollection();
    expect(fs.existsSync(`${folder}/Test.hoplodex`)).toBe(true);

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
  it("keeps two databases with their own passphrases in two folders, and switches between them (US2-1, US2-4)", async () => {
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

    // Switching to the first: only its own firearms show.
    await switchDatabase();
    await selectChooserRow("Club");
    await unlock(club.passphrase);
    expect(await showsFirearm("ClubGun")).toBe(true);
    expect(await showsFirearm("HomeGun")).toBe(false);
  });

  it("asks the same question for ⌘Q, the macOS menu's Quit and the Dock's Quit, and Cancel keeps the form as it was (FR-010, #107)", async function () {
    skipWithoutMacInput(this);
    // macOS's own Quit (`terminate:`: ⌘Q, the menu, the Dock, an Apple
    // Event) used to end the process without a request, so unsaved input
    // was lost. "Club" is open from the switch above.
    await clickButton("Add firearm");
    await $('[role="dialog"]').waitForExist();
    await fill("Make", "Unsaved make");

    const prompt = '[role="alertdialog"]';
    for (const quit of [
      () => pressCommandQ(appPid()),
      () => chooseMenuBarItem(appPid(), quitMenuItemTitle(appPid())),
      () => sendQuitEvent(appPid()),
    ]) {
      quit();
      await $(prompt).waitForExist();
      await expect($(prompt)).toHaveText(expect.stringContaining("Save changes to New firearm?"));
      await clickButton("Cancel");
      await $(prompt).waitForExist({ reverse: true });
      // Still running, with the form and what was typed in it.
      await expect($('[role="dialog"]')).toExist();
      expect(
        await browser.execute(
          () => (document.querySelector('[role="dialog"] input') as HTMLInputElement).value,
        ),
      ).toBe("Unsaved make");
    }

    // Quitting for real, through the Dock's route and the question's Discard
    // (`quit_application` must still end the process), so the next test starts
    // from a clean open of "Club".
    sendQuitEvent(appPid());
    await $(prompt).waitForExist();
    await browser.execute(() => {
      const discard = [...document.querySelectorAll('[role="alertdialog"] button')].find(
        (button) => button.textContent?.trim() === "Discard changes",
      ) as HTMLElement | undefined;
      setTimeout(() => discard?.click(), 100);
    });
    await waitForAppToQuit(15000);
    await relaunch();
    expect(await selectedChooserRow()).toBe("Club");
    await unlock(club.passphrase);
    expect(await showsFirearm("Unsaved make")).toBe(false);
  });

  it("asks save, discard or cancel when quitting with an unsaved firearm form, and Discard quits (FR-010)", async () => {
    // "Club" is open from the switch above. The prompt's buttons, Cancel and a
    // Save that fails validation are in SessionProvider.test.tsx; here the
    // real quit request, the process ending and the relaunch.
    await clickButton("Add firearm");
    await $('[role="dialog"]').waitForExist();
    await fill("Make", "Unsaved make");

    const prompt = '[role="alertdialog"]';
    await requestQuit();
    await $(prompt).waitForExist();
    await expect($(prompt)).toHaveText(expect.stringContaining("Save changes to New firearm?"));
    // The click quits the app, so it is scheduled for after this script has
    // answered: a click made inside it would take the session down unanswered.
    await browser.execute(() => {
      const discard = [...document.querySelectorAll('[role="alertdialog"] button')].find(
        (button) => button.textContent?.trim() === "Discard changes",
      ) as HTMLElement | undefined;
      setTimeout(() => discard?.click(), 100);
    });
    // Once the app's process has ended, its close (with the automatic
    // backup) is done.
    await waitForAppToQuit(15000);
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

  it("only stages forgetting it in the database settings: Cancel leaves it saved (FR-018, #34)", async () => {
    await chooseMenuItem("button.hd-db-menu", "Database settings…");
    await $('[role="dialog"]').waitForExist();
    await expect($("fieldset*=This computer")).toHaveText(
      expect.stringContaining("The passphrase is remembered on this computer"),
    );
    // Nothing differs yet, so there is nothing to save.
    await expect($('[role="dialog"] button[type="submit"]')).toBeDisabled();
    await toggle("Remember the passphrase on this computer");
    await expect($("fieldset*=This computer")).toHaveText(
      expect.stringContaining("The saved passphrase will be forgotten when you click Save."),
    );
    await expect($('[role="dialog"] button[type="submit"]')).toBeEnabled();
    await clickButton("Cancel");
    await $('[role="dialog"]').waitForExist({ reverse: true });

    await relaunch();
    expect(await selectedChooserRow()).toBe("Home");
    await expect($(".hd-db-row--selected .hd-db-row__saved")).toHaveText(SAVED_NOTE);
    expect(await asksForPassphrase()).toBe(false);
    await clickButton("Open");
    await waitForCollection();
  });

  it("forgets it with Save, and the prompt returns (FR-018, #34)", async () => {
    await chooseMenuItem("button.hd-db-menu", "Database settings…");
    await $('[role="dialog"]').waitForExist();
    await toggle("Remember the passphrase on this computer");
    await clickButton("Save");
    await $('[role="dialog"]').waitForExist({ reverse: true });

    await relaunch();
    expect(await selectedChooserRow()).toBe("Home");
    expect(await asksForPassphrase()).toBe(true);
    expect(await $(".hd-db-row__saved").isExisting()).toBe(false);
    await unlock(home.passphrase);
  });

  it("remembers it again from the database settings, asking for the passphrase only at Save (FR-017, #34)", async () => {
    await chooseMenuItem("button.hd-db-menu", "Database settings…");
    await $('[role="dialog"]').waitForExist();
    await toggle("Remember the passphrase on this computer");
    // Ticking asks for nothing: the passphrase is asked for at Save.
    expect(await $('[role="alertdialog"]').isExisting()).toBe(false);
    await clickButton("Save");
    await $('[role="alertdialog"]').waitForExist();
    await clickButton("Cancel");
    await $('[role="alertdialog"]').waitForExist({ reverse: true });
    // Cancelled: back in the form with nothing saved and the choice still staged.
    await expect($('[role="dialog"]')).toExist();

    await clickButton("Save");
    await $('[role="alertdialog"]').waitForExist();
    await fill("Passphrase", home.passphrase);
    await clickButton("Remember passphrase");
    await $('[role="dialog"]').waitForExist({ reverse: true });

    await relaunch();
    expect(await selectedChooserRow()).toBe("Home");
    expect(await asksForPassphrase()).toBe(false);
    await clickButton("Open");
    await waitForCollection();
  });
});
