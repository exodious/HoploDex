import path from "node:path";
import {
  $,
  back,
  browser,
  choose,
  groupBy,
  clickButton,
  clickEl,
  fieldValue,
  fill,
  goTo,
  search,
} from "../support/ui";
import {
  chooseMenuItem,
  requestQuit,
  selectChooserRow,
  submitPassphrase,
  settleChooserPlate,
  unlock,
  waitForChooser,
} from "../support/ui";
import { chooseTheme, shot } from "../support/screenshots";
import { realClick, realKey } from "../support/realInput";

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

/** Sends `event` to the page as if the backend had, for a state the
 * seeded collection doesn't reach on its own. */
async function emitFromBackend(event: string, payload: unknown) {
  await browser.execute(
    (name: string, data: unknown) => {
      const internals = (
        window as unknown as {
          __TAURI_INTERNALS__: { invoke: (cmd: string, args: unknown) => Promise<unknown> };
        }
      ).__TAURI_INTERNALS__;
      void internals.invoke("plugin:event|emit", { event: name, payload: data });
    },
    event,
    payload,
  );
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

/** Types `text` with real key presses into the focused field (an X keysym
 * per character), since the suggestion lists answer to real input. */
async function typeReal(text: string) {
  const names: Record<string, string> = { " ": "space", ".": "period" };
  for (const char of text) {
    const upper = char !== char.toLowerCase();
    await realKey(upper ? `Shift_L+${char.toLowerCase()}` : (names[char] ?? char));
  }
}

/** Tab lands in Caliber and brings its list up, which the shots don't want.
 * Escape closes the list only if it is up: with none, it would close the form. */
async function closeListIfOpen() {
  await browser.pause(600);
  if (await $('[role="listbox"]').isDisplayed()) {
    await realKey("Escape");
    await $('[role="listbox"]').waitForExist({ reverse: true });
  }
}

/** Closes the open form without saving, whether or not it asks first. */
async function discardForm() {
  await browser.execute(() =>
    document.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }),
    ),
  );
  await browser.pause(400);
  if (await $('[role="alertdialog"]').isExisting()) await clickButton("Discard changes");
  await $('[role="dialog"]').waitForExist({ reverse: true });
  await browser.pause(200);
}

/** Scrolls a form field to the middle of the window, so its list or note is in view. */
async function centerField(field: string) {
  await browser.execute((name: string) => {
    document
      .querySelector(`[role="dialog"] [data-field="${name}"]`)
      ?.scrollIntoView({ block: "center" });
  }, field);
  await browser.pause(300);
}

// The app starts at the chooser, listing the seeded databases. It has to be
// shot in both themes before a database is opened.
describe("Screenshots: the chooser", () => {
  for (const theme of ["Light", "Dark"] as const) {
    const suffix = theme.toLowerCase();

    it(`chooser and create database (${suffix})`, async () => {
      await waitForChooser();
      await chooseTheme(theme);
      await settleChooserPlate();
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
      await settleChooserPlate();
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

    // specs/004-cartridges-action-types contracts/ui-entry.md §9.
    it("suggestions, guesses and notes in the firearm form", async () => {
      await groupBy("Cartridge");
      await $("h2.hd-group__title").waitForExist();
      await browser.pause(300);
      await shot(`31-grouped-by-cartridge-${suffix}`);
      await groupBy("Type");

      await openDialog("Add firearm");
      await realClick('[data-field="make"] input');
      await typeReal("Gl");
      await $('[role="listbox"]').waitForDisplayed({ timeout: 5000 });
      await centerField("make");
      await shot(`28-make-suggestions-${suffix}`);
      await realKey("Escape");
      await $('[role="listbox"]').waitForExist({ reverse: true });

      await realClick('[data-field="cartridge"] input');
      await typeReal("9mm");
      await $('[role="listbox"]').waitForDisplayed({ timeout: 5000 });
      await centerField("cartridge");
      await shot(`29-cartridge-suggestions-${suffix}`);
      await realKey("Escape");
      await $('[role="listbox"]').waitForExist({ reverse: true });

      // A custom cartridge whose bore can be read from its name: the caliber
      // is filled in, marked as a guess.
      await browser.execute(() => {
        const input = document.querySelector<HTMLInputElement>('[data-field="cartridge"] input');
        input?.select();
      });
      await typeReal(".30 Custom Improved");
      await browser.waitUntil(
        async () => (await fieldValue("Cartridge")) === ".30 Custom Improved",
        {
          timeoutMsg: "the cartridge text was not typed as sent",
        },
      );
      await realKey("Escape");
      await realKey("Tab");
      await $(".hd-guess-tag").waitForExist({ timeout: 5000 });
      await closeListIfOpen();
      await centerField("caliber");
      await shot(`30-caliber-guess-${suffix}`);
      await discardForm();

      // Changing the cartridge of a saved firearm suggests a caliber rather
      // than replacing the one on record.
      await openRecord("Savage 110");
      await openDialog("Edit");
      await realClick('[data-field="cartridge"] input');
      await realKey("Control_L+a");
      await typeReal("9x19mm Parabellum");
      await realKey("Escape");
      await realKey("Tab");
      await $(".hd-caliber-suggestion").waitForExist({ timeout: 5000 });
      await closeListIfOpen();
      await centerField("caliber");
      await shot(`32-caliber-suggestion-${suffix}`);
      await discardForm();
      await back();

      // A type that doesn't allow the recorded action clears it, with a note.
      await openRecord("Mossberg 500");
      await openDialog("Edit");
      await choose("Handgun");
      await browser.waitUntil(
        () =>
          browser.execute(() =>
            Boolean(
              document
                .querySelector('[role="dialog"] [data-field="actionTypeId"]')
                ?.textContent?.includes("the action was cleared"),
            ),
          ),
        { timeout: 5000, timeoutMsg: "the cleared-action note never appeared" },
      );
      await centerField("actionTypeId");
      await shot(`33-action-cleared-${suffix}`);
      await discardForm();
      await back();
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

    it("database settings, passphrase change and restore", async () => {
      await chooseMenuItem("button.hd-db-menu", "Database settings…");
      await $('[role="dialog"]').waitForExist();
      await browser.pause(300);
      await shot(`19-database-settings-${suffix}`, { fullPage: true });
      // The seeded backups are at a custom location, so going back to the
      // default asks what to do with them (FR-026). Cancelled: nothing is
      // saved.
      await clickButton("Use the default");
      await clickButton("Save");
      await $("h2=Backups at the old location").waitForExist();
      await browser.pause(300);
      await shot(`27-backup-location-change-${suffix}`);
      await clickButton("Cancel");
      await $("h2=Backups at the old location").waitForExist({ reverse: true });
      await closeDialog();

      await chooseMenuItem("button.hd-db-menu", "Change passphrase…");
      await $('[role="dialog"] input[autocomplete="current-password"]').waitForExist();
      await browser.pause(300);
      await shot(`20-change-passphrase-${suffix}`);
      await closeDialog();

      await chooseMenuItem("button.hd-db-menu", "Restore from a backup…");
      await $('[role="dialog"] input[type="radio"]').waitForExist();
      await browser.pause(300);
      await shot(`21-restore-backup-${suffix}`, { fullPage: true });
      await closeDialog();

      await chooseMenuItem("button.hd-db-menu", "About databases and security");
      await $('[role="dialog"] .hd-db-guide').waitForExist();
      await browser.pause(300);
      await shot(`23-database-guide-${suffix}`);
      // How this database is set up, beside the defaults.
      await browser.execute(() =>
        document.querySelector(".hd-db-guide__settings")?.scrollIntoView({ block: "center" }),
      );
      await browser.pause(300);
      await shot(`26-database-guide-settings-${suffix}`);
      await closeDialog();
    });

    it("pending changes after a lock", async () => {
      // A lock with an edit under way keeps it, and the next open asks.
      await goTo("Collection");
      await openRecord(RECORD);
      await openDialog("Edit");
      await fill("Notes", "Swapped the grips for the walnut set.");
      await browser.pause(400);
      await browser.execute(() =>
        (document.activeElement ?? document.body).dispatchEvent(
          new KeyboardEvent("keydown", {
            key: "l",
            ctrlKey: true,
            bubbles: true,
            cancelable: true,
          }),
        ),
      );
      await waitForChooser();
      await submitPassphrase(process.env.HOPLODEX_E2E_SEED_PASSPHRASE!);
      await $("button=Resume editing").waitForExist({ timeout: 10000 });
      await browser.pause(300);
      await shot(`22-pending-changes-${suffix}`);
      await clickButton("Discard changes");
      await $('[role="alertdialog"]').waitForExist();
      await clickButton("Discard changes");
      await $('nav[aria-label="Sections"]').waitForExist({ timeout: 10000 });
      await browser.pause(300);
    });

    it("closing with a backup", async () => {
      // The seeded collection was backed up today, so a real close makes no
      // backup: the closing screen is shown with the events a long one sends.
      await emitFromBackend("session:closing", { reason: "closed" });
      await emitFromBackend("backup:progress", {
        processed: 96_000_000,
        total: 212_000_000,
        showNow: true,
      });
      await $('[role="progressbar"]').waitForExist();
      await shot(`18-closing-backup-${suffix}`);
      // Then a real close, and back to the collection for the next walk.
      await browser.execute(() => {
        const internals = (
          window as unknown as {
            __TAURI_INTERNALS__: { invoke: (cmd: string, args: unknown) => Promise<unknown> };
          }
        ).__TAURI_INTERNALS__;
        void internals.invoke("close_database", { reason: "closed" });
      });
      await waitForChooser();
      await unlock(process.env.HOPLODEX_E2E_SEED_PASSPHRASE!);
    });
  });
}

// The import report is shot once: importing the sample again would only find
// its rows already there. Both themes, with the report left open.
describe("Screenshots: the import report", () => {
  it("shows the derived calibers and matched spellings", async () => {
    await goTo("Collection");
    await chooseTheme("Light");
    // The seed keeps its import samples beside the config directory.
    const samples = path.join(path.dirname(process.env.XDG_CONFIG_HOME!), "import-samples");
    await clickButton("Import");
    await fill("Spreadsheet file", path.join(samples, "import-cartridges.csv"));
    await clickButton("Import");
    await $(".hd-tally").waitForExist({ timeout: 15000, timeoutMsg: "import never finished" });
    for (const title of [
      "Calibers filled in from the cartridge",
      "Spellings matched to existing values",
    ]) {
      await browser.execute((wanted: string) => {
        const trigger = [...document.querySelectorAll<HTMLElement>(".hd-disclosure__trigger")].find(
          (t) => t.textContent?.includes(wanted),
        );
        if (trigger?.getAttribute("aria-expanded") === "false") trigger.click();
      }, title);
    }
    await browser.pause(300);
    for (const theme of ["Light", "Dark"] as const) {
      await chooseTheme(theme);
      await shot(`34-import-report-${theme.toLowerCase()}`, { fullPage: true });
    }
    await closeDialog();
    await chooseTheme("Light");
  });
});
