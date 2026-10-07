import path from "node:path";
import {
  $,
  back,
  browser,
  choose,
  groupBy,
  clickButton,
  clickEl,
  fill,
  goTo,
  openPhysicalGroup,
  pressEscape,
  search,
  selectOption,
  settle,
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
import { relaunchApp } from "../support/app";
import {
  SCREENSHOT_WINDOW,
  chooseTheme,
  resizeWindow,
  shot,
  shotDisplay,
} from "../support/screenshots";

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
  await settle();
}

/** Scrolls to the bottom of a long page, so the pinned strip (the
 * continuation of the page's heading) shows, and shoots the viewport. */
async function shotScrolled(name: string) {
  await browser.execute(() => window.scrollTo(0, document.documentElement.scrollHeight));
  await $(".hd-runhead").waitForExist();
  await shot(name);
  await browser.execute(() => window.scrollTo(0, 0));
  await $(".hd-runhead").waitForExist({ reverse: true });
}

async function openDialog(button: string) {
  await clickButton(button);
  await $('[role="dialog"]').waitForExist();
  await settle();
}

/** Sends `event` to the page as if the backend had, for a state the
 * seeded collection doesn't reach on its own. Returns once the emit call has
 * resolved and the page has settled; a caller waits for what it expects to
 * appear. (This call bypasses `invoke()`, so the busy count doesn't see it.) */
async function emitFromBackend(event: string, payload: unknown) {
  await browser.executeAsync(
    (name: string, data: unknown, done: () => void) => {
      const internals = (
        window as unknown as {
          __TAURI_INTERNALS__: { invoke: (cmd: string, args: unknown) => Promise<unknown> };
        }
      ).__TAURI_INTERNALS__;
      internals.invoke("plugin:event|emit", { event: name, payload: data }).finally(done);
    },
    event,
    payload,
  );
  await settle();
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
  await settle();
}

// --- 007's document viewer ----------------------------------------------------

/** Starts recording `preview:pdf-ready` in the page, the way the frontend's
 * own listener gets it, so a shot waits until the PDF surface is there. Safe
 * to call again: it listens once per page. */
async function recordPdfReady() {
  await browser.execute(() => {
    const page = window as unknown as {
      __hdPdfReady?: number;
      __TAURI_INTERNALS__: {
        transformCallback: (callback: () => void) => number;
        invoke: (cmd: string, args: unknown) => Promise<unknown>;
      };
    };
    if (page.__hdPdfReady !== undefined) return;
    page.__hdPdfReady = 0;
    const handler = page.__TAURI_INTERNALS__.transformCallback(() => {
      page.__hdPdfReady = (page.__hdPdfReady ?? 0) + 1;
    });
    void page.__TAURI_INTERNALS__.invoke("plugin:event|listen", {
      event: "preview:pdf-ready",
      target: { kind: "Any" },
      handler,
    });
  });
}

const pdfReadyCount = () =>
  browser.execute(() => (window as unknown as { __hdPdfReady?: number }).__hdPdfReady ?? 0);

const VIEWER = ".hd-dialog__content--xl";

/** Clicks the document's name in the record's list, which opens the viewer. */
async function openDocument(name: string) {
  await browser.waitUntil(
    () =>
      browser.execute((wanted: string) => {
        const button = [...document.querySelectorAll<HTMLElement>("button.hd-doc__name")].find(
          (b) => b.textContent?.trim() === wanted,
        );
        button?.click();
        return Boolean(button);
      }, name),
    { timeout: 5000, timeoutMsg: `no document "${name}" in the list` },
  );
  await $(`${VIEWER} .hd-dialog__title`).waitForExist();
  await settle();
}

async function closeViewer() {
  await pressEscape();
  await $(VIEWER).waitForExist({ reverse: true });
  await settle();
}

async function waitForViewerStatus(startsWith: string) {
  await browser.waitUntil(
    () =>
      browser.execute(
        (wanted: string) =>
          [...document.querySelectorAll(`.hd-dialog__content--xl [role="status"]`)].some((el) =>
            el.textContent?.trim().startsWith(wanted),
          ),
        startsWith,
      ),
    { timeout: 10000, timeoutMsg: `the viewer never said "${startsWith}"` },
  );
}

/** Puts the focus in the input at `selector` (the first match). Scripted
 * rather than real input, as is all of this walk's, so it runs on every
 * platform. The suggestion lists answer to a scripted focus, change and
 * keydown as to real ones. */
async function focusInput(selector: string) {
  const found = await browser.execute((selector: string) => {
    const input = document.querySelector<HTMLInputElement>(selector);
    input?.focus();
    return Boolean(input);
  }, selector);
  if (!found) throw new Error(`focusInput: nothing matches "${selector}"`);
}

/** Focuses the input at `selector` and replaces its text with `text`, as
 * selecting it all and typing would: the change brings its suggestion list up. */
async function typeInto(selector: string, text: string) {
  await browser.execute(
    (selector: string, text: string) => {
      const input = document.querySelector<HTMLInputElement>(selector)!;
      input.focus();
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(input, text);
      input.dispatchEvent(new Event("input", { bubbles: true }));
    },
    selector,
    text,
  );
}

/** Presses Escape on whatever has focus. It must be cancelable like a real
 * key press: an open suggestion list cancels it, so the dialog stays open. */
async function pressEscapeKey() {
  await browser.execute(() =>
    (document.activeElement ?? document.body).dispatchEvent(
      new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }),
    ),
  );
}

/** Closes the suggestion list of the form field `field` (Caliber by default)
 * if it comes up, which the shots don't want. Moving the focus to Caliber, or
 * typing a cartridge, can bring a list up. Escape closes the list only if it
 * is up: with none, it would close the form. The list's options come from
 * the backend, so a list that is not up yet looks the same as one that never
 * comes: this waits for the focus, then for the app to be idle, and only then
 * decides. */
async function closeListIfOpen(field = "caliber") {
  await browser.waitUntil(
    () =>
      browser.execute(
        (field: string) =>
          Boolean(document.activeElement?.closest(`[role="dialog"] [data-field="${field}"]`)),
        field,
      ),
    { timeout: 5000, timeoutMsg: `the focus never reached ${field}` },
  );
  await settle();
  if (await $('[role="listbox"]').isDisplayed()) {
    await pressEscapeKey();
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
  // The form closes at once, or asks first: wait for either.
  await browser.waitUntil(() =>
    browser.execute(
      () =>
        !document.querySelector('[role="dialog"]') ||
        Boolean(document.querySelector('[role="alertdialog"]')),
    ),
  );
  await settle();
  if (await $('[role="alertdialog"]').isExisting()) await clickButton("Discard changes");
  await $('[role="dialog"]').waitForExist({ reverse: true });
  await settle();
}

/** Scrolls a form field to the middle of the window, so its list or note is in view. */
async function centerField(field: string) {
  await browser.execute((name: string) => {
    document
      .querySelector(`[role="dialog"] [data-field="${name}"]`)
      ?.scrollIntoView({ block: "center" });
  }, field);
  await settle();
}

/** Opens or closes a folded form group (a heading holding a button with
 * `aria-expanded`) in the open dialog, by its title. */
async function setGroup(title: string, open: boolean) {
  await browser.execute(
    (wanted: string, want: boolean) => {
      const button = [
        ...document.querySelectorAll<HTMLElement>('[role="dialog"] button[aria-expanded]'),
      ].find((b) => b.textContent?.trim().startsWith(wanted));
      if (button && (button.getAttribute("aria-expanded") === "true") !== want) button.click();
    },
    title,
    open,
  );
  await settle();
}

/** Opens the "Group by" menu (it opens on pointer down, which a plain click
 * doesn't send) and leaves it open. */
async function openGroupMenu() {
  const trigger = await $('button[aria-haspopup="menu"][aria-label^="Group by"]');
  await trigger.waitForExist();
  await browser.execute((element: HTMLElement) => {
    element.dispatchEvent(
      new PointerEvent("pointerdown", { bubbles: true, button: 0, pointerType: "mouse" }),
    );
  }, trigger);
  await $('[role="menu"]').waitForExist({ timeout: 5000 });
  await settle();
}

/** Opens a menu whose trigger is the button in `scope` reading `label` (a
 * menu opens on pointer down, which a plain click doesn't send). */
async function openMenuButton(scope: string, label: string) {
  await browser.execute(
    (within: string, wanted: string) => {
      const trigger = [...document.querySelectorAll<HTMLElement>(`${within} button`)].find(
        (b) => b.textContent?.trim() === wanted,
      );
      trigger?.dispatchEvent(
        new PointerEvent("pointerdown", { bubbles: true, button: 0, pointerType: "mouse" }),
      );
    },
    scope,
    label,
  );
  await $('[role="menu"]').waitForExist({ timeout: 5000 });
  await settle();
}

/** Chooses the open menu's item whose text is `label`. */
async function chooseOpenMenuItem(label: string) {
  await browser.waitUntil(
    () =>
      browser.execute((wanted: string) => {
        const item = [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')].find(
          (m) => (m.textContent ?? "").trim() === wanted,
        );
        item?.click();
        return Boolean(item);
      }, label),
    { timeout: 5000, timeoutMsg: `no menu item "${label}"` },
  );
  await settle();
}

/** Opens the "Mount on {name}" dialog from the record page's Mounted section,
 * and searches it for `text`. */
async function searchMountDialog(text: string) {
  await openMenuButton(".hd-mounted-section", "Mount");
  await chooseOpenMenuItem("Existing accessory or firearm…");
  await $('[role="dialog"]').waitForDisplayed({ timeout: 5000 });
  await settle();
  await typeInto('[role="dialog"] [role="combobox"]', text);
  await $('[role="listbox"]').waitForDisplayed({ timeout: 5000 });
  await settle();
}

/** Opens the Delete question of the record page (a confirmation, not a form). */
async function openDeleteQuestion() {
  await clickButton("Delete");
  await $('[role="alertdialog"]').waitForExist();
  await settle();
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
      await shot(`31-grouped-by-cartridge-${suffix}`);
      await groupBy("Type");

      await openDialog("Add firearm");
      await typeInto('[data-field="make"] input', "Gl");
      await $('[role="listbox"]').waitForDisplayed({ timeout: 5000 });
      await centerField("make");
      await shot(`28-make-suggestions-${suffix}`);
      await pressEscapeKey();
      await $('[role="listbox"]').waitForExist({ reverse: true });

      // Mounted on (006) pushes Cartridge below the footer: bring it into view first.
      await centerField("cartridge");
      await typeInto('[data-field="cartridge"] input', "9mm");
      await $('[role="listbox"]').waitForDisplayed({ timeout: 5000 });
      await centerField("cartridge");
      await shot(`29-cartridge-suggestions-${suffix}`);
      await pressEscapeKey();
      await $('[role="listbox"]').waitForExist({ reverse: true });

      // A custom cartridge whose bore can be read from its name: the caliber
      // is filled in, marked as a guess, once the focus moves on.
      await typeInto('[data-field="cartridge"] input', ".30 Custom Improved");
      await closeListIfOpen("cartridge");
      await focusInput('[role="dialog"] [data-field="caliber"] input');
      await $(".hd-guess-tag").waitForExist({ timeout: 5000 });
      await closeListIfOpen();
      await centerField("caliber");
      await shot(`30-caliber-guess-${suffix}`);
      await discardForm();

      // Changing the cartridge of a saved firearm suggests a caliber rather
      // than replacing the one on record.
      await openRecord("Savage 110");
      await openDialog("Edit");
      // Mounted on (006) pushes Cartridge below the footer: bring it into view first.
      await centerField("cartridge");
      await typeInto('[data-field="cartridge"] input', "9x19mm Parabellum");
      await closeListIfOpen("cartridge");
      await focusInput('[role="dialog"] [data-field="caliber"] input');
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

    // specs/005-regulated-item-types contracts/ui-registration.md §10. The
    // export dialog's registration note is in 13-export below: the seeded
    // collection has registered firearms, so its note names them.
    it("a suppressor and a registration in the firearm form", async () => {
      // Type Suppressor: Rated cartridge (filled in) and Caliber, the bore,
      // with their hints, no Action, Physical details open without barrel
      // length or capacity; at the default and minimum width.
      await openRecord("Dead Air Sandman-K");
      await openDialog("Edit");
      await openPhysicalGroup();
      await shot(`35-suppressor-form-${suffix}`, { fullPage: true });
      await resizeWindow(800, 1400);
      await centerField("caliber");
      await shot(`36-suppressor-form-minimum-width-${suffix}`);
      await resizeWindow(SCREENSHOT_WINDOW.width, SCREENSHOT_WINDOW.height);
      await discardForm();
      await back();

      // Registration section: closed with its prompt, then open with only
      // "Registered as", on a suppressor with no classification.
      await openRecord("SilencerCo Sparrow 22");
      await openDialog("Edit");
      await setGroup("Registration", false);
      await centerField("registrationClassId");
      await shot(`38-registration-closed-${suffix}`);
      await setGroup("Registration", true);
      await centerField("registrationClassId");
      await shot(`39-registration-open-empty-${suffix}`);
      await discardForm();
      await back();

      // A type change that clears recorded values says so before saving.
      await openRecord("Mossberg 500");
      await openDialog("Edit");
      await choose("Suppressor");
      await centerField("firearmTypeId");
      await shot(`37-type-change-clearing-note-${suffix}`);
      await discardForm();
      await back();

      // A registered record: closed with a summary, open with every detail
      // and the Form list showing the built-in names, and the question before
      // the classification is cleared.
      await openRecord("Dead Air Sandman-K");
      await shot(`41-registration-panel-${suffix}`, { fullPage: true });
      await openDialog("Edit");
      await setGroup("Registration", false);
      await centerField("registrationClassId");
      await shot(`40-registration-closed-summary-${suffix}`);
      await setGroup("Registration", true);
      await centerField("registrationForm");
      await typeInto('[data-field="registrationForm"] input', "F");
      await $('[role="listbox"]').waitForDisplayed({ timeout: 5000 });
      await centerField("registrationForm");
      await shot(`42-registration-open-details-${suffix}`);
      await pressEscapeKey();
      await $('[role="listbox"]').waitForExist({ reverse: true });
      await selectOption("Registered as", "Unspecified");
      await $('[role="alertdialog"]').waitForExist();
      await shot(`43-clear-classification-${suffix}`);
      await clickButton("Keep them");
      await $('[role="alertdialog"]').waitForExist({ reverse: true });

      // The guide at "Registered items", from the form's own link.
      await clickButton("How to record registrations");
      await $(".hd-origin-guide__part").waitForDisplayed({ timeout: 5000 });
      await settle();
      await shot(`44-registered-items-guide-${suffix}`);
      // Only the guide closes: the form under it stays open.
      await browser.execute(() =>
        document.dispatchEvent(
          new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }),
        ),
      );
      await $(".hd-origin-guide__part").waitForExist({ reverse: true });
      await discardForm();
      await back();
    });

    it("grouping by registration, and the suppressor drawing", async () => {
      await openGroupMenu();
      await shot(`45-grouping-menu-${suffix}`);
      await browser.execute(() => {
        [...document.querySelectorAll<HTMLElement>('[role="menuitemradio"]')]
          .find((m) => (m.textContent ?? "").trim() === "Registered as")
          ?.click();
      });
      await $("h2.hd-group__title").waitForExist();
      await shot(`46-grouped-by-registered-as-${suffix}`);
      await groupBy("Type");

      await choose("Tiles");
      await $(".hd-tile__name").waitForExist();
      await browser.execute(() => {
        [...document.querySelectorAll<HTMLElement>(".hd-tile__name")]
          .find((t) => t.textContent?.includes("Sparrow 22"))
          ?.scrollIntoView({ block: "center" });
      });
      await shot(`47-suppressor-tiles-${suffix}`);
      await choose("List");
    });

    // specs/006-accessory-links contracts/ui-accessories.md §13. The
    // collection's mount lines, the Mount menu and the export disclosure are
    // below; the import report with a mount warning is its own walk at the end.
    it("the Accessories page", async () => {
      await goTo("Accessories");
      await $("h1=Accessories").waitForExist();
      await $(".hd-row__name").waitForExist();
      await groupBy("Kind");
      await $("h2.hd-group__title").waitForExist();
      await shot(`48-accessories-list-${suffix}`);

      await choose("Tiles");
      await $(".hd-tile__name").waitForExist();
      await shot(`49-accessories-tiles-${suffix}`);
      await choose("List");

      await groupBy("Mounted on");
      await $("h2.hd-group__title").waitForExist();
      await shot(`50-accessories-grouped-by-mounted-on-${suffix}`, { fullPage: true });
      await groupBy("Kind");
    });

    it("the accessory form", async () => {
      await openDialog("Add accessory");
      await shot(`51-add-accessory-${suffix}`, { fullPage: true });
      await resizeWindow(800, 1400);
      await shot(`52-add-accessory-minimum-width-${suffix}`);
      await resizeWindow(SCREENSHOT_WINDOW.width, SCREENSHOT_WINDOW.height);
      await discardForm();
    });

    it("an accessory record with its chain", async () => {
      // Red dot on a scope on an upper on a receiver: the chain is three deep.
      await openRecord("Holosun HS403B");
      await shot(`53-accessory-record-${suffix}`, { fullPage: true });
      await back();
      await goTo("Collection");
    });

    it("a firearm record with its Mounted section, and mounting", async () => {
      await openRecord("LaRue Tactical PredatAR lower");
      await $(".hd-mounted-section").waitForExist();
      await browser.execute(() =>
        document.querySelector(".hd-mounted-section")?.scrollIntoView({ block: "center" }),
      );
      await shot(`54-firearm-mounted-section-${suffix}`);

      // The unmount question, for the upper that carries a scope and a light.
      await browser.execute(() => {
        document
          .querySelector<HTMLElement>('.hd-mounted-section button[aria-label^="Unmount BCM"]')
          ?.click();
      });
      await $('[role="alertdialog"]').waitForExist({ timeout: 5000 });
      await shot(`66-unmount-confirmation-${suffix}`);
      await clickButton("Cancel");
      await $('[role="alertdialog"]').waitForExist({ reverse: true });
      await back();

      // A rifle with its Mount menu open, and the existing-record dialog
      // listing a record that is mounted elsewhere, then the move question.
      await openRecord("Ruger Hawkeye Hunter");
      await browser.execute(() =>
        document.querySelector(".hd-mounted-section")?.scrollIntoView({ block: "center" }),
      );
      await openMenuButton(".hd-mounted-section", "Mount");
      await shot(`55-mount-menu-${suffix}`);
      await pressEscapeKey();
      await $('[role="menu"]').waitForExist({ reverse: true });

      await searchMountDialog("holo");
      await shot(`56-mount-existing-dialog-${suffix}`);
      await browser.execute(() => {
        [...document.querySelectorAll<HTMLElement>('[role="option"]')]
          .find((o) => (o.textContent ?? "").includes("Holosun"))
          ?.click();
      });
      await $('[role="alertdialog"]').waitForExist({ timeout: 5000 });
      await shot(`57-move-confirmation-${suffix}`);
      await clickButton("Cancel");
      await $('[role="alertdialog"]').waitForExist({ reverse: true });
      // Cancel in the "Mount on" dialog (Escape would close its open list first).
      await clickButton("Cancel");
      await $('[role="dialog"]').waitForExist({ reverse: true });
      await settle();

      // Deleting a rifle that carries a suppressor and an optic.
      await openDeleteQuestion();
      await shot(`59-delete-with-mounted-${suffix}`);
      await clickButton("Cancel");
      await $('[role="alertdialog"]').waitForExist({ reverse: true });
      await back();
    });

    it("disposing of a firearm with mounted records", async () => {
      await openRecord("Aero Precision M4E1 carbine");
      await openDialog("Mark disposed");
      await choose("Dispose with it");
      await shot(`58-dispose-with-mounted-${suffix}`, { fullPage: true });
      await discardForm();
      await back();
    });

    it("the collection with its mount lines", async () => {
      const showRow = (name: string) =>
        browser.execute((wanted: string) => {
          [...document.querySelectorAll<HTMLElement>(".hd-row__name, .hd-tile__name")]
            .find((n) => n.textContent?.includes(wanted))
            ?.scrollIntoView({ block: "center" });
        }, name);
      await showRow("PredatAR lower");
      await shot(`60-collection-mount-lines-${suffix}`);

      await choose("Tiles");
      await $(".hd-tile__name").waitForExist();
      await showRow("PredatAR lower");
      await shot(`61-collection-tiles-mount-lines-${suffix}`);
      await choose("List");
    });

    it("the value summary and the export disclosure", async () => {
      await goTo("Insurance");
      await $(".hd-policy").waitForExist();
      await shot(`62-value-summary-${suffix}`);
      await goTo("Collection");

      await openDialog("Export");
      await shot(`63-export-accessories-disclosure-${suffix}`);
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

    it("database settings, passphrase change and restore", async () => {
      await chooseMenuItem("button.hd-db-menu", "Database settings…");
      await $('[role="dialog"]').waitForExist();
      await shot(`19-database-settings-${suffix}`, { fullPage: true });
      // The seeded backups are at a custom location, so going back to the
      // default asks what to do with them (FR-026). Cancelled: nothing is
      // saved.
      await clickButton("Use the default");
      await clickButton("Save");
      await $("h2=Backups at the old location").waitForExist();
      await shot(`27-backup-location-change-${suffix}`);
      await clickButton("Cancel");
      await $("h2=Backups at the old location").waitForExist({ reverse: true });
      await closeDialog();

      await chooseMenuItem("button.hd-db-menu", "Change passphrase…");
      await $('[role="dialog"] input[autocomplete="current-password"]').waitForExist();
      await shot(`20-change-passphrase-${suffix}`);
      await closeDialog();

      await chooseMenuItem("button.hd-db-menu", "Restore from a backup…");
      await $('[role="dialog"] input[type="radio"]').waitForExist();
      await shot(`21-restore-backup-${suffix}`, { fullPage: true });
      await closeDialog();

      await chooseMenuItem("button.hd-db-menu", "About databases and security");
      await $('[role="dialog"] .hd-db-guide').waitForExist();
      await shot(`23-database-guide-${suffix}`);
      // How this database is set up, beside the defaults.
      await browser.execute(() =>
        document.querySelector(".hd-db-guide__settings")?.scrollIntoView({ block: "center" }),
      );
      await shot(`26-database-guide-settings-${suffix}`);
      await closeDialog();
    });

    // specs/007-document-preview contracts/ui-document-preview.md §10: the
    // Glock's document list, and the viewer on each kind of document.
    it("the document list and the viewer", async () => {
      await recordPdfReady();
      await openRecord(RECORD);
      // The whole record: all eight rows of the list, down to the pre-007 JPEG.
      await shot(`67-document-list-${suffix}`, { fullPage: true });

      const ready = await pdfReadyCount();
      await openDocument("Purchase receipt.pdf");
      await browser.waitUntil(async () => (await pdfReadyCount()) > ready, {
        timeout: 15000,
        timeoutMsg: "the PDF never became ready",
      });
      await shotDisplay(`68-viewer-pdf-${suffix}`);
      await closeViewer();

      // The TIFF screen shows a scan that looks like one (US Letter, 200 DPI,
      // three pages), on the suppressor's record; the Glock's own TIFF is a
      // tiny test image the viewer has to zoom a long way.
      await back();
      await openRecord("Dead Air Sandman-K");
      await openDocument("Bill of sale (scan).tif");
      await $(`${VIEWER} img[alt="Bill of sale (scan).tif, page 1 of 3"]`).waitForExist({
        timeout: 10000,
      });
      await shot(`69-viewer-tiff-${suffix}`);
      await closeViewer();
      await back();
      await openRecord(RECORD);

      await openDocument("owners-manual-notes.txt");
      await $(`${VIEWER} pre.hd-preview__text`).waitForExist({ timeout: 10000 });
      await shot(`70-viewer-text-${suffix}`);
      await closeViewer();

      await openDocument("Bill of sale.docx");
      await waitForViewerStatus("Bill of sale.docx can't be previewed here.");
      await shot(`71-viewer-cant-preview-${suffix}`);
      await closeViewer();

      await back();
    });

    it("pending changes after a lock", async () => {
      // A lock with an edit under way keeps it, and the next open asks.
      await goTo("Collection");
      await openRecord(RECORD);
      await openDialog("Edit");
      await fill("Notes", "Swapped the grips for the walnut set.");
      // The form stages its draft 250 ms after the last edit, or at once when
      // the window loses focus (usePendingDraft.ts), a timer the busy count
      // doesn't see. Send that blur, so the staging call is in flight for
      // settle() to wait on.
      await browser.execute(() => window.dispatchEvent(new Event("blur")));
      await settle();
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
      await shot(`22-pending-changes-${suffix}`);
      await clickButton("Discard changes");
      await $('[role="alertdialog"]').waitForExist();
      await clickButton("Discard changes");
      await $('nav[aria-label="Sections"]').waitForExist({ timeout: 10000 });
      await settle();
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
    const samples = path.join(
      path.dirname(process.env.HOPLODEX_E2E_CONFIG_HOME!),
      "import-samples",
    );
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
    await settle();
    for (const theme of ["Light", "Dark"] as const) {
      await chooseTheme(theme);
      await shot(`34-import-report-${theme.toLowerCase()}`, { fullPage: true });
    }
    await closeDialog();
    await chooseTheme("Light");
  });
});

// specs/006-accessory-links: the import report with the four mount warnings,
// in both themes, with the report left open.
describe("Screenshots: the import report with mount warnings", () => {
  it("lists the mount warnings and the table of each row", async () => {
    await goTo("Collection");
    await chooseTheme("Light");
    const samples = path.join(
      path.dirname(process.env.HOPLODEX_E2E_CONFIG_HOME!),
      "import-samples",
    );
    await clickButton("Import");
    await fill("Spreadsheet file", path.join(samples, "import-accessory-mount-warnings.csv"));
    await clickButton("Import");
    await $(".hd-tally").waitForExist({ timeout: 15000, timeoutMsg: "import never finished" });
    await browser.execute(() => {
      for (const trigger of document.querySelectorAll<HTMLElement>(".hd-disclosure__trigger")) {
        if (trigger.getAttribute("aria-expanded") === "false") trigger.click();
      }
    });
    await settle();
    for (const theme of ["Light", "Dark"] as const) {
      await chooseTheme(theme);
      await shot(`64-import-report-mount-warnings-${theme.toLowerCase()}`);
    }
    await closeDialog();
    await chooseTheme("Light");
  });
});

// Issue #56: replacing a record with a row that marks it disposed asks which
// of the records mounted on it go with it. The seeded "Stripped lower"
// carries an upper (with a scope that carries a red dot, and a light). The
// confirmation is shot with the upper chosen to go, then cancelled, so the
// seed is left as it was.
describe("Screenshots: replacing a record with a disposed row", () => {
  it("asks about each record mounted on it", async () => {
    await goTo("Collection");
    await chooseTheme("Light");
    const samples = path.join(
      path.dirname(process.env.HOPLODEX_E2E_CONFIG_HOME!),
      "import-samples",
    );
    await clickButton("Import");
    await fill("Spreadsheet file", path.join(samples, "import-dispose-receiver.csv"));
    await clickButton("Import");
    await $(".hd-tally").waitForExist({ timeout: 15000, timeoutMsg: "import never finished" });
    await choose("Replace existing");
    await clickButton("Apply decisions");
    await $('[role="alertdialog"]').waitForExist();
    await choose("Dispose with it");
    for (const theme of ["Light", "Dark"] as const) {
      await chooseTheme(theme);
      await shot(`65-import-replace-disposed-${theme.toLowerCase()}`);
    }
    await clickButton("Cancel");
    await $('[role="alertdialog"]').waitForExist({ reverse: true });
    await closeDialog();
    await chooseTheme("Light");
  });
});

// specs/007-document-preview FR-003a: a computer whose PDF viewer can't be
// used safely. HOPLODEX_E2E_PDF_PREVIEW=off is an E2E-only switch the app reads
// when it starts, so this one relaunches it (last in the walk: nothing after it
// wants the default back, and it is put back anyway).
describe("Screenshots: PDFs can't be previewed on this computer", () => {
  after(async () => {
    delete process.env.HOPLODEX_E2E_PDF_PREVIEW;
  });

  it("says so in the viewer", async () => {
    process.env.HOPLODEX_E2E_PDF_PREVIEW = "off";
    await relaunchApp();
    await waitForChooser();
    const select = await $('button.hd-db-row__select[aria-label^="Main collection, "]');
    if (await select.isExisting()) await select.click();
    await unlock(process.env.HOPLODEX_E2E_SEED_PASSPHRASE!);
    await goTo("Collection");
    await openRecord(RECORD);
    await openDocument("Purchase receipt.pdf");
    await waitForViewerStatus("PDFs can't be previewed on this computer.");
    for (const theme of ["Light", "Dark"] as const) {
      await chooseTheme(theme);
      await shot(`72-viewer-pdf-unavailable-${theme.toLowerCase()}`);
    }
    await closeViewer();
    await chooseTheme("Light");
  });
});
