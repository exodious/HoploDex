import { randomBytes } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
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
  pressEscape,
  requestQuit,
  scratchDocuments,
  selectChooserRow,
  selectedChooserRow,
  settle,
  settleChooserPlate,
  submitPassphrase,
  switchDatabase,
  toggle,
  unlock,
  waitForChooser,
  waitForCollection,
} from "../support/ui";
import { appPid, killApp, launchApp, relaunchApp, waitForAppToQuit } from "../support/app";
import { repoRoot } from "../support/build";
import { copyReleaseFixture, sha256Of } from "../support/sandbox";
import {
  chooseMenuBarItem,
  pressCommandQ,
  quitMenuItemTitle,
  sendQuitEvent,
  skipWithoutMacInput,
} from "../support/macInput";
import { HAS_REAL_INPUT, realClick, realKey, skipWithoutRealInput } from "../support/realInput";

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

describe("003 US1 - Protect My Collection With My Own Passphrase (was us7-databases)", () => {
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
  it("creates a database, keeps a firearm that only the right passphrase shows again (003 FR-003, FR-005, FR-006)", async () => {
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

describe("003 US2 - Keep Several Databases, Anywhere, and Take Them to Another Computer (was us7-databases)", () => {
  it("keeps two databases with their own passphrases in two folders, and switches between them (003 US2-1, US2-4)", async () => {
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

  it("asks the same question for ⌘Q, the macOS menu's Quit and the Dock's Quit, and Cancel keeps the form as it was (003 FR-010, #107)", async function () {
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

  it("asks save, discard or cancel when quitting with an unsaved firearm form, and Discard quits (003 FR-010)", async () => {
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

describe("003 US5 - Optionally Let This Computer Remember My Passphrase (was us7-databases)", () => {
  const SAVED_NOTE = "Opens without a passphrase on this computer";

  it("remembers a passphrase at open, after the confirmation (003 FR-017)", async () => {
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

  it("only stages forgetting it in the database settings: Cancel leaves it saved (003 FR-018, #34)", async () => {
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

  it("forgets it with Save, and the prompt returns (003 FR-018, #34)", async () => {
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

  it("remembers it again from the database settings, asking for the passphrase only at Save (003 FR-017, #34)", async () => {
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

// 009 US3: a database the previous release made is stored in the repository
// (src-tauri/tests/fixtures/releases/0.1.0.hoplodex), and an E2E build has one
// more step than it (009 research.md §12), so opening a copy asks the question.
// The dialogs' texts, roles and keys are in 009 contracts/ui.md §1, §3, §5 and
// §7; their parts are in UpgradeDialogs.test.tsx.
describe("009 US3 - A Beta Tester's Database Survives an Upgrade", () => {
  const FIXTURE_PASSPHRASE = "release fixture 0.1.0";
  const version = (
    JSON.parse(fs.readFileSync(path.join(repoRoot, "package.json"), "utf8")) as {
      version: string;
    }
  ).version;
  const identifier = (
    JSON.parse(fs.readFileSync(path.join(repoRoot, "src-tauri", "tauri.conf.json"), "utf8")) as {
      identifier: string;
    }
  ).identifier;
  const olderName = "Older collection";
  const strandedName = "Older no backups";
  let older: { path: string; sha256: string; folder: string };
  let stranded: { path: string; sha256: string; folder: string };

  /** The text of the open dialog or alert dialog, whitespace collapsed. */
  async function dialogText(selector: string): Promise<string> {
    return browser.execute(
      (selector: string) =>
        (document.querySelector(selector)?.textContent ?? "").replace(/\s+/g, " ").trim(),
      selector,
    );
  }

  /** The text of the button that has focus, or "" when it isn't a button. */
  async function focusedButton(): Promise<string> {
    return browser.execute(() => {
      const active = document.activeElement;
      return active instanceof HTMLButtonElement ? (active.textContent ?? "").trim() : "";
    });
  }

  /** Whether the chooser is up with `name` selected and says nothing about
   * a failure. */
  async function chooserIsQuiet(name: string): Promise<boolean> {
    await waitForChooser();
    return (
      (await selectedChooserRow()) === name &&
      !(await $('nav[aria-label="Sections"]').isExisting()) &&
      !(await $('.hd-db-row--selected [role="alert"]').isExisting()) &&
      !(await $("body").getText()).includes("HoploDex couldn't")
    );
  }

  /** Puts the two copies on the recent list, which the app reads at start:
   * the native file picker that would add them can't be driven. */
  function registerRecent(entries: { path: string; name: string }[]) {
    const dir = path.join(process.env.HOPLODEX_E2E_CONFIG_HOME!, identifier);
    const file = path.join(dir, "machine.json");
    fs.mkdirSync(dir, { recursive: true });
    const machine = fs.existsSync(file)
      ? (JSON.parse(fs.readFileSync(file, "utf8")) as { recentDatabases?: unknown[] })
      : { version: 1, machineId: randomBytes(16).toString("hex"), recentDatabases: [] };
    machine.recentDatabases = [
      ...entries.map((entry) => ({
        ...entry,
        lastOpenedAt: new Date().toISOString(),
        databaseId: null,
        backupFolder: null,
        passphraseSaved: false,
        leftModifiedAt: null,
        located: false,
      })),
      ...(machine.recentDatabases ?? []),
    ];
    fs.writeFileSync(file, JSON.stringify(machine, null, 2));
  }

  /** Opens the selected chooser row with the fixture's passphrase, and waits
   * for the upgrade question. */
  async function openUntilQuestion() {
    await submitPassphrase(FIXTURE_PASSPHRASE);
    await $('[role="alertdialog"]').waitForExist({ timeout: 10000 });
    await settle();
  }

  before(async () => {
    // Leave a database an earlier test left open, as the menu does.
    if (await $('nav[aria-label="Sections"]').isExisting()) await switchDatabase();

    const documents = scratchDocuments();
    const first = copyReleaseFixture(path.join(documents, "Older"), olderName);
    older = { ...first, folder: path.join(documents, "Older") };
    // Where this copy's "HoploDex backups" folder would be there is a file.
    const second = copyReleaseFixture(path.join(documents, "Stranded"), strandedName);
    stranded = { ...second, folder: path.join(documents, "Stranded") };
    fs.writeFileSync(path.join(stranded.folder, "HoploDex backups"), "not a folder");

    // The app reads machine.json when it starts: end it, add the two
    // copies to the list, and start it again.
    await killApp();
    registerRecent([
      { path: older.path, name: olderName },
      { path: stranded.path, name: strandedName },
    ]);
    await launchApp();
    await browser.reloadSession();
    await browser.waitUntil(
      async () => (await browser.execute(() => document.readyState)) === "complete",
      { timeout: 10000, timeoutMsg: "the restarted app never finished loading" },
    );
    await waitForChooser();
  });

  it("asks before upgrading, with Upgrade focused, and Cancel leaves the file as it was (009 FR-010)", async () => {
    // The first row is selected when the chooser opens, and a selected row has
    // no button to select it with.
    if ((await selectedChooserRow()) !== olderName) await selectChooserRow(olderName);
    await openUntilQuestion();

    const question = await dialogText('[role="alertdialog"]');
    expect(question).toContain(`Upgrade “${olderName}” for HoploDex ${version}?`);
    expect(question).toContain(
      `“${olderName}” was made by an earlier version of HoploDex. To open it, this version needs to upgrade it.`,
    );
    expect(question).toContain(
      `First, HoploDex saves a backup of it as it is now, in ${path.join(older.folder, "HoploDex backups")}.`,
    );
    expect(question).toContain(
      `After the upgrade, earlier versions of HoploDex can open that backup, but not “${olderName}” itself.`,
    );
    expect(await focusedButton()).toBe("Upgrade");

    await clickButton("Cancel");
    await $('[role="alertdialog"]').waitForExist({ reverse: true });
    expect(await chooserIsQuiet(olderName)).toBe(true);
    expect(sha256Of(older.path)).toBe(older.sha256);
  });

  it("upgrades on Enter, opens the collection with the note, and lists the backup made first (009 FR-017, FR-013)", async () => {
    await openUntilQuestion();
    expect(await focusedButton()).toBe("Upgrade");
    // Enter submits the question's form. The embedded WebDriver's key presses
    // are synthetic and submit nothing, so where real input exists it is used
    // (DEVELOPMENT.md, "Real keyboard and mouse input"); elsewhere the button
    // stands in for it.
    if (HAS_REAL_INPUT) await realKey("Return");
    else await clickButton("Upgrade");

    // The copy was locked mid-edit by the earlier version (a policy form half
    // typed): that change survived the upgrade (009 FR-015) and is offered
    // before the collection, as 003 FR-039 has it for any open. Discard it
    // to reach the collection.
    await $('[role="dialog"]').waitForExist({ timeout: 20000 });
    await settle();
    expect(await dialogText('[role="dialog"]')).toContain("Unsaved changes to Range Rider (add)");
    await clickButton("Discard changes");
    await $('[role="alertdialog"]').waitForExist();
    await clickButton("Discard changes");
    await waitForCollection();

    // The note, once, in the notes area (009 contracts/ui.md §5).
    const note = await browser.execute(
      () =>
        [...document.querySelectorAll(".hd-db-note")]
          .map((n) => (n.textContent ?? "").replace(/\s+/g, " ").trim())
          .find((text) => text.includes("was upgraded for HoploDex")) ?? "",
    );
    expect(note).toContain(`“${olderName}” was upgraded for HoploDex ${version}.`);
    const named = note.match(
      /A backup of it from before the upgrade is (.+\.hoplodex)\. Earlier versions of HoploDex can open that backup, but not the upgraded database\./,
    );
    expect(named).not.toBeNull();
    const backup = named![1];
    // It is the backup the question promised, made before the file changed.
    expect(path.resolve(path.dirname(backup))).toBe(
      path.resolve(path.join(older.folder, "HoploDex backups")),
    );
    expect(path.basename(backup)).toMatch(
      new RegExp(`^${olderName} .* before ${version}\\.hoplodex$`),
    );
    expect(fs.existsSync(backup)).toBe(true);
    // The folder is the backup location, so the extra sentence is absent.
    expect(note).not.toContain("isn't your backup location");

    // The collection is there.
    await goTo("Collection");
    expect((await listedNames()).length).toBeGreaterThan(0);

    // The restore dialog lists the backup as one made before the upgrade
    // (009 contracts/ui.md §7).
    await chooseMenuItem("button.hd-db-menu", "Restore from a backup…");
    await $('[role="dialog"] input[type="radio"]').waitForExist();
    await settle();
    const listed = await browser.execute(() =>
      [...document.querySelectorAll<HTMLInputElement>('[role="dialog"] input[type="radio"]')].map(
        (radio) => (radio.labels?.[0]?.textContent ?? "").replace(/\s+/g, " ").trim(),
      ),
    );
    expect(
      listed.some((label) => label.endsWith(` — before upgrading to HoploDex ${version}`)),
    ).toBe(true);
    await clickButton("Cancel");
    await $('[role="dialog"]').waitForExist({ reverse: true });

    // The upgrade changed the file.
    await switchDatabase();
    expect(sha256Of(older.path)).not.toBe(older.sha256);
  });

  it("offers another folder when the backup can't be saved, and Cancel leaves the file as it was (009 FR-012)", async () => {
    await selectChooserRow(strandedName);
    await openUntilQuestion();
    expect(await focusedButton()).toBe("Upgrade");
    await clickButton("Upgrade");

    await $('[role="dialog"]').waitForExist({ timeout: 10000 });
    await settle();
    const refusal = await dialogText('[role="dialog"]');
    expect(refusal).toContain("Save the backup somewhere else?");
    expect(refusal).toContain(
      `HoploDex couldn't save the backup it makes before upgrading “${strandedName}” in ${path.join(stranded.folder, "HoploDex backups")}:`,
    );
    expect(refusal).toContain(
      `You can save this one backup in a folder you choose instead, or cancel. “${strandedName}” isn't upgraded without a backup.`,
    );
    expect(await focusedButton()).toBe("Choose a folder…");
    // Choosing a folder opens the OS's picker, which can't be driven here
    // (UpgradeDialogs.test.tsx mocks it; upgrade_test.rs covers the command).

    await clickButton("Cancel");
    await $('[role="dialog"]').waitForExist({ reverse: true });
    expect(await chooserIsQuiet(strandedName)).toBe(true);
    expect(sha256Of(stranded.path)).toBe(stranded.sha256);
    expect(fs.readFileSync(path.join(stranded.folder, "HoploDex backups"), "utf8")).toBe(
      "not a folder",
    );
  });
});

// 010 US1: About HoploDex, from the chooser and from an open database. It reads
// no database or setting (010 SC-005), so the journey takes the SHA-256 of a
// database file and of machine.json around it. The notices are the
// `dist/notices.json` the E2E build embedded (the generator writes it for this
// build's own target), so the component counts and the per-OS components are
// checked against what this OS ships (010 SC-001, 010 FR-009).
describe("010 US1 - Anyone Running HoploDex Can See Its License and What It Is Built From", () => {
  const SOURCE_URL = "https://github.com/exodious/HoploDex";
  const FILTER = "Find a component";
  const DATABASE = "About";
  const version = (
    JSON.parse(fs.readFileSync(path.join(repoRoot, "package.json"), "utf8")) as {
      version: string;
    }
  ).version;
  const identifier = (
    JSON.parse(fs.readFileSync(path.join(repoRoot, "src-tauri", "tauri.conf.json"), "utf8")) as {
      identifier: string;
    }
  ).identifier;
  // Read in `before`, so a build without notices fails this journey alone.
  let notices: { components: { name: string }[] };
  let total = 0;
  const lockKeys = {
    key: "l",
    ctrlKey: process.platform !== "darwin",
    metaKey: process.platform === "darwin",
  };

  /** The count line the dialog shows when `query` leaves `shown` of the
   * components (010 contracts/ui.md §2). */
  function countLine(query: string): string {
    const needle = query.trim().toLowerCase();
    const shown = notices.components.filter((c) => c.name.toLowerCase().includes(needle)).length;
    const noun = total === 1 ? "component" : "components";
    return shown === total ? `${total} ${noun}` : `${shown} of ${total} ${noun}`;
  }

  /** The visible text of the open dialog, whitespace collapsed. */
  async function aboutText(): Promise<string> {
    return browser.execute(() =>
      (document.querySelector('[role="dialog"]')?.textContent ?? "").replace(/\s+/g, " ").trim(),
    );
  }

  /** Waits for About's component list: the notices have loaded. */
  async function waitForAbout() {
    await $('[role="dialog"] .hd-about__list').waitForExist({ timeout: 10000 });
    await settle();
  }

  /** The listed components' titles and summaries, in order. */
  async function listed(): Promise<{ name: string; summary: string }[]> {
    return browser.execute(() =>
      [...document.querySelectorAll('[role="dialog"] .hd-about__list .hd-disclosure')].map(
        (row) => ({
          name: row.querySelector(".hd-disclosure__title")?.textContent?.trim() ?? "",
          summary: row.querySelector(".hd-disclosure__summary")?.textContent?.trim() ?? "",
        }),
      ),
    );
  }

  /** The text of the button that has focus, or "" when it isn't a button. */
  async function focusedButton(): Promise<string> {
    return browser.execute(() => {
      const active = document.activeElement;
      return active instanceof HTMLButtonElement ? (active.textContent ?? "").trim() : "";
    });
  }

  /** Types `query` into the filter in one input event, as a paste would, and
   * returns the milliseconds until the count line shows the result (010
   * SC-004), measured in the page, or -1 when it never did. */
  async function filterBy(query: string): Promise<number> {
    const elapsed = await browser.execute(
      async (query: string, expected: string, field: string) => {
        const label = [...document.querySelectorAll('[role="dialog"] label')].find(
          (l) => l.textContent?.trim() === field,
        ) as HTMLLabelElement;
        const input = document.getElementById(label.htmlFor) as HTMLInputElement;
        const started = performance.now();
        Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(
          input,
          query,
        );
        input.dispatchEvent(new Event("input", { bubbles: true }));
        return new Promise<number>((resolve) => {
          const check = () => {
            const shown = document.querySelector(".hd-about__count")?.textContent?.trim();
            if (shown === expected) resolve(performance.now() - started);
            else if (performance.now() - started > 5000) resolve(-1);
            else requestAnimationFrame(check);
          };
          check();
        });
      },
      query,
      countLine(query),
      FILTER,
    );
    await settle();
    expect(elapsed).toBeGreaterThanOrEqual(0);
    expect(elapsed).toBeLessThan(500);
    return elapsed;
  }

  /** Opens or closes the disclosure titled `title` in the open dialog, with
   * one click on its button. */
  async function toggleDisclosure(title: string) {
    const clicked = await browser.execute((wanted: string) => {
      const trigger = [
        ...document.querySelectorAll<HTMLElement>('[role="dialog"] .hd-disclosure__trigger'),
      ].find((t) => t.querySelector(".hd-disclosure__title")?.textContent?.trim() === wanted);
      trigger?.click();
      return Boolean(trigger);
    }, title);
    if (!clicked) throw new Error(`no disclosure "${title}"`);
    await settle();
  }

  /** Presses Ctrl/⌘+L on whatever has focus, as the user would. */
  async function pressLockShortcut() {
    await browser.execute((keys) => {
      (document.activeElement ?? document.body).dispatchEvent(
        new KeyboardEvent("keydown", { ...keys, bubbles: true, cancelable: true }),
      );
    }, lockKeys);
  }

  before(async () => {
    // The notices the E2E build embedded (the build writes them for its own
    // target), as they are in the repository's dist/.
    notices = JSON.parse(fs.readFileSync(path.join(repoRoot, "dist", "notices.json"), "utf8"));
    total = notices.components.length;
    // The chooser, whatever an earlier journey left.
    if (await $('nav[aria-label="Sections"]').isExisting()) await switchDatabase();
    await waitForChooser();
  });

  it("opens from the chooser in one action, with the statements, the version, the source address and every component (010 US1-1, SC-001, SC-003, SC-004)", async () => {
    // One action: the chooser's button. The list is timed from the click to
    // the first frame that holds it, the notices' first load included.
    const opened = await browser.execute(async () => {
      const button = [...document.querySelectorAll("button")].find(
        (b) => b.textContent?.trim() === "About HoploDex",
      );
      if (!button) return -2;
      const started = performance.now();
      button.click();
      return new Promise<number>((resolve) => {
        const check = () => {
          if (document.querySelector('[role="dialog"] .hd-about__list .hd-disclosure')) {
            resolve(performance.now() - started);
          } else if (performance.now() - started > 5000) resolve(-1);
          else requestAnimationFrame(check);
        };
        check();
      });
    });
    expect(opened).toBeGreaterThanOrEqual(0);
    expect(opened).toBeLessThan(1000); // 010 SC-004
    await waitForAbout();
    await expect($('[role="dialog"] .hd-dialog__title')).toHaveText("About HoploDex");

    // Focus lands on the license disclosure's button once the notices load.
    expect(await focusedButton()).toContain("GNU General Public License, version 3");

    const text = await aboutText();
    expect(text).toContain("HoploDex");
    expect(text).toContain(`Version ${version}`);
    expect(text).toMatch(/Copyright © 2026(–\d{4})? exodious and the HoploDex contributors/);
    expect(text).toContain(
      "HoploDex is free software: you can share it and change it under the terms of the GNU General Public License, version 3 only, as published by the Free Software Foundation.",
    );
    expect(text).toContain(
      "It comes with no warranty, to the extent permitted by law. See the license for details.",
    );
    expect(text).toContain("Additional permission under GNU GPL version 3 section 7");

    // The source address is one selection: a click on it selects all of it
    // (`user-select: all`), and it is the address, as text, not a link.
    expect(await $(".hd-about__source").getText()).toBe(SOURCE_URL);
    await $(".hd-about__source").click();
    expect(await browser.execute(() => window.getSelection()?.toString().trim())).toBe(SOURCE_URL);
    expect(await $('[role="dialog"] a[href]').isExisting()).toBe(false);

    // Every component of the build's notices, and no more (010 SC-001).
    expect(await $(".hd-about__count").getText()).toBe(`${total} components`);
    expect((await listed()).map((row) => row.name)).toEqual(
      notices.components.map((component) => component.name),
    );
  });

  it("shows the license in a second action, and SQLCipher's notice with Zetetic's copyright (010 US1-3, US1-4, SC-003)", async () => {
    await toggleDisclosure("GNU General Public License, version 3");
    const gpl = await $('[role="region"][aria-label="GNU General Public License text"]');
    await gpl.waitForExist();
    expect((await gpl.getText()).trim().startsWith("GNU GENERAL PUBLIC LICENSE")).toBe(true);
    await toggleDisclosure("GNU General Public License, version 3");

    await filterBy("sqlcipher");
    const rows = await listed();
    expect(rows.length).toBeGreaterThan(0);
    const sqlcipher = rows.find((row) => row.name.toLowerCase() === "sqlcipher");
    expect(sqlcipher).toBeDefined();
    await toggleDisclosure(sqlcipher!.name);
    const region = await $(`[role="region"][aria-label="${sqlcipher!.name} license text"]`);
    await region.waitForExist();
    expect(await region.getText()).toContain("Zetetic LLC");
    await toggleDisclosure(sqlcipher!.name);

    await filterBy("zzzz");
    expect(await $("[role='dialog']").getText()).toContain("No component matches “zzzz”.");
    expect(await $('[role="dialog"] .hd-about__list').isExisting()).toBe(false);

    await filterBy("");
    expect(await $(".hd-about__count").getText()).toBe(`${total} components`);
    expect((await listed()).length).toBe(total);
  });

  it("lists the components this OS ships, and not another's (010 SC-001, FR-009)", async () => {
    const none = async (query: string) => {
      await filterBy(query);
      expect(await aboutText()).toContain(`No component matches “${query}”.`);
    };
    const found = async (query: string) => {
      await filterBy(query);
      const rows = await listed();
      expect(rows.length).toBeGreaterThan(0);
      return rows;
    };

    if (process.platform === "linux") {
      expect((await found("webkit2gtk")).map((row) => row.name)).toContain("webkit2gtk");
      await none("objc2");
    } else if (process.platform === "darwin") {
      expect((await found("objc2")).map((row) => row.name)).toContain("objc2");
      await none("webkit2gtk");
    } else {
      const openssl = (await found("openssl")).find((row) => row.name.toLowerCase() === "openssl");
      expect(openssl).toBeDefined();
      expect(openssl!.summary).toMatch(/^3\./);
      expect(openssl!.summary).toContain("Apache-2.0");
      const loader = (await found("webview2")).find(
        (row) => row.name === "Microsoft WebView2 loader",
      );
      expect(loader).toBeDefined();
      expect(loader!.summary).toContain("BSD-3-Clause");
      await none("webkit2gtk");
    }
  });

  it("scrolls the whole list with no frame over 100 ms (010 SC-004)", async () => {
    await filterBy("");
    expect((await listed()).length).toBe(total);
    // One viewport height per animation frame, from the top to the bottom.
    const { longest, frames } = await browser.execute(async () => {
      const body = document.querySelector<HTMLElement>(".hd-dialog__body")!;
      body.scrollTop = 0;
      return new Promise<{ longest: number; frames: number }>((resolve) => {
        let last = performance.now();
        let longest = 0;
        let frames = 0;
        const step = () => {
          const now = performance.now();
          longest = Math.max(longest, now - last);
          last = now;
          frames += 1;
          if (body.scrollTop + body.clientHeight >= body.scrollHeight - 1) {
            resolve({ longest, frames });
            return;
          }
          body.scrollTop += body.clientHeight;
          requestAnimationFrame(step);
        };
        requestAnimationFrame(step);
      });
    });
    expect(frames).toBeGreaterThan(1);
    expect(longest).toBeLessThan(100);
  });

  it("clears the filter with the first Escape, closes with the second, and returns focus to the button (010 US1-5)", async () => {
    await browser.execute((field: string) => {
      const label = [...document.querySelectorAll('[role="dialog"] label')].find(
        (l) => l.textContent?.trim() === field,
      ) as HTMLLabelElement;
      document.getElementById(label.htmlFor)!.focus();
    }, FILTER);
    await filterBy("zzzz");

    await pressEscape();
    await expect($(".hd-about__count")).toHaveText(`${total} components`);
    expect(await $('[role="dialog"]').isExisting()).toBe(true);

    await pressEscape();
    await $('[role="dialog"]').waitForExist({ reverse: true });
    expect(await focusedButton()).toBe("About HoploDex");
  });

  it("opens from the database menu without reading or writing the database or machine.json, and Ctrl/⌘+L locks with it open (010 US1-2, SC-005)", async () => {
    await createDatabase({ name: DATABASE });
    const database = path.join(scratchDocuments(), "HoploDex", `${DATABASE}.hoplodex`);
    const machine = path.join(process.env.HOPLODEX_E2E_CONFIG_HOME!, identifier, "machine.json");
    // Opening and locking a database write both files themselves, so the
    // hashes are taken just around About.
    const before = { database: sha256Of(database), machine: sha256Of(machine) };

    await chooseMenuItem("button.hd-db-menu", "About HoploDex");
    await waitForAbout();
    expect(await focusedButton()).toContain("GNU General Public License, version 3");
    expect(await $(".hd-about__count").getText()).toBe(`${total} components`);
    await pressEscape();
    await $('[role="dialog"]').waitForExist({ reverse: true });

    expect(sha256Of(database)).toBe(before.database);
    expect(sha256Of(machine)).toBe(before.machine);

    // Open again: a lock ends the session, and About with it, without asking.
    await chooseMenuItem("button.hd-db-menu", "About HoploDex");
    await waitForAbout();
    await pressLockShortcut();
    await waitForChooser();
    expect(await $('[role="dialog"]').isExisting()).toBe(false);
    expect(await $('[role="alertdialog"]').isExisting()).toBe(false);
  });

  it("opens from the macOS menu bar, from the chooser and with a database open, in HoploDex's own window (010 US1-7)", async function () {
    skipWithoutMacInput(this);
    chooseMenuBarItem(appPid(), "About HoploDex");
    await waitForAbout();
    // The app's own About, not the system's panel: the dialog is the page's,
    // and the app has no other window.
    expect(await $('[role="dialog"] .hd-dialog__title').getText()).toBe("About HoploDex");
    expect((await browser.getWindowHandles()).length).toBe(1);
    await pressEscape();
    await $('[role="dialog"]').waitForExist({ reverse: true });

    // A lock leaves the database's row selected, asking for its passphrase.
    if ((await selectedChooserRow()) !== DATABASE) await selectChooserRow(DATABASE);
    await unlock(E2E_PASSPHRASE);
    chooseMenuBarItem(appPid(), "About HoploDex");
    await waitForAbout();
    expect(await $('[role="dialog"] .hd-dialog__title').getText()).toBe("About HoploDex");
    expect((await browser.getWindowHandles()).length).toBe(1);
    await pressEscape();
    await $('[role="dialog"]').waitForExist({ reverse: true });
  });
});
