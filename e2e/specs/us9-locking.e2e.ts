import {
  $,
  E2E_PASSPHRASE,
  addFirearm,
  browser,
  chooseMenuItem,
  clickButton,
  createDatabase,
  expect,
  fieldValue,
  fill,
  openFirearm,
  scratchDocuments,
  selectOption,
  selectedChooserRow,
  submitPassphrase,
  waitForChooser,
  waitForCollection,
} from "../support/ui";

/**
 * End-to-end coverage of specs/003-database-protection-management's User
 * Story 6 against the real built app: "lock now" with Ctrl+L while a form
 * has unsaved input, the pending changes offered at the next open (resumed,
 * then discarded), and the idle lock.
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
  // Past the draft's 250 ms staging.
  await browser.pause(400);
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
  it("locks with Ctrl+L while a firearm is being edited, leaving nothing from the collection", async () => {
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
  });

  it("shows the locked database selected, with the locked notice", async () => {
    expect(await selectedChooserRow()).toBe("Locked");
    expect(await chooserSays("HoploDex locked “Locked”.")).toBe(true);
    const focused = await browser.execute(
      () => document.activeElement?.closest(".hd-db-row--selected") != null,
    );
    expect(focused).toBe(true);
  });

  it("offers the unsaved changes at the next open, and resuming brings back the exact input", async () => {
    await submitPassphrase(E2E_PASSPHRASE);

    expect(await pendingQuestion()).toContain("Your changes were kept.");
    expect(await $('nav[aria-label="Sections"]').isExisting()).toBe(false);
    await clickButton("Resume editing");

    await dialogTitled(`Edit ${firearm.make} ${firearm.model}`);
    expect(await fieldValue("Notes")).toBe("typed before the lock");
  });

  it("discards them on the next open when asked to, leaving the form clean", async () => {
    await pressLockShortcut();
    await waitForChooser();
    await submitPassphrase(E2E_PASSPHRASE);
    await pendingQuestion();

    await clickButton("Discard changes");
    await $('[role="alertdialog"]').waitForExist();
    await clickButton("Discard changes");
    await waitForCollection();

    await openFirearm(`${firearm.make} ${firearm.model}`);
    await clickButton("Edit");
    await $('[role="dialog"]').waitForExist();
    expect(await fieldValue("Notes")).toBe("");
    await clickButton("Cancel");
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
      timeout: 90000,
      interval: 1000,
      timeoutMsg: "the idle lock never locked",
    });

    expect(await chooserSays("HoploDex locked “Locked” after 1 minute without use.")).toBe(true);
    // A minute of waiting, beyond the usual limit.
  }).timeout(150000);
});
