import fs from "node:fs";
import {
  $,
  E2E_PASSPHRASE,
  addFirearm,
  browser,
  clickButton,
  clickEl,
  expect,
  fill,
  goTo,
  isButtonDisabled,
  listedNames,
  scratchDocuments,
  submitPassphrase,
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

  it("creates a database only with a long enough passphrase and the acknowledgement (FR-003, FR-004)", async () => {
    await clickButton("Create a new database…");
    await $('[role="dialog"]').waitForExist();
    await fill("Name", "Test");
    await fill("Folder", folder);
    await expect($(`p=Saved as ${folder}/Test.hoplodex`)).toExist();

    // Create stays disabled until the no-recovery acknowledgement is ticked.
    expect(await isButtonDisabled("Create database")).toBe(true);
    await toggle(ACKNOWLEDGEMENT);
    expect(await isButtonDisabled("Create database")).toBe(false);

    await fill("Passphrase", "too short");
    await fill("Confirm passphrase", "too short");
    await clickButton("Create database");
    await expect($("p=Use at least 12 characters.")).toExist();
    expect(fs.existsSync(`${folder}/Test.hoplodex`)).toBe(false);

    await fill("Passphrase", E2E_PASSPHRASE);
    await fill("Confirm passphrase", E2E_PASSPHRASE);
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
        "p=That passphrase didn't open Test. Either the passphrase is wrong, or the file isn't a HoploDex database or is damaged.",
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
