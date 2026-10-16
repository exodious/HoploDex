import {
  $,
  $$,
  attachFile,
  clickButton,
  createDatabase,
  expect,
  fill,
  goTo,
  titleBlock,
} from "../support/ui";
import {
  TASK_LIMIT_MS,
  chooseKind,
  shiftTabToControl,
  tabTo,
  tabToControl,
  typeReal,
} from "../support/accessories";
import { realClick, realKey, skipWithoutRealInput } from "../support/realInput";

/**
 * End-to-end coverage of specs/006-accessory-links' User Story 1 (SC-001,
 * US1-1, US1/AC8): on the Accessories page, an Optic is added from the
 * keyboard, scheduled under a policy for less than its value (the
 * under-insured warning shows), given a photo and edited. These go through the
 * real IPC paths the unit tests mock. It needs no firearm, so it starts from a
 * new database. Real keys, as in the other stories of this feature
 * (`us12-accessories-mounting`, `-browse` and `-dispose`).
 */

/** Opens the focused select's list and chooses the option whose text starts
 * with `text` (an option's text can carry a detail line after its name). */
async function chooseOptionStartingWith(text: string) {
  await realKey("Return");
  await $('[role="listbox"]').waitForDisplayed({ timeout: 5000 });
  for (let step = 0; step < 14; step++) {
    const option = await browser.execute(() => {
      const active = document.activeElement as HTMLElement | null;
      return active?.getAttribute("role") === "option" ? (active.textContent ?? "").trim() : null;
    });
    if (option?.startsWith(text)) break;
    await realKey("Down");
  }
  await realKey("Return");
}

/** A local calendar date `days` from today, as YYYY-MM-DD. */
function isoDaysFromNow(days: number): string {
  const d = new Date();
  d.setDate(d.getDate() + days);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

// A tiny but genuinely valid PNG, so the backend's real image decoding and
// thumbnail generation run (as in us4-photos-documents.e2e.ts).
const SAMPLE_PNG_BASE64 =
  "iVBORw0KGgoAAAANSUhEUgAAABQAAAAUCAIAAAAC64paAAAAGUlEQVR42mNgaPhPPhrVPKp5VPOo5oHVDADApFaPDOtbFgAAAABJRU5ErkJggg==";

describe("006 US1 - Record an Accessory (was us12-accessories-record)", () => {
  before(function () {
    skipWithoutRealInput(this);
  });
  before(async () => {
    await createDatabase();
  });

  it("adds an Optic, schedules it under a policy below its value, adds a photo and edits it, from the keyboard (SC-001, US1-1, US1/AC8)", async () => {
    // Setup, not timed: a schedule-only policy, from the Insurance page.
    await goTo("Insurance");
    await clickButton("Add policy");
    await fill("Policy name", "E2E Optic Rider");
    await fill("Policy number", "AC-1");
    await fill("Insurance company", "Acme Insurance");
    await fill("Coverage starts", "2020-01-01");
    await fill("Coverage ends", isoDaysFromNow(365));
    await clickButton("Add policy");
    await $("article.hd-policy*=E2E Optic Rider").waitForExist({ timeout: 8000 });

    const started = Date.now();
    // Real focus on the page, then Shift+Tab back to the Accessories tab.
    await realClick("h1");
    await shiftTabToControl("Accessories");
    await realKey("Return");
    await $("h1=Accessories").waitForExist({ timeout: 8000 });
    await tabToControl("Add accessory");
    await realKey("Return");
    await $('[role="dialog"]').waitForDisplayed({ timeout: 5000 });

    await tabTo("Kind");
    await chooseKind("Optic");
    await tabTo("Make");
    await typeReal("Leupold");
    await tabTo("Model");
    await typeReal("VX3");
    await tabTo("Serial number");
    await typeReal("SC1");
    await tabTo("Estimated value");
    await typeReal("800");
    await realKey("Return");

    // Saving opens the Optic's record.
    await browser.waitUntil(
      async () =>
        (await $("#record-name").isExisting()) &&
        (await $("#record-name").getText()).includes("Leupold VX3"),
      { timeout: 8000, timeoutMsg: "the new Optic's record never opened" },
    );
    expect(await titleBlock("Replacement value")).toBe("$800");

    // Schedule it under the policy for $300, below its $800 value. This is
    // the real assign_accessory_coverage call (T125).
    await realClick("#record-name");
    await tabToControl("Assign");
    await realKey("Return");
    await $('[role="dialog"]').waitForDisplayed({ timeout: 5000 });
    await tabTo("Policy");
    await chooseOptionStartingWith("E2E Optic Rider");
    await tabTo("Scheduled amount");
    await typeReal("300");
    await realKey("Return");

    await browser.waitUntil(async () => (await titleBlock("Coverage")) === "Under-insured", {
      timeout: 8000,
      timeoutMsg: "the Optic never showed as under-insured",
    });
    await expect($(".hd-coverage*=$500 short of its value")).toExist();
    await expect($(".hd-facts--compact*=E2E Optic Rider")).toExist();
    await expect($(".hd-facts--compact*=Scheduled, $300")).toExist();

    // Add a photo. A file picker can't be driven by keys, so this puts the
    // file into the gallery's input as the picker would.
    await attachFile('input[aria-label="Add photos"]', {
      name: "scope.png",
      type: "image/png",
      base64: SAMPLE_PNG_BASE64,
    });
    await $(".hd-photo").waitForExist({ timeout: 8000 });
    await expect($(".hd-photo__tag*=Thumbnail")).toExist();

    // Edit it: a lower value, which the $300 schedule now covers.
    await realClick("#record-name");
    await tabToControl("Edit");
    await realKey("Return");
    await $('[role="dialog"]').waitForDisplayed({ timeout: 5000 });
    await tabTo("Estimated value");
    await realKey("Control_L+a");
    await realKey("BackSpace");
    await typeReal("250");
    await realKey("Return");

    await browser.waitUntil(async () => (await titleBlock("Replacement value")) === "$250", {
      timeout: 8000,
      timeoutMsg: "the edited value never showed",
    });
    expect(await titleBlock("Coverage")).toBe("Covered");
    // The photo and the schedule survive the edit.
    expect(await $$(".hd-photo").length).toBe(1);
    await expect($(".hd-facts--compact*=Scheduled, $300")).toExist();
    expect(Date.now() - started).toBeLessThan(TASK_LIMIT_MS);
  });
});
