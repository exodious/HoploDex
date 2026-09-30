import fs from "node:fs";
import path from "node:path";
import {
  $,
  addFirearm,
  attachFile,
  back,
  browser,
  clickButton,
  clickEl,
  expect,
} from "../support/ui";
import { openFirearm, rowThumbnail } from "../support/ui";
import { createDatabase } from "../support/ui";

/**
 * End-to-end coverage of User Story 4's acceptance scenarios (spec.md),
 * driven against the real built app via tauri-driver / WebKitWebDriver.
 * See e2e/support/ui.ts for why interactions go through page JS.
 */

// A tiny (20x20, solid blue) but genuinely valid PNG, so the backend's
// real image decoding and thumbnail generation run — no mocks. (A commonly
// copy-pasted "1x1 transparent PNG" base64 string was tried here first; it
// has a corrupted IDAT CRC that browsers tolerate but the `image` crate
// correctly rejects.)
const SAMPLE_PNG_BASE64 =
  "iVBORw0KGgoAAAANSUhEUgAAABQAAAAUCAIAAAAC64paAAAAGUlEQVR42mNgaPhPPhrVPKp5VPOo5oHVDADApFaPDOtbFgAAAABJRU5ErkJggg==";
const SAMPLE_PDF_BASE64 = Buffer.from("%PDF-1.4 sample receipt contents").toString("base64");

// The run's isolated cache/data dirs (see the E2E isolation notes): the
// opened-document copies live under the cache dir. Checks that touch the
// filesystem only run when the run is isolated, so they can never reach a
// real user's data.
const cacheHome = process.env.XDG_CACHE_HOME;
const openedRoot =
  cacheHome && path.join(cacheHome, "io.github.exodious.HoploDex", "opened-documents");

function filesUnder(dir: string): string[] {
  if (!fs.existsSync(dir)) return [];
  return fs
    .readdirSync(dir, { withFileTypes: true })
    .flatMap((entry) =>
      entry.isDirectory() ? filesUnder(path.join(dir, entry.name)) : [path.join(dir, entry.name)],
    );
}

describe("User Story 4 - Attach Photos and Documents", () => {
  // Each spec's session starts at the chooser with no databases (wdio.conf.ts).
  before(async () => {
    await createDatabase();
  });

  it("shows a firearm with no photos with its type's generic drawing (Scenario 3)", async () => {
    await addFirearm({
      make: "InsE2EMediaGlock",
      model: "43",
      caliber: "9mm",
      type: "Handgun",
      serial: "MEDIA-1",
    });

    await expect($(".hd-plate__figure svg.hd-drawing")).toExist();
    await expect($("button*=Drop photos here")).toExist();

    await back();
    expect(await rowThumbnail("InsE2EMediaGlock 43")).toBe("drawing");
  });

  it("adds a photo that becomes the thumbnail (Scenario 1)", async () => {
    await openFirearm("InsE2EMediaGlock 43");
    await attachFile('input[aria-label="Add photos"]', {
      name: "range-day.png",
      type: "image/png",
      base64: SAMPLE_PNG_BASE64,
    });

    await $(".hd-photo").waitForExist();
    await expect($(".hd-photo__tag*=Thumbnail")).toExist();
    // The record's plate now shows the photo instead of the drawing, and it
    // really decoded: a Content-Security-Policy that refused `data:` images
    // would leave the element in place with nothing drawn.
    await expect($(".hd-plate__figure img")).toExist();
    await browser.waitUntil(
      () =>
        browser.execute(() => {
          const img = document.querySelector<HTMLImageElement>(".hd-plate__figure img");
          return Boolean(img?.complete && img.naturalWidth > 0);
        }),
      { timeoutMsg: "the photo on the record never rendered" },
    );

    await back();
    await browser.waitUntil(async () => (await rowThumbnail("InsE2EMediaGlock 43")) === "photo", {
      timeoutMsg: "the collection row never showed the new photo thumbnail",
    });
    await openFirearm("InsE2EMediaGlock 43");
  });

  it("adds a second photo and lets it be explicitly selected as thumbnail (Scenario 2)", async () => {
    await attachFile('input[aria-label="Add photos"]', {
      name: "bench.png",
      type: "image/png",
      base64: SAMPLE_PNG_BASE64,
    });
    await browser.waitUntil(
      async () =>
        (await browser.execute(() => document.querySelectorAll(".hd-photo").length)) === 2,
    );

    // The first photo added stays the thumbnail until another is chosen.
    const tagsBefore = await browser.execute(() =>
      [...document.querySelectorAll(".hd-photo")].map((p) =>
        Boolean(p.querySelector(".hd-photo__tag")),
      ),
    );
    expect(tagsBefore).toEqual([true, false]);

    await clickEl('button[aria-label="View bench.png"]');
    await clickButton("Use as thumbnail");
    await expect($("button=Current thumbnail")).toExist();
    await browser.keys(["Escape"]);
    await browser.pause(300);

    const tagsAfter = await browser.execute(() =>
      [...document.querySelectorAll(".hd-photo")].map((p) =>
        Boolean(p.querySelector(".hd-photo__tag")),
      ),
    );
    expect(tagsAfter).toEqual([false, true]);
  });

  it("attaches a document and can reopen it (Scenario 4)", async () => {
    await attachFile('input[aria-label="Attach documents"]', {
      name: "receipt.pdf",
      type: "application/pdf",
      base64: SAMPLE_PDF_BASE64,
    });

    await expect($(".hd-doc__name=receipt.pdf")).toExist();
    await clickButton("Open");
    await browser.pause(800);
    await expect($(".hd-toast--error")).not.toExist();

    // Reopening hands the OS a temporary copy of the stored bytes. When the
    // run isolates its data under XDG_CACHE_HOME, check that copy directly.
    const cacheHome = process.env.XDG_CACHE_HOME;
    if (cacheHome) {
      const openedRoot = path.join(cacheHome, "io.github.exodious.HoploDex", "opened-documents");
      const copies = fs
        .readdirSync(openedRoot)
        .map((dir) => path.join(openedRoot, dir, "receipt.pdf"))
        .filter((file) => fs.existsSync(file));
      expect(copies.length).toBe(1);
      expect(fs.readFileSync(copies[0], "utf-8")).toBe("%PDF-1.4 sample receipt contents");
    }
  });

  // Scenario 5: no decrypted copy outlives the session (FR-035, SC-010).
  // Ending a WebDriver session kills the app without letting it run its exit
  // handler, which makes it a faithful crash: whatever Scenario 4's "Open"
  // left behind must be swept at the next launch. Clean exit (window closed, or
  // SIGTERM/SIGHUP/SIGINT) can't be observed through WebDriver and is checked
  // against the real binary by e2e/scripts/quit-cleanup.py.
  it("deletes the opened copy at the next launch after an abrupt termination (Scenario 5)", async function () {
    if (!openedRoot) return this.skip();
    expect(filesUnder(openedRoot).length).toBe(1);

    // The relaunch opens no database: the sweep runs at startup, before the
    // chooser (FR-035).
    await browser.reloadSession();

    await browser.waitUntil(async () => filesUnder(openedRoot).length === 0, {
      timeout: 10000,
      timeoutMsg: "a decrypted document copy survived a relaunch",
    });
  });
  it("refuses every request that would leave the device (FR-021, SC-008)", async () => {
    // A restrictive Content-Security-Policy is what stops the webview, and so
    // anything the UI is ever made to run, from sending records anywhere.
    const violations: string[] = await browser.executeAsync((done: (blocked: string[]) => void) => {
      const blocked: string[] = [];
      document.addEventListener("securitypolicyviolation", (e) =>
        blocked.push(e.effectiveDirective),
      );
      const attempts = [
        fetch("https://example.com/collect").catch(() => {}),
        new Promise<void>((resolve) => {
          const img = new Image();
          img.onerror = () => resolve();
          img.onload = () => resolve();
          img.src = "https://example.com/pixel.png";
        }),
      ];
      Promise.all(attempts).then(() => setTimeout(() => done(blocked), 300));
    });

    expect(violations).toContain("connect-src");
    expect(violations).toContain("img-src");
  });
});
