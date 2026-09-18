import { $, addFirearm, back, browser, clickButton, choose, expect, fill } from "../support/ui";
import { listedNames, openFirearm, titleBlock, toggle } from "../support/ui";

/**
 * End-to-end coverage of User Story 1's acceptance scenarios (spec.md),
 * driven against the real built app via tauri-driver / WebKitWebDriver —
 * no mocks, exercising the full stack (React UI -> Tauri IPC -> SQLCipher).
 * See e2e/support/ui.ts for why interactions go through page JS.
 *
 * Creating a firearm opens its record page, so scenarios assert against
 * the record right after creation.
 */
describe("User Story 1 - Record a Firearm", () => {
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
    await fill("Price paid", "1,450.00");
    await clickButton("Save changes");

    await expect($("dd*=Local gun shop")).toExist();
    // Regression: "1,450.00" used to be parsed as $1.00.
    await expect($("dd=$1,450.00")).toExist();
  });

  it("marks a firearm disposed while retaining its history (Scenario 4)", async () => {
    await clickButton("Mark disposed");
    await choose("Sold");
    await fill("Transferred to", "Jane Doe");
    await fill("Date", "2025-06-15");
    await fill("Price received", "400.00");
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
    await browser.pause(300);
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
});
