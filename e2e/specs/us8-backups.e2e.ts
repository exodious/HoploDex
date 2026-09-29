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
} from "../support/ui";
import { realClick } from "../support/realInput";

/**
 * End-to-end coverage of specs/003-database-protection-management's User
 * Story 3 against the real built app: a backup when a changed database is
 * closed, at most once a day, restoring from one, and moving or leaving
 * them when the backup location changes.
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
  await browser.pause(300);
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

/** What the chooser row `row` says under `label` (FR-040): the moment, and
 * the note after it, or `null` when the row doesn't show that fact. */
async function chooserFact(
  label: string,
  row = ".hd-db-row--selected",
): Promise<{ moment: string; note: string } | null> {
  return browser.execute(
    (row: string, label: string) => {
      const term = [...document.querySelectorAll(`${row} .hd-db-row__details dt`)].find(
        (dt) => dt.textContent?.trim() === label,
      );
      const detail = term?.nextElementSibling;
      if (!detail) return null;
      return {
        moment: detail.firstChild?.textContent?.trim() ?? "",
        note: detail.querySelector(".hd-db-row__fact-note")?.textContent?.trim() ?? "",
      };
    },
    row,
    label,
  );
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
    // The chooser, back on the database just closed, says so (FR-040).
    const opened = await chooserFact("Last opened here");
    expect(opened?.moment).toMatch(/^Today at /);
    const backedUp = await chooserFact("Last backup");
    expect(backedUp?.moment).toMatch(/^Today at /);
    expect(backedUp?.note).toBe("1 kept");
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

  it("leaves the backups at the old location, which no longer lists them (FR-026, US3-4a)", async () => {
    // Still open after the restore, with both backups in the default folder.
    expect(backupFiles()).toHaveLength(2);
    fs.mkdirSync(elsewhere, { recursive: true });
    await useElsewhere("move");
    // Both moved, and any backup the lock made is at the new location too.
    const moved = backupFiles(elsewhere);
    expect(moved.length).toBeGreaterThanOrEqual(2);
    expect(backupFiles()).toEqual([]);

    await backToTheDefault("Leave them where they are");

    expect(backupFiles(elsewhere)).toEqual(moved);
    expect(backupFiles()).toEqual([]);
    expect(await restorable()).toBe(0);
  });

  it("moves the backups to the new location (FR-026, US3-4a)", async () => {
    // Their folder chosen again: they are the database's backups again.
    await useElsewhere();
    const made = backupFiles(elsewhere);
    expect(made.length).toBeGreaterThanOrEqual(2);
    expect(await restorable()).toBe(made.length);

    await backToTheDefault("Move them to the new location");

    expect(backupFiles()).toEqual(made);
    expect(backupFiles(elsewhere)).toEqual([]);
    expect(await restorable()).toBe(made.length);
  });

  it("shows what it knows of the selected database only (FR-040)", async () => {
    await closeDatabase();
    await createDatabase({ folder: elsewhere, name: "Other" });
    await closeDatabase();

    // The one just closed is selected; "Kept", with its backups, isn't.
    await expect($(".hd-db-row--selected .hd-db-row__name")).toHaveText("Other");
    expect(await chooserFact("Last opened here")).not.toBeNull();
    const kept = ".hd-db-row:not(.hd-db-row--selected)";
    await expect($(`${kept} .hd-db-row__name`)).toHaveText("Kept");
    expect(await chooserFact("Last opened here", kept)).toBeNull();
    expect(await chooserFact("Last backup", kept)).toBeNull();
  });
});
