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
import { realClick } from "../support/realInput";

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

/** Locking is the database menu's close: it makes the backup a close makes. */
async function closeDatabase() {
  await chooseMenuItem("button.hd-db-menu", "Lock now");
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

    // Opened with a real click, as a person does: WebDriver's own clicks
    // don't bring out the focus ring.
    await realClick("button.hd-db-menu");
    await realClick('[role="menuitem"]*=Restore from a backup');
    await $('[role="dialog"] input[type="radio"]').waitForExist();
    // Regression: the backup it starts on showed the keyboard focus ring,
    // after a mouse click, as though already chosen.
    expect(
      await browser.execute(() => {
        const focused = document.activeElement;
        return focused?.matches('input[type="radio"]') && !focused.matches(":focus-visible");
      }),
    ).toBe(true);
    // Regression: WebKitGTK counted the backup list's <fieldset> legend twice
    // when the dialog first sized itself, leaving it too tall until focus
    // moved. The body fits its content exactly once the entrance is over.
    await browser.pause(300);
    const slack = await browser.execute(() => {
      const body = document.querySelector<HTMLElement>('[role="dialog"] .hd-dialog__body')!;
      const style = getComputedStyle(body);
      const parts = [...body.children];
      const content =
        parts[parts.length - 1].getBoundingClientRect().bottom -
        parts[0].getBoundingClientRect().top;
      return (
        body.clientHeight - parseFloat(style.paddingTop) - parseFloat(style.paddingBottom) - content
      );
    });
    expect(Math.abs(slack)).toBeLessThan(1);
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
