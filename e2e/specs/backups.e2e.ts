import fs from "node:fs";
import {
  $,
  $$,
  E2E_PASSPHRASE,
  addFirearm,
  browser,
  choose,
  chooseMenuItem,
  clickButton,
  clickEl,
  createDatabase,
  invokeCommand,
  expect,
  fill,
  goTo,
  listedNames,
  scratchDocuments,
  unlock,
  waitForChooser,
  waitForCollection,
  settle,
} from "../support/ui";
import { realClick, skipWithoutRealInput } from "../support/realInput";

/**
 * End-to-end coverage of specs/003-database-protection-management's User
 * Story 3 against the real built app: a backup when a changed database is
 * closed, restoring from one, and moving them when the backup location
 * changes. The rules (once a day, the chooser's facts, leaving them, and
 * the return from a cancelled restore) are tested in Rust and Vitest.
 */

const folder = `${scratchDocuments()}/Backed up`;
const backups = `${folder}/HoploDex backups`;

/** Where the backups are moved to, and left, in the location flows. */
const elsewhere = `${scratchDocuments()}/Elsewhere`;

/** The backups of "Kept" in its default folder, or in `folder`. */
function backupFiles(folder = backups): string[] {
  if (!fs.existsSync(folder)) return [];
  return fs
    .readdirSync(folder)
    .filter((name) => /^Kept \d{4}-\d{2}-\d{2} \d{6} [0-9a-f]{8}\.hoplodex$/.test(name))
    .sort();
}

/** Locking is the database menu's close: it makes the backup a close makes. */
async function closeDatabase() {
  await chooseMenuItem("button.hd-db-menu", "Lock now");
  await waitForChooser();
}

/** The backups the restore dialog offers: those at the current location. */
async function restorable(): Promise<number> {
  await chooseMenuItem("button.hd-db-menu", "Restore from a backup…");
  await $('[role="dialog"] .hd-dialog__body').waitForExist();
  await settle();
  const count = (await $$('[role="dialog"] input[type="radio"]')).length;
  await clickButton("Cancel");
  await $('[role="dialog"]').waitForExist({ reverse: true });
  return count;
}

/** Makes `elsewhere` the backup location, doing `existingBackups` with the
 * backups at the old one. The folder is chosen in the OS's own dialog, which
 * WebDriver can't reach (Tauri's IPC can't be stubbed in the page either),
 * so the command it leads to is sent directly; a lock and an unlock then
 * show the settings as saved. */
async function useElsewhere(existingBackups?: "move" | "leave") {
  await invokeCommand("update_backup_settings", {
    enabled: true,
    keepCount: 5,
    location: { kind: "custom", path: elsewhere },
    ...(existingBackups ? { existingBackups } : {}),
  });
  await closeDatabase();
  await unlock(E2E_PASSPHRASE);
}

/** Goes back to the default backup location in the database settings,
 * answering "Backups at the old location" with `choice`. */
async function backToTheDefault(choice: string) {
  await chooseMenuItem("button.hd-db-menu", "Database settings…");
  await $('[role="dialog"]').waitForExist();
  await clickButton("Use the default");
  await clickButton("Save");
  await $("h2=Backups at the old location").waitForExist();
  await choose(choice);
  await clickButton("Change location");
  await $('[role="dialog"]').waitForExist({ reverse: true, timeout: 20000 });
}

/** The error shown for the field labelled `label` in the open dialog: the
 * text of the element its `aria-describedby` names when it is `aria-invalid`,
 * or `null` when the field shows none. */
async function passphraseFieldError(label: string): Promise<string | null> {
  return browser.execute((label: string) => {
    const labelEl = [...document.querySelectorAll('[role="dialog"] label')].find(
      (l) => l.textContent?.trim() === label,
    ) as HTMLLabelElement | undefined;
    const field = labelEl && document.getElementById(labelEl.htmlFor);
    if (!field || field.getAttribute("aria-invalid") !== "true") return null;
    const ids = (field.getAttribute("aria-describedby") ?? "").split(" ");
    for (const id of ids) {
      const el = document.getElementById(id);
      if (el?.classList.contains("hd-field__error")) return el.textContent?.trim() ?? "";
    }
    return null;
  }, label);
}

async function listed(make: string): Promise<boolean> {
  await goTo("Collection");
  return (await listedNames()).some((name) => name.includes(make));
}

describe("003 US3 - Automatic Backups and Restoring From One (was us8-backups)", () => {
  it("backs up a changed database when it is closed and restores that backup with its passphrase (FR-025, FR-028)", async () => {
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

    await unlock(E2E_PASSPHRASE);
    await addFirearm({
      make: "AfterBackup",
      model: "Two",
      caliber: ".45 ACP",
      type: "Handgun",
      serial: "BK-2",
    });
    expect(await listed("AfterBackup")).toBe(true);

    await chooseMenuItem("button.hd-db-menu", "Restore from a backup…");
    await $$('[role="dialog"] input[type="radio"]')[0].waitForExist();
    await settle();
    // A wrong passphrase: the backend's field error lands on the field, and
    // nothing is restored (003 US3 Scenario 5; the dialog's UI side is in
    // RestoreBackupDialog.test.tsx).
    await fill("Passphrase for this backup", "not the backup's passphrase");
    await clickEl('[role="dialog"] button[type="submit"]');
    await $('[role="alertdialog"]').waitForExist();
    await clickEl('[role="alertdialog"] .hd-button--danger');
    await $('[role="alertdialog"]').waitForExist({ reverse: true });
    const message = await passphraseFieldError("Passphrase for this backup");
    expect(message).toMatch(/^That passphrase didn't open the backup from /);
    expect(backupFiles()).toHaveLength(1);

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

  it("opens the restore dialog from a real click with no focus ring, sized to its content", async function () {
    // Both regressions were WebKitGTK's, and need a real click, as a person
    // makes, to show: WebDriver's own clicks don't bring them out.
    skipWithoutRealInput(this);
    await realClick("button.hd-db-menu");
    await realClick('[role="menuitem"]*=Restore from a backup');
    await $$('[role="dialog"] input[type="radio"]')[0].waitForExist();
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
    await settle();
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
    await clickButton("Cancel");
    await $('[role="dialog"]').waitForExist({ reverse: true });
  });

  it("moves the backups to the new location (FR-026, US3-4a)", async () => {
    // Open, with both backups in the default folder: the journey above.
    expect(backupFiles()).toHaveLength(2);
    // Setting the new location with its choice sent directly, as the folder
    // picker is the OS's own; the way back is through the settings dialog.
    fs.mkdirSync(elsewhere, { recursive: true });
    await useElsewhere("move");
    const made = backupFiles(elsewhere);
    expect(made.length).toBeGreaterThanOrEqual(2);
    expect(backupFiles()).toEqual([]);
    expect(await restorable()).toBe(made.length);

    await backToTheDefault("Move them to the new location");

    expect(backupFiles()).toEqual(made);
    expect(backupFiles(elsewhere)).toEqual([]);
    expect(await restorable()).toBe(made.length);
  });
});
