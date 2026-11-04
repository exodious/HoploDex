import { appPid, relaunchApp } from "../support/app";
import {
  type DialogView,
  logMark,
  pressInDialog,
  tickAcknowledgement,
  typeInDialog,
  waitForLog,
  waitForView,
} from "../support/dialogInput";
import { HAS_MEMORY_SCAN, copiesIn, webProcesses } from "../support/memoryScan";
import {
  $,
  browser,
  clickButton,
  expect,
  fill,
  passphraseLog,
  scratchDocuments,
  settle,
  switchDatabase,
  waitForChooser,
  waitForCollection,
} from "../support/ui";

/**
 * The native passphrase dialog itself, typed into with real keys on every OS
 * (amended 2026-11-04 to 003 FR-007, 003 contracts/ui-databases.md §0): the
 * app is launched with `HOPLODEX_E2E_PASSPHRASE_DIALOG=native`, so no queue
 * answers for it, and what it shows comes back through its log (never a
 * passphrase). Every other spec answers it from the queue. The rules it checks
 * as it is typed are Rust's (`passphrase_dialog::checks`); this file shows that
 * each OS's dialog applies them, takes the keys a person presses, and keeps the
 * passphrase out of the web view.
 */

const folder = `${scratchDocuments()}/Native dialog`;
const NAME = "Native";
const QUOTED = `“${NAME}”`;
// Lowercase letters and spaces: what every OS's input helper types.
const PASSPHRASE = "quartz lantern meadow rivet";
const WRONG = "not the passphrase at all";
const NEW = "copper heron tablet violin";

const disabled = (view: DialogView) => !view.ready;

/** Clicks the button labelled `label` without waiting for the app to settle:
 * the command it sends waits for the dialog, so it is busy until answered. */
async function pressButton(label: string) {
  const button = await $(`button=${label}`);
  await button.waitForClickable();
  await button.click();
}

/** Chooses `item` from the database menu, as `chooseMenuItem` does, without
 * waiting for the command it sends to finish. */
async function pressMenuItem(item: string) {
  const trigger = await $("button.hd-db-menu");
  await trigger.waitForExist();
  await browser.execute((element: HTMLElement) => {
    element.dispatchEvent(
      new PointerEvent("pointerdown", { bubbles: true, button: 0, pointerType: "mouse" }),
    );
  }, trigger);
  const found = await $(`[role="menuitem"]*=${item}`);
  await found.waitForExist({ timeout: 5000 });
  await found.click();
}

/** Presses Open… on the selected row and waits for the dialog to ask. */
async function openDialogFromChooser(): Promise<number> {
  await waitForChooser();
  const mark = logMark();
  await $(".hd-db-row--selected button[type=submit]").click();
  const asked = await waitForLog(mark, (l) => "asked" in l, "the open dialog");
  expect(asked.asked).toMatchObject({ title: `Open ${QUOTED}`, primary: "Open" });
  return mark;
}

describe("003 US1 - Protect My Collection With My Own Passphrase", () => {
  before(async () => {
    process.env.HOPLODEX_E2E_PASSPHRASE_DIALOG = "native";
    await relaunchApp();
    await waitForChooser();
  });

  after(() => {
    delete process.env.HOPLODEX_E2E_PASSPHRASE_DIALOG;
  });

  it("creates a database with a passphrase typed twice in the native dialog, which waits for the rules and the acknowledgement (003 FR-003, FR-004)", async () => {
    await clickButton("Create a new database…");
    await $('[role="dialog"]').waitForExist();
    await fill("Name", NAME);
    await fill("Folder", folder);
    const mark = logMark();
    await pressButton("Create database…");

    const asked = await waitForLog(mark, (l) => "asked" in l, "the create dialog");
    expect(asked.asked).toMatchObject({
      title: "Choose a passphrase",
      primary: "Create database",
      fields: ["Passphrase", "Confirm passphrase"],
    });
    // Disabled from the start: nothing typed, and nothing ticked.
    await waitForView(mark, disabled, "a disabled button");

    // The strength hint as it is typed (003 FR-003): an estimate of the
    // passphrase in the field, worked out outside the web view.
    await typeInDialog(PASSPHRASE);
    await waitForView(mark, (v) => v.strength === "Very weak", "the first letter's estimate");
    await waitForView(
      mark,
      (v) => v.strength === "Strong" || v.strength === "Very strong",
      "the whole passphrase's estimate",
    );

    // A confirmation a character short doesn't match, once the typing pauses.
    let step = logMark();
    await pressInDialog("Tab");
    await typeInDialog(PASSPHRASE.slice(0, -1));
    await waitForView(
      step,
      (v) => v.confirmation === "The passphrases don't match." && !v.ready,
      "the mismatch after the pause",
    );
    // Put right, the problem goes at once; the acknowledgement is still due.
    step = logMark();
    await typeInDialog(PASSPHRASE.slice(-1));
    await waitForView(
      step,
      (v) => v.confirmation === null && v.passphrase === null && !v.ready,
      "the match, with the acknowledgement still unticked",
    );
    // The One Enter rule (#44): Enter does nothing while the button is
    // disabled. (Last, as on macOS it also selects the field's text, so a key
    // after it would replace what was typed: the platform's own behaviour.)
    step = logMark();
    await pressInDialog("Return");
    await browser.pause(700);
    expect(passphraseLog().slice(step).some((l) => "closed" in l)).toBe(false);

    step = logMark();
    await tickAcknowledgement();
    await waitForView(step, (v) => v.ready, "the button enabled by the acknowledgement");

    step = logMark();
    await pressInDialog("Return");
    await waitForLog(step, (l) => "closed" in l, "the dialog closing", 30000);
    await waitForCollection();
  });

  it("asks again under its field for a wrong passphrase, then opens with the right one (003 FR-005, FR-006)", async () => {
    await switchDatabase();
    const mark = await openDialogFromChooser();

    await typeInDialog(WRONG);
    await pressInDialog("Return");
    const retry = await waitForLog(mark, (l) => "retry" in l, "the dialog asking again", 30000);
    expect(retry.retry).toMatchObject({ field: "passphrase" });
    expect((retry.retry as { message: string }).message).toMatch(
      new RegExp(`^That passphrase didn't open ${QUOTED}\\.`),
    );
    // Asked again in place: one dialog, its field emptied.
    expect(passphraseLog().slice(mark).filter((l) => "asked" in l)).toHaveLength(1);

    await typeInDialog(PASSPHRASE);
    await pressInDialog("Return");
    await waitForCollection();
  });

  // Amended 2026-11-04 to 003 FR-007: the reason for the native dialog. The
  // web view's processes keep copies of what is typed into the page, which
  // nothing in it can wipe (the passphrase-copies investigation); typed in the
  // dialog, the passphrase never reaches them.
  it("leaves no copy of the passphrase in the web view's processes", async function () {
    if (!HAS_MEMORY_SCAN) this.skip();
    const web = webProcesses(appPid());
    expect(web.length).toBeGreaterThan(0);
    // The control: the folder typed into the page's own field is found, so
    // the scan reads the heap where a page's strings are.
    expect(web.reduce((sum, pid) => sum + copiesIn(pid, folder), 0)).toBeGreaterThan(0);
    for (const pid of web) {
      expect({ pid, copies: copiesIn(pid, PASSPHRASE) }).toEqual({ pid, copies: 0 });
      expect({ pid, copies: copiesIn(pid, WRONG) }).toEqual({ pid, copies: 0 });
    }
  });

  it("goes back to Open… on Escape, with nothing opened and nothing said", async () => {
    await switchDatabase();
    const mark = await openDialogFromChooser();

    await pressInDialog("Escape");

    await waitForLog(mark, (l) => "closed" in l, "the dialog closing");
    await browser.waitUntil(
      async () =>
        (await $(".hd-db-row--selected button[type=submit]").getText()).trim() === "Open…",
      { timeout: 10000, timeoutMsg: "the row never came back to Open…" },
    );
    expect(await $(".hd-db-row--selected [role=alert]").isExisting()).toBe(false);
    expect(await $('nav[aria-label="Sections"]').isExisting()).toBe(false);
  });
});

describe("003 US4 - Change My Passphrase Safely", () => {
  before(async () => {
    process.env.HOPLODEX_E2E_PASSPHRASE_DIALOG = "native";
  });

  after(() => {
    delete process.env.HOPLODEX_E2E_PASSPHRASE_DIALOG;
  });

  it("changes it in the native dialog's three fields, asking again for a wrong current one (003 FR-015)", async () => {
    await openDialogFromChooser();
    await typeInDialog(PASSPHRASE);
    await pressInDialog("Return");
    await waitForCollection();

    const mark = logMark();
    await pressMenuItem("Change passphrase…");
    const asked = await waitForLog(mark, (l) => "asked" in l, "the change dialog");
    expect(asked.asked).toMatchObject({
      title: "Change passphrase",
      primary: "Change passphrase",
      fields: ["Current passphrase", "New passphrase", "Confirm new passphrase"],
    });
    // The page shows nothing while the passphrases are asked for.
    expect(await $('[role="dialog"]').isExisting()).toBe(false);

    await typeInDialog(WRONG);
    await pressInDialog("Tab");
    await typeInDialog(NEW);
    await pressInDialog("Tab");
    await typeInDialog(NEW);
    await waitForView(mark, (v) => v.ready, "the button enabled");
    await pressInDialog("Return");
    const retry = await waitForLog(mark, (l) => "retry" in l, "the dialog asking again", 30000);
    expect(retry.retry).toMatchObject({
      field: "current",
      message: "That isn't the current passphrase.",
    });

    // Only the current passphrase was emptied, and it has the focus.
    const step = logMark();
    await typeInDialog(PASSPHRASE);
    await waitForView(step, (v) => v.ready && v.current === null, "the refusal cleared");
    await pressInDialog("Return");

    await $('[role="dialog"] [role="status"]').waitForExist({
      timeout: 60000,
      timeoutMsg: "the passphrase change never finished",
    });
    await expect($('[role="dialog"] [role="status"]')).toHaveText(
      expect.stringContaining("The passphrase has been changed."),
    );
    await clickButton("Done");
    await settle();

    // The new passphrase opens it.
    await switchDatabase();
    await openDialogFromChooser();
    await typeInDialog(NEW);
    await pressInDialog("Return");
    await waitForCollection();
  });
});
