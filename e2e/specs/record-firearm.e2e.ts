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
import { relaunchApp } from "../support/app";

/**
 * End-to-end coverage of 001 US1's acceptance scenarios (001 spec.md),
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
describe("001 US1 - Record a Firearm (was us1-record-firearm)", () => {
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

  it("records, edits, disposes and deletes a firearm (001 US1-1 to US1-5)", async () => {
    // 001 US1-1: created, and shown with its values intact.
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

    // 001 US1-2: an edited field persists on reopen.
    await clickButton("Edit");
    await fill("Notes", "scratch on left side");
    await clickButton("Save changes");

    await expect($(".hd-textblock*=scratch on left side")).toExist();

    // Reopen from the collection to confirm persistence beyond the
    // in-memory record state.
    await back();
    await openFirearm("Glock 19");
    await expect($(".hd-textblock*=scratch on left side")).toExist();

    // 001 US1-3: acquisition details are saved and shown.
    await clickButton("Edit");
    await fill("Acquired from", "Local gun shop");
    await fill("Date acquired", "2025-03-01");
    await fill("Price paid", "1450");
    await clickButton("Save changes");

    await expect($("dd*=Local gun shop")).toExist();
    // Entered as plain digits, shown grouped and with no cents (001 FR-037).
    await expect($("dd=$1,450")).toExist();

    // 001 US1-4: disposed, with its history retained.
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

    // 001 FR-025: hidden from the collection by default, shown on request.
    await back();
    expect(await listedNames()).not.toContain("Glock 19");
    await toggle("Show disposed");
    expect(await listedNames()).toContain("Glock 19");
    await toggle("Show disposed");

    // 001 US1-5: deleted after confirmation, and gone from the collection.
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

  it("refuses a duplicate nickname on the Nickname field (001 US1-9)", async () => {
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

  it("blocks a reversal while its nickname is taken, on the New nickname field, until it is renamed (001 US1-12)", async () => {
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

  it("keeps a long record's name and actions in reach as it scrolls (001 US1-18)", async () => {
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

    // 001 FR-041: scrolled down, the strip keeps the way back, the name and the
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

/** What a modal dialog's scroll lock must leave where it was: the top bar, the
 * pinned strip with its actions, and the toasts' corner. */
const PAGE_BARS = [
  ".hd-topbar",
  ".hd-topbar__inner",
  ".hd-record__bar",
  ".hd-runhead",
  ".hd-runhead__inner",
  ".hd-runhead__actions",
  ".hd-toasts",
];

describe("Issue #117 - Nothing on the page moves when a modal opens", () => {
  // The app's web view draws overlay scroll bars by default (they take no
  // room, so there is nothing to lose); GTK_OVERLAY_SCROLLING=0 gives it the
  // classic bars macOS shows with "Show scroll bars: Always" and Windows
  // always shows, where a modal's scroll lock takes the bar away and the page
  // widens. The setting is this block's alone.
  before(async () => {
    process.env.GTK_OVERLAY_SCROLLING = "0";
    await relaunchApp();
    await createDatabase({ name: "Scroll bars" });
    // Enough notes for the record to scroll well past its plate.
    const notes = Array.from({ length: 80 }, (_, i) => `Range note ${i + 1}.`).join("\n");
    await addFirearm({
      make: "E2EBars",
      model: "Long",
      caliber: "9mm",
      type: "Handgun",
      serial: "BAR-1",
      notes,
    });
  });

  after(() => {
    delete process.env.GTK_OVERLAY_SCROLLING;
  });

  const snapshot = () =>
    browser.execute((selectors: string[]) => {
      const rects: Record<string, { left: number; right: number }> = {};
      for (const selector of selectors) {
        const box = document.querySelector(selector)?.getBoundingClientRect();
        if (box) rects[selector] = { left: box.left, right: box.right };
      }
      return { scrollbar: window.innerWidth - document.documentElement.clientWidth, rects };
    }, PAGE_BARS);

  /** Opens something that locks scrolling with `open`, checks that every bar
   * is where it was and that the lock took the scroll bar away, then `close`s it. */
  async function expectBarsStay(open: () => Promise<void>, close: () => Promise<void>) {
    // A toast leaves on its own timer (4 s for a success), so on a slow run
    // one that is there for `before` can be gone by `during` and the corner
    // narrows to its right edge (#145). Start with none: the corner's right
    // edge is what a scroll lock could move.
    await browser.execute(() =>
      document.querySelectorAll<HTMLButtonElement>(".hd-toast__close").forEach((b) => b.click()),
    );
    await $(".hd-toast").waitForExist({ reverse: true });
    await settle();
    const before = await snapshot();
    // On Linux the bar is there to be taken away: without this a run with
    // overlay bars would pass whatever the page did.
    if (process.platform === "linux") expect(before.scrollbar).toBeGreaterThan(0);
    await open();
    await settle();
    const during = await snapshot();
    expect(await browser.execute(() => document.body.hasAttribute("data-scroll-locked"))).toBe(
      true,
    );
    expect(during.scrollbar).toBe(0);
    expect(during.rects).toEqual(before.rects);
    await close();
  }

  const closeModal = async () => {
    await browser.keys(["Escape"]);
    await $('[role="dialog"], [role="alertdialog"], [role="menu"]').waitForExist({
      reverse: true,
    });
    await settle();
  };

  it("keeps the top bar, the pinned strip and the toasts in place under a dialog, a confirmation and a menu (#117)", async () => {
    // The page's top: the top bar and toasts are all there is to move.
    await browser.execute(() => window.scrollTo({ top: 0, behavior: "instant" }));
    await settle();
    await expectBarsStay(() => clickButton("Edit"), closeModal);

    // Scrolled until the strip shows, which is fixed to the window's edges.
    await scrollToPinnedStrip();
    await expectBarsStay(() => clickPinned("Edit"), closeModal);
    await expectBarsStay(() => clickPinned("Delete"), closeModal);
    await expectBarsStay(async () => {
      await browser.execute(() => {
        document
          .querySelector("button.hd-db-menu")!
          .dispatchEvent(
            new PointerEvent("pointerdown", { bubbles: true, button: 0, pointerType: "mouse" }),
          );
      });
      await $('[role="menu"]').waitForExist();
    }, closeModal);
  });
});
