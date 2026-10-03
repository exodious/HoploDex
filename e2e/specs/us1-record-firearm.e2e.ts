import { $, addFirearm, back, browser, clickButton, choose, expect, fill, settle } from "../support/ui";
import {
  backLinkShown,
  clickEl,
  clickPinned,
  displayName,
  fieldValue,
  fillFirearmForm,
  focusedFieldLabel,
  followAddLink,
  hasHighlightedSection,
  isButtonDisabled,
  isFieldDisabled,
  isFieldInView,
  listedNames,
  openPhysicalGroup,
  openFirearm,
  pasteInto,
  pinnedStrip,
  pressEscape,
  scrollToPinnedStrip,
  selectOption,
  titleBlock,
  toggle,
} from "../support/ui";
import type { NewFirearm } from "../support/ui";
import { createDatabase } from "../support/ui";

/**
 * End-to-end coverage of User Story 1's acceptance scenarios (spec.md),
 * driven against the real built app through its embedded WebDriver server —
 * no mocks, exercising the full stack (React UI -> Tauri IPC -> SQLCipher).
 * See e2e/support/ui.ts for why interactions go through page JS.
 *
 * Creating a firearm opens its record page, so scenarios assert against
 * the record right after creation.
 */
describe("User Story 1 - Record a Firearm", () => {
  // Each spec's session starts at the chooser with no databases (wdio.conf.ts).
  before(async () => {
    await createDatabase();
  });

  it("creates a firearm and shows it with its values intact (Scenario 1)", async () => {
    await addFirearm({
      make: "Glock",
      model: "19",
      caliber: "9mm",
      type: "Handgun",
      serial: "E2E-001",
    });

    await expect($("#record-name")).toHaveText("Glock 19");
    await expect($(".hd-plate__serial")).toHaveText("E2E-001");
    expect(await titleBlock("Caliber")).toBe("9mm");
  });

  it("persists an edited field on reopen (Scenario 2)", async () => {
    await clickButton("Edit");
    await fill("Notes", "scratch on left side");
    await clickButton("Save changes");

    await expect($(".hd-textblock*=scratch on left side")).toExist();

    // Reopen from the collection to confirm persistence beyond the
    // in-memory record state.
    await back();
    await openFirearm("Glock 19");
    await expect($(".hd-textblock*=scratch on left side")).toExist();
  });

  it("saves acquisition details and shows them on the record (Scenario 3)", async () => {
    await clickButton("Edit");
    await fill("Acquired from", "Local gun shop");
    await fill("Date acquired", "2025-03-01");
    await fill("Price paid", "1450");
    await clickButton("Save changes");

    await expect($("dd*=Local gun shop")).toExist();
    // Entered as plain digits, shown grouped and with no cents (FR-037).
    await expect($("dd=$1,450")).toExist();
  });

  it("marks a firearm disposed while retaining its history (Scenario 4)", async () => {
    await clickButton("Mark disposed");
    await choose("Sold");
    await fill("Transferred to", "Jane Doe");
    await fill("Date", "2025-06-15");
    await fill("Price received", "400");
    await clickButton("Mark as disposed");

    expect(await titleBlock("Status")).toContain("Sold");
    // History retained: identifying and acquisition details still shown.
    await expect($(".hd-plate__serial")).toHaveText("E2E-001");
    await expect($("dd*=Jane Doe")).toExist();
    await expect($("dd*=Local gun shop")).toExist();

    // FR-025: hidden from the collection by default, shown on request.
    await back();
    expect(await listedNames()).not.toContain("Glock 19");
    await toggle("Show disposed");
    expect(await listedNames()).toContain("Glock 19");
    await toggle("Show disposed");
  });

  it("deletes a firearm only after confirmation (Scenario 5)", async () => {
    await addFirearm({
      make: "ToDelete",
      model: "X",
      caliber: ".22",
      type: "Other",
      serial: "DEL-1",
    });

    await clickButton("Delete");
    await expect($('[role="alertdialog"]')).toExist();
    await clickButton("Cancel");
    await expect($("#record-name")).toHaveText("ToDelete X");

    await clickButton("Delete");
    await clickButton("Delete firearm");

    // Deleting returns to the collection, without the firearm.
    await $(".hd-page-title").waitForExist();
    await settle();
    expect(await listedNames()).not.toContain("ToDelete X");
  });

  it("saves a blank serial number once attested (Scenario 6)", async () => {
    await addFirearm({
      make: "Homemade",
      model: "80% build",
      caliber: ".223",
      type: "Rifle",
      noSerial: true,
    });

    await expect($(".hd-plate__no-serial")).toExist();
    await back();
  });

  it("blocks saving a blank, unattested serial number (Scenario 7)", async () => {
    await clickButton("Add firearm");
    await fill("Make", "Blocked");
    await fill("Model", "Case");
    await choose("Handgun");
    await fill("Caliber", "9mm");
    await clickButton("Add firearm");

    await expect($("p*=Enter a serial number, or confirm this firearm has none.")).toExist();
    await expect($('[role="dialog"]')).toExist();
    await expect($("#record-name")).not.toExist();
    await clickButton("Cancel");
  });

  // --- Scenarios 8-14: nicknames, uniqueness, reversal, dates ---

  const twin = (serial: string, nickname?: string): NewFirearm => ({
    make: "E2ENick",
    model: "Twin",
    caliber: "9mm",
    type: "Handgun",
    serial,
    nickname,
  });

  /** Opens the Add firearm dialog and fills it, without saving. */
  async function startAdding(firearm: NewFirearm) {
    await clickButton("Add firearm");
    await $('[role="dialog"]').waitForExist();
    await fillFirearmForm(firearm);
  }

  async function markDisposed(price = "1") {
    await clickButton("Mark disposed");
    await choose("Sold");
    await fill("Transferred to", "Jane Doe");
    await fill("Price received", price);
    await clickButton("Mark as disposed");
    await $('[role="alertdialog"], [role="dialog"]').waitForExist({ reverse: true });
    await settle();
  }

  it("tells identical firearms apart by nickname, and leaves a blank one alone (Scenario 8)", async () => {
    await addFirearm(twin("N-1", "Range gun"));
    await back();
    await addFirearm(twin("N-2", "Carry gun"));
    await back();
    await addFirearm(twin("N-3"));
    await back();

    const names = await listedNames();
    expect(names).toContain("E2ENick Twin “Range gun”");
    expect(names).toContain("E2ENick Twin “Carry gun”");
    expect(names).toContain("E2ENick Twin");
  });

  it("blocks a duplicate nickname, ignoring case, until the first firearm is disposed (Scenario 9)", async () => {
    await startAdding({ ...twin("N-4", "range GUN"), model: "Third" });
    await clickButton("Add firearm");

    await expect($('[role="dialog"]*=already used by E2ENick Twin')).toExist();
    await expect($("#record-name")).not.toExist();
    await clickButton("Cancel");

    await openFirearm("E2ENick Twin “Range gun”");
    await markDisposed();
    await back();

    await addFirearm({ ...twin("N-4", "Range gun"), model: "Third" });
    await back();
    expect(await listedNames()).toContain("E2ENick Third “Range gun”");
  });

  it("blocks a duplicate make/model/serial, naming it, until the first is disposed (Scenario 10)", async () => {
    const pistol: NewFirearm = {
      make: "E2EDup",
      model: "Pistol",
      caliber: "9mm",
      type: "Handgun",
      serial: "D-1",
    };
    await addFirearm(pistol);
    await back();

    await startAdding(pistol);
    await clickButton("Add firearm");
    await expect($('[role="dialog"]*=already has this make, model and serial number')).toExist();
    await clickButton("Cancel");

    await openFirearm("E2EDup Pistol");
    await markDisposed();
    await back();
    await addFirearm(pistol); // reacquired: a new record
    await back();
  });

  it("takes a no-serial firearm out of the duplicate check and locks its serial field (Scenario 11)", async () => {
    const rifle: NewFirearm = {
      make: "E2EExempt",
      model: "Rifle",
      caliber: ".30-06",
      type: "Rifle",
      serial: "X-1",
    };
    await addFirearm(rifle);
    await back();

    // A serial typed before the box is checked is discarded with it.
    await startAdding({ ...rifle, noSerial: true });
    expect(await isFieldDisabled("Serial number")).toBe(true);
    await clickButton("Add firearm");
    await browser.waitUntil(async () => await $("#record-name").isExisting(), {
      timeoutMsg: "the no-serial firearm was not saved",
    });
    await expect($(".hd-plate__no-serial")).toExist();
    await back();

    // Unchecking the box brings the serial requirement back.
    await clickButton("Add firearm");
    await $('[role="dialog"]').waitForExist();
    await fill("Make", "E2EExempt");
    await fill("Model", "Rifle");
    await choose("Rifle");
    await fill("Caliber", ".30-06");
    await toggle("This firearm has no serial number");
    await toggle("This firearm has no serial number");
    await clickButton("Add firearm");
    await expect($("p*=Enter a serial number, or confirm this firearm has none.")).toExist();
    await clickButton("Cancel");
  });

  it("reverses a disposition, blocked while its nickname is taken, then keeps it as history (Scenario 12)", async () => {
    // Twin "Range gun" was disposed above, and Third now holds that nickname.
    await toggle("Show disposed");
    await openFirearm("E2ENick Twin “Range gun”");
    await clickButton("Restore to collection");
    await expect($('[role="alertdialog"]')).toExist();

    // The keep/discard choice is never made for the user.
    expect(await isButtonDisabled("Restore to collection")).toBe(true);
    await choose("Keep as history");
    await clickButton("Restore to collection");
    await expect($('[role="alertdialog"]*=already used by E2ENick Third')).toExist();

    // Rename in the same step to resolve the clash.
    await fill("New nickname", "Range gun II");
    await clickButton("Restore to collection");
    await browser.waitUntil(
      async () =>
        (await $("#record-name").getText()).replace(/\s+/g, " ") ===
        displayName({ make: "E2ENick", model: "Twin", nickname: "Range gun II" }),
      { timeoutMsg: "the restored firearm never showed its new nickname" },
    );

    expect(await titleBlock("Status")).toBe("Active");
    await expect($("h3=Earlier dispositions")).toExist();
    await expect($(".hd-past-dispositions*=Jane Doe")).toExist();
    await back();
    await toggle("Show disposed");
  });

  it("rejects future acquisition dates and a disposition before the acquisition (Scenarios 13-14)", async () => {
    const tomorrow = new Date();
    tomorrow.setDate(tomorrow.getDate() + 1);
    const pad = (n: number) => String(n).padStart(2, "0");
    const future = `${tomorrow.getFullYear()}-${pad(tomorrow.getMonth() + 1)}-${pad(tomorrow.getDate())}`;
    const today = new Date();
    const todayIso = `${today.getFullYear()}-${pad(today.getMonth() + 1)}-${pad(today.getDate())}`;

    const dated: NewFirearm = {
      make: "E2EDates",
      model: "One",
      caliber: "9mm",
      type: "Handgun",
      serial: "DT-1",
    };
    await startAdding({ ...dated, acquisitionDate: future });
    await clickButton("Add firearm");
    await expect($(`[role="dialog"]*=Acquisition date can't be in the future`)).toExist();
    await clickButton("Cancel");

    // Today is fine.
    await addFirearm({ ...dated, acquisitionDate: todayIso });

    // Dispose it before it was acquired: blocked, naming the field.
    await clickButton("Mark disposed");
    await choose("Sold");
    await fill("Transferred to", "Jane Doe");
    await fill("Date", "2000-01-01");
    await fill("Price received", "1");
    await clickButton("Mark as disposed");
    await expect($('[role="dialog"]*=earlier than the acquisition date')).toExist();
    await clickButton("Cancel");
    expect(await titleBlock("Status")).toBe("Active");
  });
  it("opens the edit form on the notes or accessories field from an Add link (Scenario 15)", async () => {
    // Scenarios 13-14 end on a record; Add firearm lives on the collection.
    await back();
    await addFirearm({
      make: "E2EAdd",
      model: "Links",
      caliber: "9mm",
      type: "Handgun",
      serial: "ADD-1",
    });

    for (const [emptyText, label] of [
      ["No notes recorded.", "Notes"],
      ["No accessories recorded.", "Accessories"],
    ] as const) {
      await followAddLink(emptyText);
      await $('[role="dialog"]').waitForExist();
      await browser.waitUntil(async () => (await focusedFieldLabel()) === label, {
        timeoutMsg: `the ${label} field was not focused`,
      });
      // The dialog scrolls the field into view smoothly, which `settle()`
      // can't see: wait for the scroll to land.
      await browser.waitUntil(() => isFieldInView(label), {
        timeoutMsg: `the ${label} field was not scrolled into view`,
      });
      expect(await hasHighlightedSection()).toBe(true);
      await clickButton("Cancel");
    }

    // The plain Edit button lands on the first field, with nothing highlighted.
    await clickButton("Edit");
    await $('[role="dialog"]').waitForExist();
    expect(await focusedFieldLabel()).toBe("Make");
    expect(await hasHighlightedSection()).toBe(false);
    await clickButton("Cancel");
    await back();
  });

  it("takes amounts as whole dollars only and shows them grouped (Scenario 16)", async () => {
    await clickButton("Add firearm");
    await $('[role="dialog"]').waitForExist();
    await fillFirearmForm({
      make: "E2EMoney",
      model: "Whole",
      caliber: "9mm",
      type: "Handgun",
      serial: "MNY-1",
    });

    // A decimal point, comma or other character is not accepted as typed.
    await fill("Estimated replacement value", "1,2.5x0");
    expect(await fieldValue("Estimated replacement value")).toBe("1250");

    // A pasted amount with cents is refused with a message and changes nothing.
    await pasteInto("Estimated replacement value", "1250.50");
    expect(await fieldValue("Estimated replacement value")).toBe("1250");
    await expect($('[role="dialog"]*=whole dollars')).toExist();

    // A pasted "$" and thousands commas are dropped.
    await fill("Estimated replacement value", "");
    await pasteInto("Estimated replacement value", "$1,250");
    expect(await fieldValue("Estimated replacement value")).toBe("1250");
    await expect($('[role="dialog"]*=whole dollars')).not.toExist();

    await clickButton("Add firearm");
    await $("#record-name").waitForExist();

    // Only the digits were entered and stored; the record shows "$1,250".
    expect(await titleBlock("Replacement value")).toBe("$1,250");

    // Editing shows the digits again, never the grouping.
    await clickButton("Edit");
    await $('[role="dialog"]').waitForExist();
    expect(await fieldValue("Estimated replacement value")).toBe("1250");
    await clickButton("Cancel");
    await back();
  });

  it("records physical details, rounds excess precision, and hides the panel when cleared (Scenario 17)", async () => {
    await clickButton("Add firearm");
    await $('[role="dialog"]').waitForExist();
    await fillFirearmForm({
      make: "E2EPhys",
      model: "Details",
      caliber: "5.56",
      type: "Rifle",
      serial: "PHYS-1",
    });
    await openPhysicalGroup();
    await fill("Barrel length (in)", "16.25");
    await fill("Overall length (in)", "36");
    await fill("Weight (lb)", "2");
    await fill("Weight (oz)", "8.5");
    await fill("Capacity", "30");
    await fill("Finish", "Cerakote flat dark earth");
    await selectOption("Condition", "Excellent");
    await clickButton("Add firearm");
    await $("#record-name").waitForExist();

    // Reopened from the collection, the record shows them.
    await back();
    await openFirearm("E2EPhys Details");
    const panel = await $('section[aria-labelledby="physical-title"]');
    await panel.waitForExist();
    const shown = (await panel.getText()).replace(/\s+/g, " ");
    for (const text of ["16.25 in", "36 in", "2 lb 8.5 oz", "30 rounds", "Cerakote", "Excellent"]) {
      expect(shown).toContain(text);
    }

    // Extra decimal places are rounded to the stored unit, not rejected:
    // 16.255 in is saved as 16.26, and 2.53 lb (40.48 oz) as 2 lb 8.5 oz.
    await clickButton("Edit");
    await $('[role="dialog"]').waitForExist();
    await fill("Barrel length (in)", "16.255");
    await fill("Weight (lb)", "2.53");
    await fill("Weight (oz)", "");
    await clickButton("Save changes");
    await $('[role="dialog"]').waitForExist({ reverse: true });
    await browser.waitUntil(
      async () =>
        (await $('section[aria-labelledby="physical-title"]').getText())
          .replace(/\s+/g, " ")
          .includes("16.26 in"),
      { timeoutMsg: "the rounded barrel length never showed" },
    );
    expect(
      (await $('section[aria-labelledby="physical-title"]').getText()).replace(/\s+/g, " "),
    ).toContain("2 lb 8.5 oz");

    // Clearing all six saves normally and the panel disappears.
    await clickButton("Edit");
    await $('[role="dialog"]').waitForExist();
    for (const label of [
      "Barrel length (in)",
      "Overall length (in)",
      "Weight (lb)",
      "Weight (oz)",
      "Capacity",
      "Finish",
    ]) {
      await fill(label, "");
    }
    await selectOption("Condition", "Not recorded");
    await clickButton("Save changes");
    await $('[role="dialog"]').waitForExist({ reverse: true });
    await $('section[aria-labelledby="physical-title"]').waitForExist({ reverse: true });
    await back();
  });

  it("goes back with Escape, and keeps a long record's name and actions in reach (Scenario 18)", async () => {
    // Enough notes to make the record scroll well past its plate.
    const notes = Array.from({ length: 80 }, (_, i) => `Range note ${i + 1}.`).join("\n");
    await addFirearm({
      make: "E2EEsc",
      model: "Long",
      caliber: "9mm",
      type: "Handgun",
      serial: "ESC-1",
      notes,
    });
    // FR-040: the back link shows its shortcut, and Escape does what it does.
    expect(await backLinkShown()).toEqual({ label: "Collection", key: "Esc" });
    await pressEscape();
    await expect($("#record-name")).not.toExist();
    await expect($(".hd-page-title")).toHaveText("Collection");

    // With a dialog open, Escape closes the dialog and stays on the record.
    await openFirearm("E2EEsc Long");
    await clickButton("Edit");
    await $('[role="dialog"]').waitForExist();
    await pressEscape();
    await $('[role="dialog"]').waitForExist({ reverse: true });
    await expect($("#record-name")).toHaveText("E2EEsc Long");

    // With a menu open inside it, Escape closes only the menu.
    await clickButton("Edit");
    await $('[role="dialog"]').waitForExist();
    await openPhysicalGroup();
    await browser.execute(() => {
      const label = [...document.querySelectorAll('[role="dialog"] label')].find(
        (l) => l.textContent?.trim() === "Condition",
      ) as HTMLLabelElement | undefined;
      document.getElementById(label?.htmlFor ?? "")?.click();
    });
    await $('[role="listbox"]').waitForExist();
    await pressEscape();
    await $('[role="listbox"]').waitForExist({ reverse: true });
    await expect($('[role="dialog"]')).toExist();
    await pressEscape();
    await $('[role="dialog"]').waitForExist({ reverse: true });
    await expect($("#record-name")).toHaveText("E2EEsc Long");

    // FR-041: scrolled down, the strip keeps the way back, the name and the
    // heading's actions.
    await scrollToPinnedStrip();
    expect(await pinnedStrip()).toEqual({
      back: "Collection",
      backKey: "Esc",
      name: "E2EEsc Long",
      stamp: "ESC-1",
      actions: ["Edit", "Mark disposed", "Delete"],
    });

    await clickPinned("Edit");
    await expect($('[role="dialog"]*=Edit E2EEsc Long')).toExist();
    await pressEscape();
    await $('[role="dialog"]').waitForExist({ reverse: true });
    await expect($(".hd-runhead")).toExist();

    // The name returns to the top.
    await clickEl(".hd-runhead__title");
    await browser.waitUntil(() => browser.execute(() => window.scrollY === 0), {
      timeoutMsg: "the page never returned to the top",
    });
    await $(".hd-runhead").waitForExist({ reverse: true });
    await expect($("#record-name")).toBeFocused();

    await pressEscape();
    await expect($(".hd-page-title")).toHaveText("Collection");
  });
});
