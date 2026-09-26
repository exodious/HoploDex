import { $, back, browser, choose, clickButton, clickEl, fill, goTo, search } from "../support/ui";
import {
  requestQuit,
  selectChooserRow,
  submitPassphrase,
  unlock,
  waitForChooser,
} from "../support/ui";
import { chooseTheme, shot } from "../support/screenshots";

/**
 * The standard screenshot set for pull requests that change the UI: the main
 * screens and dialogs, in light and dark mode, against the human-testing
 * collection (wdio.conf.ts seeds it for specs in this directory). Not part of
 * `npm run test:e2e`; run it with `npm run screenshots`, which writes
 * `<nn>-<screen>-<theme>.png` to e2e/screenshots-out/.
 *
 * Names are stable, so running it on the base branch and on the PR branch
 * gives before/after pairs. Add a screen here when a change adds one; the
 * screens that need a sandbox with no databases are in first-run.e2e.ts.
 */

const RECORD = "Glock 19 Gen5"; // the seeded record with photos, documents and every detail

async function openRecord(name: string) {
  await browser.waitUntil(
    () =>
      browser.execute((wanted: string) => {
        const button = [...document.querySelectorAll<HTMLElement>(".hd-row__name")].find((b) =>
          b.textContent?.trim().startsWith(wanted),
        );
        button?.click();
        return Boolean(button);
      }, name),
    { timeout: 5000, timeoutMsg: `no firearm "${name}" in the list` },
  );
  await $("#record-name").waitForExist();
  await browser.pause(300);
}

/** Scrolls to the bottom of a long page, so the pinned strip (the
 * continuation of the page's heading) shows, and shoots the viewport. */
async function shotScrolled(name: string) {
  await browser.execute(() => window.scrollTo(0, document.documentElement.scrollHeight));
  await $(".hd-runhead").waitForExist();
  await browser.pause(300);
  await shot(name);
  await browser.execute(() => window.scrollTo(0, 0));
  await $(".hd-runhead").waitForExist({ reverse: true });
}

async function openDialog(button: string) {
  await clickButton(button);
  await $('[role="dialog"]').waitForExist();
  await browser.pause(300);
}

/** Dismisses the open dialog the way Escape would, without saving. It must
 * be cancelable like a real key press: the dialog cancels it, which stops the
 * record page's own Escape handler from also going back to the list. */
async function closeDialog() {
  await browser.execute(() =>
    document.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }),
    ),
  );
  await $('[role="dialog"]').waitForExist({ reverse: true });
  await browser.pause(200);
}

// The app starts at the chooser, listing the seeded databases. It has to be
// shot in both themes before a database is opened.
describe("Screenshots: the chooser", () => {
  for (const theme of ["Light", "Dark"] as const) {
    const suffix = theme.toLowerCase();

    it(`chooser and create database (${suffix})`, async () => {
      await waitForChooser();
      await chooseTheme(theme);
      await shot(`14-chooser-${suffix}`);

      await openDialog("Create a new database…");
      await fill("Passphrase", "vivid otter ledger crane");
      await $('[role="meter"]').waitForExist();
      await shot(`16-create-database-${suffix}`, { fullPage: true });
      await closeDialog();
    });

    it(`open on another computer (${suffix})`, async () => {
      // The seeded "Shared collection" is marked open on "Workshop PC".
      await selectChooserRow("Shared collection");
      await submitPassphrase(process.env.HOPLODEX_E2E_SEED_PASSPHRASE!);
      await $("button=Take over…").waitForExist();
      await shot(`17-open-elsewhere-${suffix}`);
      await clickButton("Go back");
      await selectChooserRow("Main collection");
    });
  }

  after(async () => {
    await unlock(process.env.HOPLODEX_E2E_SEED_PASSPHRASE!);
  });
});

for (const theme of ["Light", "Dark"] as const) {
  const suffix = theme.toLowerCase();

  describe(`Screenshots (${suffix})`, () => {
    before(async () => {
      await goTo("Collection");
      await chooseTheme(theme);
    });

    it("collection", async () => {
      await $(".hd-row__name").waitForExist();
      await shot(`01-collection-list-${suffix}`);

      await choose("Tiles");
      await $(".hd-tile__name").waitForExist();
      await shot(`02-collection-tiles-${suffix}`);
      await choose("List");

      await search("12 gauge");
      await shot(`03-collection-search-${suffix}`);
      await search("");
    });

    it("firearm record and its dialogs", async () => {
      await openRecord(RECORD);
      await shot(`04-record-${suffix}`, { fullPage: true });
      await shotScrolled(`04-record-scrolled-${suffix}`);

      await openDialog("Edit");
      await shot(`05-edit-firearm-${suffix}`, { fullPage: true });
      await closeDialog();

      await openDialog((await $("button=Change").isExisting()) ? "Change" : "Assign");
      await shot(`06-coverage-${suffix}`);
      await closeDialog();

      await openDialog("Mark disposed");
      await shot(`07-mark-disposed-${suffix}`);
      await closeDialog();

      // Quitting with an edit under way asks save, discard or cancel.
      await openDialog("Edit");
      await fill("Notes", "Swapped the grips for the walnut set.");
      await requestQuit();
      await $('[role="alertdialog"]').waitForExist();
      await browser.pause(300);
      await shot(`25-unsaved-changes-${suffix}`);
      await clickButton("Cancel");
      await $('[role="alertdialog"]').waitForExist({ reverse: true });
      await closeDialog();

      await back();
    });

    it("add firearm", async () => {
      await openDialog("Add firearm");
      await shot(`08-add-firearm-${suffix}`, { fullPage: true });
      await closeDialog();
    });

    it("insurance", async () => {
      await goTo("Insurance");
      await $(".hd-policy").waitForExist();
      await shot(`09-insurance-${suffix}`, { fullPage: true });

      await clickEl(".hd-policy__open");
      await $(".hd-backlink").waitForExist();
      await shot(`10-policy-${suffix}`, { fullPage: true });
      await back();

      await openDialog("Add policy");
      await shot(`11-add-policy-${suffix}`, { fullPage: true });
      await closeDialog();
      await goTo("Collection");
    });

    it("import and export", async () => {
      await openDialog("Import");
      await shot(`12-import-${suffix}`);
      await closeDialog();

      await openDialog("Export");
      await shot(`13-export-${suffix}`);
      await closeDialog();
    });
  });
}
