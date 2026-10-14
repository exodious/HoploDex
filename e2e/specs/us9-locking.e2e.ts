import fs from "node:fs";
import path from "node:path";
import { killApp, relaunchApp } from "../support/app";
import { sandboxPath, seedCollection } from "../support/sandbox";
import {
  $,
  E2E_PASSPHRASE,
  addFirearm,
  browser,
  chooseMenuItem,
  clickButton,
  confirmTakeOver,
  createDatabase,
  expect,
  fieldValue,
  fill,
  scratchDocuments,
  selectChooserRow,
  selectOption,
  selectedChooserRow,
  submitPassphrase,
  waitForChooser,
  settle,
} from "../support/ui";

/**
 * End-to-end coverage of specs/003-database-protection-management's User
 * Story 6 against the real built app: "lock now" with Ctrl+L while a form
 * has unsaved input and the pending changes resumed at the next open, and the
 * idle lock.
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

/** Opens the record's edit form and types into its notes, leaving them
 * unsaved. */
async function editNotes(notes: string) {
  await clickButton("Edit");
  await $('[role="dialog"]').waitForExist();
  await fill("Notes", notes);
  // Stage the draft now instead of sleeping past its 250 ms debounce, the way
  // the window losing focus does (usePendingDraft.ts); `settle()` then waits
  // for the backend call. (The debounce itself is not counted as busy: that
  // would make every `fill` in every form wait 250 ms.)
  await browser.execute(() => window.dispatchEvent(new Event("blur")));
  await settle();
}

/** Waits for the open dialog titled `title`, and returns its text. */
async function dialogTitled(title: string): Promise<string> {
  let text = "";
  await browser.waitUntil(
    async () => {
      text = await browser.execute((wanted: string) => {
        const dialog = [...document.querySelectorAll('[role="dialog"]')].find(
          (d) => d.querySelector(".hd-dialog__title")?.textContent?.trim() === wanted,
        );
        return dialog?.textContent ?? "";
      }, title);
      return text !== "";
    },
    { timeout: 10000, timeoutMsg: `no dialog "${title}"` },
  );
  return text;
}

function pendingQuestion(): Promise<string> {
  return dialogTitled(`Unsaved changes to ${LABEL}`);
}

/** Whether the chooser shows a notice saying `text`. */
async function chooserSays(text: string): Promise<boolean> {
  return browser.execute(
    (wanted: string) =>
      [...document.querySelectorAll(".hd-chooser__notice")].some((n) =>
        (n.textContent ?? "").includes(wanted),
      ),
    text,
  );
}

describe("User Story 6 (003) - Lock the Application When I Step Away", () => {
  // One journey for the story: lock with Ctrl+L while a form has unsaved input,
  // then resume it at the next open (003 US6 Scenarios 1, 2, 6 and 7). What the
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
});

// specs/008-hardening-batch User Story 4: changes a form this build doesn't
// know can't be resumed, but the database can be closed with them kept
// (contracts/ui.md §1). Each test seeds the human-testing collection afresh,
// whose "Shared collection" holds pending changes for the Beretta 92FS edit
// and is marked open on another computer.
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

describe("User Story 4 (008) - Pending changes from another version", () => {
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
