import fs from "node:fs";
import path from "node:path";
import { appPid, killApp, relaunchApp, waitForAppToQuit } from "../support/app";
import { sendLogOutQuitEvent, skipWithoutMacInput } from "../support/macInput";
import { realKey, skipWithoutRealInput } from "../support/realInput";
import { sandboxPath, seedCollection } from "../support/sandbox";
import {
  $,
  $$,
  E2E_PASSPHRASE,
  addFirearm,
  browser,
  chooseMenuItem,
  clickButton,
  clickEl,
  chooserSays,
  confirmTakeOver,
  createDatabase,
  dialogTitled,
  editNotes,
  expect,
  fieldValue,
  fill,
  openFirearm,
  scratchDocuments,
  selectChooserRow,
  selectOption,
  selectedChooserRow,
  submitPassphrase,
  waitForChooser,
} from "../support/ui";

/**
 * End-to-end coverage of 003 US6 against the real built app: "lock now" with
 * Ctrl+L while a form has unsaved input and the pending changes resumed at the
 * next open, and the idle lock.
 */

const folder = `${scratchDocuments()}/Locking`;
const firearm = {
  make: "Lockable",
  model: "One",
  caliber: "9mm",
  type: "Handgun" as const,
  serial: "LK-1",
};
const LABEL = "Lockable One (edit)";

/** Presses Ctrl+L on whatever has focus, as the user would. */
async function pressLockShortcut() {
  await browser.execute(() => {
    const target = document.activeElement ?? document.body;
    target.dispatchEvent(
      new KeyboardEvent("keydown", { key: "l", ctrlKey: true, bubbles: true, cancelable: true }),
    );
  });
}

function pendingQuestion(): Promise<string> {
  return dialogTitled(`Unsaved changes to ${LABEL}`);
}

describe("003 US6 - Lock the Application When I Step Away (was us9-locking)", () => {
  // One journey for the story: lock with Ctrl+L while a form has unsaved input,
  // then resume it at the next open (003 US6-1, US6-2, US6-6 and US6-7). What the
  // chooser says after a lock (the notice, the selected row), discarding the
  // changes and closing from the dialog are covered below the E2E level.
  it("locks with Ctrl+L while a firearm is being edited, leaving nothing from the collection, and resumes the exact input at the next open", async () => {
    await createDatabase({ folder, name: "Locked" });
    await addFirearm(firearm);
    await editNotes("typed before the lock");

    await pressLockShortcut();
    await waitForChooser();

    expect(await $('nav[aria-label="Sections"]').isExisting()).toBe(false);
    expect(await $('[role="dialog"]').isExisting()).toBe(false);
    const text = await browser.execute(() => document.body.textContent ?? "");
    expect(text).not.toContain("typed before the lock");
    expect(text).not.toContain("LK-1");

    await submitPassphrase(E2E_PASSPHRASE);

    expect(await pendingQuestion()).toContain("Your changes were kept.");
    expect(await $('nav[aria-label="Sections"]').isExisting()).toBe(false);
    await clickButton("Resume editing");

    await dialogTitled(`Edit ${firearm.make} ${firearm.model}`);
    expect(await fieldValue("Notes")).toBe("typed before the lock");

    // Leave the form so the next test finds the collection.
    await clickButton("Cancel");
    await browser.waitUntil(async () => !(await $('[role="dialog"]').isExisting()), {
      timeout: 5000,
      timeoutMsg: "the edit form never closed",
    });
  });

  it("locks after the idle duration set in its settings", async () => {
    await chooseMenuItem("button.hd-db-menu", "Database settings…");
    await $('[role="dialog"]').waitForExist();
    await selectOption("After", "1 minute");
    await clickButton("Save");
    await browser.waitUntil(async () => !(await $('[role="dialog"]').isExisting()), {
      timeout: 5000,
      timeoutMsg: "the settings never closed",
    });

    await browser.waitUntil(async () => $(".hd-chooser__title").isExisting(), {
      timeout: 20000,
      interval: 500,
      timeoutMsg: "the idle lock never locked",
    });

    expect(await chooserSays("HoploDex locked “Locked” after 1 minute without use.")).toBe(true);
  });

  // 003 FR-039, #107: on macOS a log out, restart or shut down asks every
  // application to quit with the quit Apple Event and a reason. Cancelling it
  // (as a user's Quit is, to ask about unsaved changes) cancels the log out.
  it("quits at once for a log out's quit event, keeping unsaved input as pending changes (macOS)", async function () {
    skipWithoutMacInput(this);
    // The idle lock above left the chooser.
    await submitPassphrase(E2E_PASSPHRASE);
    await $('nav[aria-label="Sections"]').waitForExist();
    // The open lands on the collection list, not on the record.
    await openFirearm(`${firearm.make} ${firearm.model}`);
    await editNotes("typed before the log out");

    // The quit loginwindow sends, to this process only.
    sendLogOutQuitEvent(appPid());
    // The process ends without the unsaved-changes question being answered.
    await waitForAppToQuit(15000);

    await relaunchApp();
    await browser.waitUntil(
      async () => (await browser.execute(() => document.readyState)) === "complete",
      { timeout: 10000, timeoutMsg: "the relaunched app never finished loading" },
    );
    await waitForChooser();
    await submitPassphrase(E2E_PASSPHRASE);
    expect(await pendingQuestion()).toContain("Your changes were kept.");
    await clickButton("Resume editing");
    await dialogTitled(`Edit ${firearm.make} ${firearm.model}`);
    expect(await fieldValue("Notes")).toBe("typed before the log out");
    await clickButton("Cancel");
  });
});

// 008 US4: changes a form this build doesn't know can't be resumed, but the
// database can be closed with them kept (008 contracts/ui.md §1). Each test
// seeds the human-testing collection afresh, whose "Shared collection" holds
// pending changes for the Beretta 92FS edit and is marked open on another
// computer.
const SHARED = "Shared collection";
const BERETTA_LABEL = "Beretta 92FS — edit";

/** Replaces the sandbox's databases with a new seed, the pending changes of
 * "Shared collection" kept by `pendingFrom` when given (else by this build),
 * and starts the app on them. The app is stopped and the earlier seed's folder
 * removed first, since the seed won't write into one that exists. */
async function reseed(pendingFrom?: string) {
  await killApp();
  fs.rmSync(path.join(sandboxPath()!, "seed"), { recursive: true, force: true });
  seedCollection({ pendingFrom });
  await relaunchApp();
}

/** Opens "Shared collection", taking it over from the other computer when
 * asked, and waits for the pending changes question. */
async function openSharedToPending(): Promise<string> {
  await waitForChooser();
  if ((await selectedChooserRow()) !== SHARED) await selectChooserRow(SHARED);
  await submitPassphrase(process.env.HOPLODEX_E2E_SEED_PASSPHRASE!);
  const takeOver = await $("button=Take over…");
  if (await takeOver.waitForExist({ timeout: 2000 }).catch(() => false)) {
    await confirmTakeOver(SHARED, process.env.HOPLODEX_E2E_SEED_PASSPHRASE!);
  }
  return dialogTitled(`Unsaved changes to ${BERETTA_LABEL}`);
}

/** The labels of the buttons in the open dialog's footer, in order. */
async function footerButtons(): Promise<string[]> {
  return browser.execute(() =>
    [...document.querySelectorAll('[role="dialog"] .hd-dialog__footer button')].map(
      (button) => button.textContent?.trim() ?? "",
    ),
  );
}

describe("008 US4 - Pending Changes Kept by Another Version (was us9-locking)", () => {
  it("pending changes from another version: close, then reopen", async () => {
    await reseed("1.2.0");

    const text = await openSharedToPending();
    expect(text).toContain("They were kept by HoploDex 1.2.0.");
    expect(await footerButtons()).toEqual(["Discard changes", "Close the database"]);

    await clickButton("Close the database");
    await waitForChooser();
    expect(await selectedChooserRow()).toBe(SHARED);
    expect(await $('nav[aria-label="Sections"]').isExisting()).toBe(false);

    // The changes were kept: the next open asks about them again.
    await submitPassphrase(process.env.HOPLODEX_E2E_SEED_PASSPHRASE!);
    const again = await dialogTitled(`Unsaved changes to ${BERETTA_LABEL}`);
    expect(again).toContain("They were kept by HoploDex 1.2.0.");
    expect(await footerButtons()).toEqual(["Discard changes", "Close the database"]);
  });
});

// Issue #44: the app's Enter rule, with real key presses in the real app.
// Enter in a confirmation's field confirms it, and Enter on a radio only
// operates the radio: WebKitGTK would otherwise submit the radio's form.
describe("Issue #44 - Enter in a confirmation's field and on a radio", () => {
  it("takes over with Enter in the passphrase field, then Enter on a radio submits nothing", async function () {
    skipWithoutRealInput(this);
    await reseed();
    await waitForChooser();
    if ((await selectedChooserRow()) !== SHARED) await selectChooserRow(SHARED);
    const passphrase = process.env.HOPLODEX_E2E_SEED_PASSPHRASE!;
    await submitPassphrase(passphrase);
    await clickEl("button=Take over…");
    await $('[role="alertdialog"]').waitForExist();
    const field = await $(`[role="alertdialog"] input[type="password"]`);
    await browser.waitUntil(async () => field.isFocused(), { timeoutMsg: "no focus in the field" });
    await fill(`Passphrase for “${SHARED}”`, passphrase);

    await realKey("Return");

    await $('[role="alertdialog"]').waitForExist({ reverse: true });
    expect(await dialogTitled(`Unsaved changes to ${BERETTA_LABEL}`)).toContain(
      "Your changes were kept.",
    );

    // The pending edit resumes in a form with radios (the firearm's type).
    await clickButton("Resume editing");
    await browser.waitUntil(
      async () => (await $$('[role="dialog"] input[type="radio"]')).length > 0,
      {
        timeout: 10000,
        timeoutMsg: "the edit form never showed its radios",
      },
    );
    await browser.execute(() => {
      const seen = window as unknown as { __submits?: number };
      seen.__submits = 0;
      document.addEventListener("submit", () => (seen.__submits = (seen.__submits ?? 0) + 1), true);
      document.querySelector<HTMLInputElement>('[role="dialog"] input[type="radio"]')!.focus();
    });

    await realKey("Return");

    expect(
      await browser.execute(() => (window as unknown as { __submits?: number }).__submits),
    ).toBe(0);
    expect(await $('[role="dialog"]').isExisting()).toBe(true);
  });
});
