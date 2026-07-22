import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { $$, $, browser, expect } from "@wdio/globals";

/**
 * End-to-end coverage of User Story 4's acceptance scenarios (spec.md),
 * driven against the real built app via tauri-driver / WebKitWebDriver.
 * See us1-record-firearm.e2e.ts for the JS-click rationale.
 */
async function clickEl(selector: string) {
  const el = await $(selector);
  await el.waitForExist();
  await browser.execute((element: HTMLElement) => element.click(), el);
  await browser.pause(200);
}

async function clickInRole(role: "dialog" | "alertdialog", buttonText: string) {
  const container = await $(`[role="${role}"]`);
  const btn = await container.$(`button=${buttonText}`);
  await btn.waitForExist();
  await browser.execute((element: HTMLElement) => element.click(), btn);
  await browser.pause(200);
}

async function setValueBySiblingInput(
  labelSelector: string,
  tag: "input" | "textarea",
  value: string,
) {
  const field = await $(labelSelector).parentElement().$(tag);
  await field.setValue(value);
  await browser.pause(100);
}

// A tiny (20x20, solid blue) but genuinely valid PNG file, written to a
// temp path so the file input can upload real bytes — no mocks. (A
// commonly copy-pasted "1x1 transparent PNG" base64 string was tried here
// first; it turned out to have a corrupted IDAT CRC that browsers/`file`
// tolerate but the `image` crate correctly rejects — generated fresh via
// Python's zlib instead, and round-trip-verified against
// `models::photo::generate_thumbnail` directly.)
const SAMPLE_PNG_BYTES = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAABQAAAAUCAIAAAAC64paAAAAGUlEQVR42mNgaPhPPhrVPKp5VPOo5oHVDADApFaPDOtbFgAAAABJRU5ErkJggg==",
  "base64",
);

describe("User Story 4 - Attach Photos and Documents", () => {
  let pngPath: string;
  let pdfPath: string;

  before(() => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), "hoplodex-e2e-"));
    pngPath = path.join(dir, "range-day.png");
    fs.writeFileSync(pngPath, SAMPLE_PNG_BYTES);
    pdfPath = path.join(dir, "receipt.pdf");
    fs.writeFileSync(pdfPath, "%PDF-1.4 sample receipt contents");
  });

  it("adds a firearm with no photos and shows its generic type thumbnail (Scenario 3)", async () => {
    await clickEl("button=Add firearm");
    await setValueBySiblingInput("label=Make", "input", "InsE2EMediaGlock");
    await setValueBySiblingInput("label=Model", "input", "43");
    await setValueBySiblingInput("label=Caliber", "input", "9mm");
    // Scoped to the open dialog: BrowsePage's own "Group by" combobox
    // stays mounted underneath and would otherwise match first.
    const typeTrigger = await $('[role="dialog"]').$('[role="combobox"]');
    await typeTrigger.waitForExist();
    await browser.execute((element: HTMLElement) => element.click(), typeTrigger);
    await browser.pause(200);
    await clickEl('[role="option"]=Handgun');
    await setValueBySiblingInput("label=Serial number", "input", "MEDIA-1");
    await clickInRole("dialog", "Add firearm");

    await expect($("h3=Photos")).toExist();
    await expect($("p*=No photos yet")).toExist();

    await clickEl("button=← Back to collection");
    await expect($("button*=InsE2EMediaGlock 43")).toExist();
    // The generic-thumbnail <img> loads asynchronously (a separate IPC
    // round trip per row); give it a moment before asserting.
    await browser.pause(500);
    // ValueSummaryPanel's "Unassigned" list also renders a plain
    // <li>make model: $value</li> for this firearm, earlier in the DOM
    // than BrowseList's own row — scope via the row's button (which only
    // BrowseList renders) rather than matching any <li> by text.
    await expect($("button*=InsE2EMediaGlock 43").parentElement().$("img")).toExist();
  });

  it("adds a photo that becomes the thumbnail (Scenario 1)", async () => {
    await clickEl("button*=InsE2EMediaGlock 43");

    const fileInput = await $('input[aria-label="Add photo"]');
    await fileInput.setValue(pngPath);
    await browser.pause(500);

    await expect($("span=Thumbnail")).toExist();
    await expect($("h3=Photos").parentElement().$("img")).toExist();
  });

  it("adds a second photo and lets it be explicitly selected as thumbnail (Scenario 2)", async () => {
    const fileInput = await $('input[aria-label="Add photo"]');
    await fileInput.setValue(pngPath);
    await browser.pause(500);

    // Only one photo (the first) should be marked as the thumbnail so far.
    await expect($("button=Set as thumbnail")).toExist();
    await clickEl("button=Set as thumbnail");

    const thumbnailLabels = await $$("span=Thumbnail");
    expect(thumbnailLabels.length).toBe(1);
  });

  it("attaches a document and can reopen it (Scenario 4)", async () => {
    const fileInput = await $('input[aria-label="Attach document"]');
    await fileInput.setValue(pdfPath);
    await browser.pause(500);

    await expect($("li*=receipt.pdf")).toExist();
    // "Reopen" opens the document via a blob: URL — just confirm the
    // action is available and doesn't raise an error banner.
    await clickEl("button=Reopen");
    await expect($("p*=Failed to reopen")).not.toExist();
  });
});
