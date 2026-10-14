import {
  $,
  addFirearm,
  back,
  browser,
  clickButton,
  choose,
  expect,
  fill,
  settle,
} from "../support/ui";
import {
  clickEl,
  clickPinned,
  displayName,
  fillFirearmForm,
  listedNames,
  openFirearm,
  pinnedStrip,
  scrollToPinnedStrip,
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
 * Four tests: the story's journey, one field error for each of the two
 * forms that show a backend refusal on a field (FirearmForm's nickname and
 * RestoreDialog's New nickname), and the record page's real-layout pinned
 * strip. Every other rule is checked in Vitest and the Rust integration
 * tests, each citing its scenario.
 *
 * Creating a firearm opens its record page, so scenarios assert against
 * the record right after creation.
 */
describe("User Story 1 - Record a Firearm", () => {
  // Each spec's session starts at the chooser with no databases (wdio.conf.ts).
  before(async () => {
    await createDatabase();
  });

  /** The error shown for the field labelled `label` in the open dialog: the
   * text of the element its `aria-describedby` names when it is
   * `aria-invalid`, or `null` when the field shows none. */
  async function dialogFieldError(label: string): Promise<string | null> {
    return browser.execute((label: string) => {
      const labelEl = [
        ...document.querySelectorAll('[role="dialog"] label, [role="alertdialog"] label'),
      ].find((l) => l.textContent?.trim() === label) as HTMLLabelElement | undefined;
      const field = labelEl && document.getElementById(labelEl.htmlFor);
      if (!field || field.getAttribute("aria-invalid") !== "true") return null;
      for (const id of (field.getAttribute("aria-describedby") ?? "").split(" ")) {
        const el = document.getElementById(id);
        if (el?.classList.contains("hd-field__error")) return el.textContent?.trim() ?? "";
      }
      return null;
    }, label);
  }

  /** Waits for the labelled field in the open dialog to show an error, and
   * returns its text. */
  async function waitForFieldError(label: string): Promise<string> {
    let text: string | null = null;
    await browser.waitUntil(
      async () => {
        text = await dialogFieldError(label);
        return text !== null;
      },
      { timeoutMsg: `the "${label}" field never showed an error` },
    );
    return text!;
  }

  it("records, edits, disposes and deletes a firearm (Scenarios 1-5)", async () => {
    // Scenario 1: created, and shown with its values intact.
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

    // Scenario 2: an edited field persists on reopen.
    await clickButton("Edit");
    await fill("Notes", "scratch on left side");
    await clickButton("Save changes");

    await expect($(".hd-textblock*=scratch on left side")).toExist();

    // Reopen from the collection to confirm persistence beyond the
    // in-memory record state.
    await back();
    await openFirearm("Glock 19");
    await expect($(".hd-textblock*=scratch on left side")).toExist();

    // Scenario 3: acquisition details are saved and shown.
    await clickButton("Edit");
    await fill("Acquired from", "Local gun shop");
    await fill("Date acquired", "2025-03-01");
    await fill("Price paid", "1450");
    await clickButton("Save changes");

    await expect($("dd*=Local gun shop")).toExist();
    // Entered as plain digits, shown grouped and with no cents (FR-037).
    await expect($("dd=$1,450")).toExist();

    // Scenario 4: disposed, with its history retained.
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

    // Scenario 5: deleted after confirmation, and gone from the collection.
    await addFirearm({
      make: "ToDelete",
      model: "X",
      caliber: ".22",
      type: "Other",
      serial: "DEL-1",
    });

    await clickButton("Delete");
    await expect($('[role="alertdialog"]')).toExist();
    await clickButton("Delete firearm");

    // Deleting returns to the collection, without the firearm.
    await $(".hd-page-title").waitForExist();
    await settle();
    expect(await listedNames()).not.toContain("ToDelete X");
  });

  // --- Nicknames, and reversing a disposition ---

  const twin = (serial: string, nickname?: string): NewFirearm => ({
    make: "E2ENick",
    model: "Twin",
    caliber: "9mm",
    type: "Handgun",
    serial,
    nickname,
  });

  it("refuses a duplicate nickname on the Nickname field (Scenario 9)", async () => {
    await addFirearm(twin("N-1", "Range gun"));
    await back();

    await clickButton("Add firearm");
    await $('[role="dialog"]').waitForExist();
    await fillFirearmForm({ ...twin("N-4", "Range gun"), model: "Third" });
    await clickButton("Add firearm");

    // The backend's refusal lands on the Nickname field, not just the dialog.
    expect(await waitForFieldError("Nickname")).toContain("already used by E2ENick Twin");
    await expect($("#record-name")).not.toExist();
    await clickButton("Cancel");
  });

  it("blocks a reversal while its nickname is taken, on the New nickname field, until it is renamed (Scenario 12)", async () => {
    // A disposed firearm, and an active one that holds its nickname.
    await addFirearm({
      make: "E2ERestore",
      model: "Twin",
      caliber: "9mm",
      type: "Handgun",
      serial: "R-1",
      nickname: "Carry gun",
    });
    await clickButton("Mark disposed");
    await choose("Sold");
    await fill("Transferred to", "Jane Doe");
    await fill("Price received", "1");
    await clickButton("Mark as disposed");
    await $('[role="alertdialog"], [role="dialog"]').waitForExist({ reverse: true });
    await settle();
    await back();
    await addFirearm({
      make: "E2ERestore",
      model: "Third",
      caliber: "9mm",
      type: "Handgun",
      serial: "R-2",
      nickname: "Carry gun",
    });
    await back();

    await toggle("Show disposed");
    await openFirearm("E2ERestore Twin “Carry gun”");
    await clickButton("Restore to collection");
    await expect($('[role="alertdialog"]')).toExist();

    await choose("Keep as history");
    await clickButton("Restore to collection");
    // The clash is shown on the New nickname field it asks to change.
    expect(await waitForFieldError("New nickname")).toContain("already used by E2ERestore Third");

    // Rename in the same step to resolve the clash.
    await fill("New nickname", "Carry gun II");
    await clickButton("Restore to collection");
    await browser.waitUntil(
      async () =>
        (await $("#record-name").getText()).replace(/\s+/g, " ") ===
        displayName({ make: "E2ERestore", model: "Twin", nickname: "Carry gun II" }),
      { timeoutMsg: "the restored firearm never showed its new nickname" },
    );
    expect(await titleBlock("Status")).toBe("Active");
    await back();
    await toggle("Show disposed");
  });

  it("keeps a long record's name and actions in reach as it scrolls (Scenario 18)", async () => {
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

    // #47: the strip takes over the actions the moment their bar has gone
    // under the top bar, long before the plate under it is scrolled away.
    // Scrolled to leave 10px of the bar showing, there is no strip yet.
    const scrollBarTo = (gap: number) =>
      browser.execute((gap: number) => {
        const topbar = document.querySelector(".hd-topbar")!.getBoundingClientRect().bottom;
        const bar = document.querySelector(".hd-record__bar")!.getBoundingClientRect();
        window.scrollTo({ top: window.scrollY + bar.bottom - topbar - gap, behavior: "instant" });
      }, gap);
    const barAndPlate = () =>
      browser.execute(() => {
        const topbar = document.querySelector(".hd-topbar")!.getBoundingClientRect().bottom;
        return {
          barBottom:
            document.querySelector(".hd-record__bar")!.getBoundingClientRect().bottom - topbar,
          plateShown: document.querySelector(".hd-plate")!.getBoundingClientRect().bottom - topbar,
        };
      });
    await scrollBarTo(10);
    await settle();
    expect((await barAndPlate()).barBottom).toBeGreaterThan(5);
    expect(await $(".hd-runhead").isExisting()).toBe(false);
    // Scrolled until the bar is 4px under the top bar: the strip shows with
    // the plate still most of the way in view.
    await scrollBarTo(-4);
    await $(".hd-runhead").waitForExist({ timeoutMsg: "the strip never showed past the bar" });
    await settle();
    const past = await barAndPlate();
    expect(past.barBottom).toBeLessThan(0);
    expect(past.plateShown).toBeGreaterThan(100);
    expect((await pinnedStrip()).actions).toEqual(["Edit", "Mark disposed", "Delete"]);
    // The top bar is solid: the record's own Edit and Mark disposed buttons,
    // scrolled under it, must not show through next to Import and Export.
    const topbarBackground = await browser.execute(
      () => getComputedStyle(document.querySelector(".hd-topbar")!).backgroundColor,
    );
    const alpha = /\/\s*([\d.]+)(%?)\s*\)$|^rgba\([^)]*,\s*([\d.]+)\s*\)$/.exec(topbarBackground);
    const topbarAlpha = alpha
      ? alpha[1]
        ? Number(alpha[1]) / (alpha[2] ? 100 : 1)
        : Number(alpha[3])
      : 1;
    expect({ background: topbarBackground, alpha: topbarAlpha }).toEqual({
      background: topbarBackground,
      alpha: 1,
    });
    await browser.execute(() => window.scrollTo({ top: 0, behavior: "instant" }));
    await $(".hd-runhead").waitForExist({ reverse: true });

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
    await clickButton("Cancel");
    await $('[role="dialog"]').waitForExist({ reverse: true });
    await expect($(".hd-runhead")).toExist();

    // The name returns to the top.
    await clickEl(".hd-runhead__title");
    await browser.waitUntil(() => browser.execute(() => window.scrollY === 0), {
      timeoutMsg: "the page never returned to the top",
    });
    await $(".hd-runhead").waitForExist({ reverse: true });
    await expect($("#record-name")).toBeFocused();
  });
});
