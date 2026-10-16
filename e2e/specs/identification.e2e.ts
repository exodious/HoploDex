import {
  $,
  back,
  browser,
  clickButton,
  createDatabase,
  displayName,
  expect,
  fill,
  fillFirearmForm,
  listedNames,
  openFirearm,
  openOriginGroup,
  settle,
  type NewFirearm,
} from "../support/ui";

/** Waits for the record page of a firearm the open Add firearm dialog just
 * saved (what `addFirearm` does after clicking Add firearm). */
async function waitForRecord(firearm: NewFirearm) {
  const expected = displayName(firearm);
  await browser.waitUntil(
    async () =>
      (await $("#record-name").isExisting()) &&
      (await $("#record-name").getText()).replace(/\s+/g, " ") === expected,
    { timeout: 8000, timeoutMsg: `record for ${expected} never opened` },
  );
  await settle();
}

/** Starts recording the body of every `create_firearm` request the page
 * sends, on `window.__createFirearmCalls`. `__TAURI_INTERNALS__.invoke` is
 * read-only, so this wraps `fetch`, which Tauri's IPC custom protocol
 * sends each command through. */
async function recordCreateFirearmCalls() {
  await browser.execute(() => {
    const w = window as unknown as { __createFirearmCalls?: unknown[] };
    const calls: unknown[] = [];
    w.__createFirearmCalls = calls;
    const original = window.fetch;
    window.fetch = async (input, init) => {
      const url = input instanceof Request ? input.url : String(input);
      if (url.endsWith("/create_firearm")) {
        const body = init?.body;
        const text =
          typeof body === "string"
            ? body
            : body instanceof ArrayBuffer || ArrayBuffer.isView(body)
              ? new TextDecoder().decode(body)
              : "";
        calls.push(JSON.parse(text));
      }
      return original.call(window, input, init);
    };
  });
}

async function createFirearmCalls(): Promise<Array<Record<string, unknown>>> {
  return browser.execute(
    () =>
      (window as unknown as { __createFirearmCalls: Array<Record<string, unknown>> })
        .__createFirearmCalls,
  );
}

/** Fills the original maker's marks, offered once the origin is Imported
 * or Re-imported (the origin group is already open by then). */
async function fillOriginalMarks(maker: string, model: string, serial: string) {
  await fill("Original maker", maker);
  await fill("Original model", model);
  await fill("Original serial number", serial);
}

/**
 * End-to-end coverage of 002 SC-007 and US1-7: a first-time user follows the
 * "How do I record this?" guide's re-imported M1 Carbine example, saves with no
 * further prompt, reopens the record, and confirms every value shows labeled.
 *
 * Driven against the real built app through its embedded WebDriver server — no
 * mocks. See e2e/support/ui.ts for why interactions go through page JS.
 */
describe("002 US1 - Record Where a Firearm Came From (was us6-identification)", () => {
  // Each spec's session starts at the chooser with no databases (wdio.conf.ts).
  before(async () => {
    await createDatabase();
  });

  it("records a re-imported M1 Carbine following the guide's example (002 SC-007, US1-7)", async () => {
    // The guide opens from the Add firearm form, and Escape closes only the
    // guide: the form stays open under it, and is the one the record is
    // saved from.
    await clickButton("Add firearm");
    await $('[role="dialog"]').waitForExist();
    await openOriginGroup();
    await clickButton("How do I record this?");
    // Displayed, not just present: getText() reads "" while the dialog is still
    // fading in from opacity 0, which software rendering (no GPU, as in the
    // development container) makes long enough to hit.
    const guide = await $('[role="dialog"]*=How to record where a firearm came from');
    await guide.waitForDisplayed();
    const guideText = (await guide.getText()).replace(/\s+/g, " ");
    expect(guideText).toContain(
      "Record what is stamped on the firearm and what your paperwork says. HoploDex doesn't check it against any rules or decide what is regulated.",
    );
    expect(guideText).toContain("Re-imported M1 Carbine");

    await browser.keys(["Escape"]);
    await $('[role="dialog"]*=How to record where a firearm came from').waitForExist({
      reverse: true,
    });
    await $('[role="dialog"]').waitForExist();

    const firearm = {
      make: "Inland",
      model: "M1 Carbine",
      caliber: ".30 Carbine",
      type: "Rifle" as const,
      serial: "6222533",
      origin: "Re-imported" as const,
      importerName: "Century International Arms",
    };
    await fillFirearmForm(firearm);
    await clickButton("Add firearm");
    await waitForRecord(firearm);

    // No country field was offered for Re-imported, and saving needed no
    // further prompt (no discard/warning dialog appeared) — addFirearm
    // already waited for the record page, so getting here proves it.
    expect((await $(".hd-record").getText()).replace(/\s+/g, " ")).toContain(
      "Century International Arms",
    );

    // Reopen from the collection to confirm persistence, and that every
    // value shows labeled under Identification.
    await back();
    await openFirearm("Inland M1 Carbine");

    const identification = await $('section[aria-labelledby="identification-title"]');
    await identification.waitForExist();
    const text = (await identification.getText()).replace(/\s+/g, " ");
    expect(text).toContain("Re-imported");
    expect(text).toContain("United States");
    expect(text).toContain("Century International Arms");
  });

  // M1 (issue #77 step 8): the only E2E check of FirearmForm's overridable
  // warning, the `confirmedWarnings` resend. 002 US3-5.
  it("warns when another firearm has the same original marks, and saves once confirmed (002 US3-5)", async () => {
    await back();
    const marks = { maker: "Fabrique Nationale", model: "Hi-Power", serial: "74C12345" };

    // The first firearm records the original maker's marks; its own main
    // serial number is the importer's.
    await clickButton("Add firearm");
    await $('[role="dialog"]').waitForExist();
    const first = {
      make: "Browning",
      model: "Hi-Power",
      caliber: "9mm",
      type: "Handgun" as const,
      serial: "T-1001",
      origin: "Imported" as const,
      countryOfManufacture: "Belgium",
      importerName: "Browning Imports",
    };
    await fillFirearmForm(first);
    await fillOriginalMarks(marks.maker, marks.model, marks.serial);
    await clickButton("Add firearm");
    await waitForRecord(first);
    await back();

    // A second one has a different main serial number but the same original
    // marks, so only the warning applies, never the duplicate block.
    await recordCreateFirearmCalls();
    await clickButton("Add firearm");
    await $('[role="dialog"]').waitForExist();
    const second = { ...first, serial: "T-2002", importerName: "Second Importer" };
    await fillFirearmForm(second);
    await fillOriginalMarks(marks.maker, marks.model, marks.serial);
    await clickButton("Add firearm");

    // The warning shows, naming the other firearm. Nothing is saved yet.
    const warning = await $('[role="alertdialog"]*=Another firearm has the same original marks');
    await warning.waitForDisplayed();
    expect((await warning.getText()).replace(/\s+/g, " ")).toContain(
      "Browning Hi-Power (serial T-1001) already has these original maker's marks.",
    );
    expect((await createFirearmCalls()).map((call) => call.confirmedWarnings ?? false)).toEqual([
      false,
    ]);

    // Confirming resends the same input with confirmedWarnings: true.
    await clickButton("Save anyway");
    await waitForRecord(second);
    const calls = await createFirearmCalls();
    expect(calls.map((call) => call.confirmedWarnings ?? false)).toEqual([false, true]);
    expect(calls[1].input).toEqual(calls[0].input);

    // The record is saved with the entered values.
    expect((await $(".hd-record").getText()).replace(/\s+/g, " ")).toContain("Second Importer");
    expect((await $(".hd-record").getText()).replace(/\s+/g, " ")).toContain("T-2002");
    await back();
    expect((await listedNames()).filter((name) => name === "Browning Hi-Power")).toHaveLength(2);
  });
});
