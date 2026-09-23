import {
  $,
  addFirearm,
  back,
  browser,
  clickButton,
  expect,
  openFirearm,
  openOriginGroup,
} from "../support/ui";

/**
 * End-to-end coverage of specs/002-firearm-identification's User Story 1
 * (SC-007, US1-7): a first-time user follows the "How do I record this?"
 * guide's re-imported M1 Carbine example, saves with no further prompt,
 * reopens the record, and confirms every value shows labeled.
 *
 * Driven against the real built app via tauri-driver / WebKitWebDriver — no
 * mocks. See e2e/support/ui.ts for why interactions go through page JS.
 */
describe("User Story 1 - Identification (specs/002-firearm-identification)", () => {
  it("opens the origin guide from the Add firearm form", async () => {
    await clickButton("Add firearm");
    await $('[role="dialog"]').waitForExist();

    await openOriginGroup();
    await clickButton("How do I record this?");
    await $('[role="dialog"]*=How to record where a firearm came from').waitForExist();
    expect(
      (await $('[role="dialog"]*=How to record where a firearm came from').getText()).replace(
        /\s+/g,
        " ",
      ),
    ).toContain(
      "Record what is stamped on the firearm and what your paperwork says. The app does not check it against any rules.",
    );
    expect(
      (await $('[role="dialog"]*=How to record where a firearm came from').getText()).replace(
        /\s+/g,
        " ",
      ),
    ).toContain("Re-imported M1 Carbine");

    await browser.keys(["Escape"]);
    await $('[role="dialog"]*=How to record where a firearm came from').waitForExist({
      reverse: true,
    });

    // The Add firearm dialog itself is still open underneath.
    await $('[role="dialog"]').waitForExist();
    await clickButton("Cancel");
  });

  it("records a re-imported M1 Carbine following the guide's example (SC-007, US1-7)", async () => {
    await addFirearm({
      make: "Inland",
      model: "M1 Carbine",
      caliber: ".30 Carbine",
      type: "Rifle",
      serial: "6222533",
      origin: "Re-imported",
      importerName: "Century International Arms",
    });

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
});
