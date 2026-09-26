import fs from "node:fs";
import {
  $,
  E2E_PASSPHRASE,
  addFirearm,
  browser,
  chooseMenuItem,
  clickEl,
  createDatabase,
  expect,
  fill,
  goTo,
  listedNames,
  scratchDocuments,
  unlock,
  waitForChooser,
  waitForCollection,
} from "../support/ui";

/**
 * End-to-end coverage of specs/003-database-protection-management's User
 * Story 3 against the real built app: a backup when a changed database is
 * closed, at most once a day, and restoring from one.
 */

const folder = `${scratchDocuments()}/Backed up`;
const backups = `${folder}/HoploDex backups`;

/** The backups of "Kept" in its default folder. */
function backupFiles(): string[] {
  if (!fs.existsSync(backups)) return [];
  return fs
    .readdirSync(backups)
    .filter((name) => /^Kept \d{4}-\d{2}-\d{2} \d{6} [0-9a-f]{8}\.hoplodex$/.test(name))
    .sort();
}

async function closeDatabase() {
  await chooseMenuItem("button.hd-db-menu", "Close database");
  await waitForChooser();
}

async function listed(make: string): Promise<boolean> {
  await goTo("Collection");
  return (await listedNames()).some((name) => name.includes(make));
}

describe("User Story 3 (003) - Automatic Backups and Restoring From One", () => {
  it("backs up a changed database when it is closed (FR-025)", async () => {
    await createDatabase({ folder, name: "Kept" });
    await addFirearm({
      make: "BackedUp",
      model: "One",
      caliber: "9mm",
      type: "Handgun",
      serial: "BK-1",
    });

    await closeDatabase();

    expect(backupFiles()).toHaveLength(1);
  });

  it("makes no second backup the same day", async () => {
    await unlock(E2E_PASSPHRASE);
    await addFirearm({
      make: "AfterBackup",
      model: "Two",
      caliber: ".45 ACP",
      type: "Handgun",
      serial: "BK-2",
    });

    await closeDatabase();

    expect(backupFiles()).toHaveLength(1);
  });

  it("restores that backup with its passphrase, backing up the current database first (FR-028)", async () => {
    await unlock(E2E_PASSPHRASE);
    expect(await listed("AfterBackup")).toBe(true);

    await chooseMenuItem("button.hd-db-menu", "Restore from a backup…");
    await $('[role="dialog"] input[type="radio"]').waitForExist();
    await fill("Passphrase for this backup", E2E_PASSPHRASE);
    await clickEl('[role="dialog"] button[type="submit"]');
    await $('[role="alertdialog"]').waitForExist();
    await clickEl('[role="alertdialog"] .hd-button--danger');
    await $('[role="dialog"]').waitForExist({ reverse: true, timeout: 20000 });
    await waitForCollection();

    await expect($('section[aria-label="Restored from a backup"]')).toExist();
    expect(await listed("BackedUp")).toBe(true);
    expect(await listed("AfterBackup")).toBe(false);
    // The "before restoring" backup holds the change the restore undid.
    await browser.waitUntil(async () => backupFiles().length === 2, {
      timeout: 5000,
      timeoutMsg: "no before-restoring backup appeared",
    });
  });
});
