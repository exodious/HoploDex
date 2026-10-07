import { execFileSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import zlib from "node:zlib";
import { relaunchApp } from "../support/app";
import { writeLargePdf } from "../support/largePdf";
import { realClick, realKey, skipWithoutRealInput } from "../support/realInput";
import {
  $,
  attachFile,
  browser,
  choose,
  chooseMenuItem,
  clickEl,
  expect,
  goTo,
  invokeCommand,
  pressEscape,
  scratchDocuments,
  selectOption,
  settle,
  switchDatabase,
  unlock,
  clickButton,
  waitForChooser,
  waitForCollection,
} from "../support/ui";

/**
 * End-to-end coverage of specs/007-document-preview's User Story 1 (spec.md,
 * contracts/ui-document-preview.md) against the real built app, on the
 * human-testing seed's Glock 19 Gen5 and the Leupold mounted on it. The
 * harness seeds the collection for this spec (wdio.conf.ts); the specs of US2
 * (below) and US3 extend this file.
 *
 * What happens inside the PDF surface (the viewer's own controls, every
 * hostile PDF) is the surface check's (research.md §23); this spec drives the
 * main web view and watches what HoploDex itself does: the viewer, the events
 * the backend sends it, what is left on disk, and what is on the screen.
 */

const GLOCK = "Glock 19 Gen5";
const GLOCK_SERIAL = "BXKT482";
const LEUPOLD = "Leupold VX-5HD";
const FIXTURES = fileURLToPath(
  new URL("../../src-tauri/tests/fixtures/documents/", import.meta.url),
);

// The Glock's seeded documents, in list order (examples/human_seed.rs).
const SEEDED = [
  "Purchase receipt.pdf",
  "Appraisal (protected).pdf",
  "Appraisal scan.tif",
  "owners-manual-notes.txt",
  "Round count.csv",
  "Bill of sale.docx",
  "Range log.ods",
  "Old scan.jpg",
];

// --- Helpers ----------------------------------------------------------------

/** Opens a record from the collection list by the start of its name. */
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
    { timeout: 5000, timeoutMsg: `no record "${name}" in the list` },
  );
  await $("#record-name").waitForExist();
  await settle();
}

/** The chooser opens with the last-opened database ("Main collection") already
 * selected and its passphrase field showing; a row's select button exists only
 * for the others. */
async function chooseMainCollection() {
  const select = await $('button.hd-db-row__select[aria-label^="Main collection, "]');
  if (await select.isExisting()) await select.click();
}

/** Leaves a record for the Collection page and opens the Glock again, which
 * reloads its document list. */
async function reopenGlock() {
  await goTo("Collection");
  await openRecord(GLOCK);
}

type LoggedEvent = { event: string; at: number; payload: { previewId: number; reason?: string } };

/** Starts recording the backend's preview events in the page, with the page's
 * clock, the way the frontend's own listeners get them
 * (`plugin:event|listen`, as @tauri-apps/api's `listen` does). Safe to call
 * again: it listens once per page. */
async function recordPreviewEvents() {
  await browser.execute(() => {
    const page = window as unknown as {
      __hdEvents?: unknown[];
      __TAURI_INTERNALS__: {
        transformCallback: (callback: (event: { payload: unknown }) => void) => number;
        invoke: (cmd: string, args: unknown) => Promise<unknown>;
      };
    };
    if (page.__hdEvents) return;
    page.__hdEvents = [];
    for (const event of [
      "preview:pdf-ready",
      "preview:pdf-ended",
      "preview:escape",
      "preview:focus-chrome",
    ]) {
      const handler = page.__TAURI_INTERNALS__.transformCallback((e) =>
        page.__hdEvents!.push({ event, at: performance.now(), payload: e.payload }),
      );
      void page.__TAURI_INTERNALS__.invoke("plugin:event|listen", {
        event,
        target: { kind: "Any" },
        handler,
      });
    }
  });
}

async function loggedEvents(): Promise<LoggedEvent[]> {
  return browser.execute(
    () => (window as unknown as { __hdEvents?: LoggedEvent[] }).__hdEvents ?? [],
  );
}

/** How many events have been recorded so far: a mark to wait past. */
async function eventMark(): Promise<number> {
  return (await loggedEvents()).length;
}

/** Waits for the first `event` recorded after `mark`. */
async function waitForEvent(event: string, mark: number, timeout = 15000): Promise<LoggedEvent> {
  let found: LoggedEvent | undefined;
  await browser.waitUntil(
    async () => {
      found = (await loggedEvents()).slice(mark).find((e) => e.event === event);
      return Boolean(found);
    },
    { timeout, interval: 50, timeoutMsg: `${event} never arrived` },
  );
  return found!;
}

/** Clicks the document's name in the list, which opens the viewer on it (the
 * default setting, "Preview in HoploDex"), and returns the page's clock at the
 * click. */
async function openDocument(name: string): Promise<number> {
  await browser.waitUntil(
    () =>
      browser.execute((wanted: string) => {
        return [...document.querySelectorAll<HTMLElement>("button.hd-doc__name")].some(
          (b) => b.textContent?.trim() === wanted,
        );
      }, name),
    { timeout: 5000, timeoutMsg: `no document "${name}" in the list` },
  );
  const at = await browser.execute((wanted: string) => {
    const button = [...document.querySelectorAll<HTMLElement>("button.hd-doc__name")].find(
      (b) => b.textContent?.trim() === wanted,
    )!;
    const clicked = performance.now();
    button.click();
    return clicked;
  }, name);
  await browser.waitUntil(async () => (await viewerTitle()) === name, {
    timeout: 5000,
    timeoutMsg: `the viewer never opened on "${name}"`,
  });
  return at;
}

/** The title of the open viewer (the 90 vw dialog), or null. */
async function viewerTitle(): Promise<string | null> {
  return browser.execute(
    () =>
      document.querySelector(".hd-dialog__content--xl .hd-dialog__title")?.textContent?.trim() ??
      null,
  );
}

async function closeViewer() {
  await pressEscape();
  await browser.waitUntil(async () => (await viewerTitle()) === null, {
    timeout: 5000,
    timeoutMsg: "the viewer never closed",
  });
}

/** Presses the viewer's "Next document" with a script click, and waits for the
 * title to change. */
async function nextDocument(from: string) {
  await clickEl('.hd-dialog__content--xl button[aria-label="Next document"]');
  await browser.waitUntil(async () => (await viewerTitle()) !== from, {
    timeout: 10000,
    timeoutMsg: `the viewer never left "${from}"`,
  });
}

async function nextIsEnabled(): Promise<boolean> {
  return browser.execute(() => {
    const next = document.querySelector<HTMLButtonElement>(
      '.hd-dialog__content--xl button[aria-label="Next document"]',
    );
    return Boolean(next && !next.disabled);
  });
}

/** The text of every `role="status"` region in the viewer. */
async function viewerStatuses(): Promise<string[]> {
  return browser.execute(() =>
    [...document.querySelectorAll('.hd-dialog__content--xl [role="status"]')].map(
      (el) => el.textContent?.trim() ?? "",
    ),
  );
}

/** An error `invoke` rejects with, as the command's `CommandError`, or null. */
async function invokeFailure(
  cmd: string,
  args: Record<string, unknown>,
): Promise<{ code: string } | null> {
  return browser.executeAsync(
    (name: string, data: Record<string, unknown>, done: (outcome: unknown) => void) => {
      const internals = (
        window as unknown as {
          __TAURI_INTERNALS__: { invoke: (cmd: string, args: unknown) => Promise<unknown> };
        }
      ).__TAURI_INTERNALS__;
      internals.invoke(name, data).then(
        () => done(null),
        (error) => done(error),
      );
    },
    cmd,
    args,
  ) as Promise<{ code: string } | null>;
}

async function glockId(): Promise<number> {
  const listed = (await invokeCommand("list_firearms", { input: { query: GLOCK_SERIAL } })) as {
    groups: { firearms: { id: number }[] }[];
  };
  return listed.groups[0].firearms[0].id;
}

/** Attaches a document through the Documents panel's file input. */
async function attach(name: string, type: string, bytes: Buffer) {
  await attachFile('input[aria-label="Attach documents"]', {
    name,
    type,
    base64: bytes.toString("base64"),
  });
  await browser.waitUntil(
    () =>
      browser.execute((wanted: string) => {
        return [...document.querySelectorAll("button.hd-doc__name")].some(
          (b) => b.textContent?.trim() === wanted,
        );
      }, name),
    { timeout: 10000, timeoutMsg: `${name} never joined the list` },
  );
}

/** A one-page PDF whose page content stream is `content` (uncompressed, so
 * what it draws or says is in the file as it is). */
function pdfWith(content: string, trailer = ""): Buffer {
  const objects = [
    "<< /Type /Catalog /Pages 2 0 R >>",
    "<< /Type /Pages /Count 1 /Kids [3 0 R] >>",
    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R " +
      "/Resources << /Font << /F1 5 0 R >> >> >>",
    `<< /Length ${content.length} >>\nstream\n${content}\nendstream`,
    "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
  ];
  let body = "%PDF-1.4\n";
  const offsets: number[] = [];
  objects.forEach((object, i) => {
    offsets.push(body.length);
    body += `${i + 1} 0 obj\n${object}\nendobj\n`;
  });
  const xref = body.length;
  body += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`;
  for (const offset of offsets) body += `${String(offset).padStart(10, "0")} 00000 n \n`;
  body += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n${trailer}`;
  return Buffer.from(body, "latin1");
}

/** A 64-byte marker that nothing else holds. */
function marker(kind: string): string {
  return `HDMARK-${kind}-${crypto.randomBytes(32).toString("hex")}`.slice(0, 64);
}

// --- Disk and screen --------------------------------------------------------

/** Every folder a leaked copy could land in: the sandbox's XDG folders, TMPDIR
 * and the system temp folder (research.md §23, SC-002). */
function scanRoots(): string[] {
  const roots = [
    process.env.XDG_DATA_HOME,
    process.env.XDG_CACHE_HOME,
    process.env.XDG_CONFIG_HOME,
    process.env.TMPDIR,
    os.tmpdir(),
    "/tmp",
    "/var/tmp",
  ].filter((root): root is string => Boolean(root) && fs.existsSync(root as string));
  return [...new Set(roots.map((root) => fs.realpathSync(root)))];
}

/** The files under the scan roots that hold `needles`. */
function filesHolding(needles: string[]): string[] {
  const found = new Set<string>();
  const wanted = needles.map((needle) => Buffer.from(needle, "latin1"));
  const visit = (entry: string) => {
    let stat: fs.Stats;
    try {
      stat = fs.lstatSync(entry);
    } catch {
      return; // gone since it was listed
    }
    if (stat.isSymbolicLink()) return;
    if (stat.isDirectory()) {
      let names: string[];
      try {
        names = fs.readdirSync(entry);
      } catch {
        return;
      }
      for (const name of names) visit(path.join(entry, name));
    } else if (stat.isFile() && stat.size <= 256 * 1024 * 1024) {
      try {
        const content = fs.readFileSync(entry);
        if (wanted.some((needle) => content.includes(needle))) found.add(entry);
      } catch {
        // unreadable or gone: nothing of ours
      }
    }
  };
  scanRoots().forEach(visit);
  return [...found];
}

interface Screen {
  width: number;
  height: number;
  /** Three bytes a pixel, red, green, blue. */
  pixels: Buffer;
  /** Where the main web view's top left is on the screen (Linux's window
   * sits at 0,0 with no frame; macOS's has a title bar above the page). */
  originX: number;
  originY: number;
}

/** A PNG's pixels, as three bytes each (8 bits a channel, RGB or RGBA, not
 * interlaced: what macOS's `screencapture` writes). */
function decodePng(png: Buffer): { width: number; height: number; pixels: Buffer } {
  let at = 8; // the signature
  let width = 0;
  let height = 0;
  let channels = 0;
  const data: Buffer[] = [];
  while (at < png.length) {
    const length = png.readUInt32BE(at);
    const type = png.toString("latin1", at + 4, at + 8);
    const body = png.subarray(at + 8, at + 8 + length);
    if (type === "IHDR") {
      width = body.readUInt32BE(0);
      height = body.readUInt32BE(4);
      const depth = body[8];
      const colour = body[9];
      if (depth !== 8 || (colour !== 2 && colour !== 6) || body[12] !== 0) {
        throw new Error(
          `unsupported PNG: depth ${depth}, colour type ${colour}, interlace ${body[12]}`,
        );
      }
      channels = colour === 2 ? 3 : 4;
    } else if (type === "IDAT") {
      data.push(body);
    }
    at += 12 + length;
  }
  const raw = zlib.inflateSync(Buffer.concat(data));
  const stride = width * channels;
  const rows = Buffer.alloc(stride * height);
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)];
    const line = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    for (let x = 0; x < stride; x++) {
      const left = x >= channels ? rows[y * stride + x - channels] : 0;
      const up = y > 0 ? rows[(y - 1) * stride + x] : 0;
      const upLeft = y > 0 && x >= channels ? rows[(y - 1) * stride + x - channels] : 0;
      let predictor = 0;
      if (filter === 1) predictor = left;
      else if (filter === 2) predictor = up;
      else if (filter === 3) predictor = (left + up) >> 1;
      else if (filter === 4) {
        const estimate = left + up - upLeft;
        const dLeft = Math.abs(estimate - left);
        const dUp = Math.abs(estimate - up);
        const dUpLeft = Math.abs(estimate - upLeft);
        predictor = dLeft <= dUp && dLeft <= dUpLeft ? left : dUp <= dUpLeft ? up : upLeft;
      }
      rows[y * stride + x] = (line[x] + predictor) & 0xff;
    }
  }
  if (channels === 3) return { width, height, pixels: rows };
  const pixels = Buffer.alloc(width * height * 3);
  for (let i = 0; i < width * height; i++) {
    pixels[i * 3] = rows[i * 4];
    pixels[i * 3 + 1] = rows[i * 4 + 1];
    pixels[i * 3 + 2] = rows[i * 4 + 2];
  }
  return { width, height, pixels };
}

/** The screen as the user sees it, which includes the PDF surface (a
 * WebDriver screenshot holds only the main web view): the whole X screen on
 * Linux, where the window sits at 0,0; on Windows the window's client area
 * (e2e/scripts/window-shot.ps1), so a point in the page is the same point in
 * the picture on both; on macOS the whole screen, with the page's origin
 * found in it. */
async function windowScreenshot(): Promise<Screen> {
  if (process.platform === "darwin") {
    // The window has a title bar above the page, and WebDriver's rectangle
    // doesn't say where the page starts, so a green square in the page's top
    // left corner (clear of the surface) is found in the capture and gives
    // the page's origin.
    const size = 40;
    await browser.execute((px) => {
      const marker = document.createElement("div");
      marker.id = "e2e-origin-marker";
      marker.style.cssText = `position:fixed;left:0;top:0;width:${px}px;height:${px}px;background:#00ff00;z-index:2147483647`;
      document.body.appendChild(marker);
    }, size);
    await browser.pause(300);
    const file = path.join(os.tmpdir(), `hoplodex-e2e-screen-${process.pid}.png`);
    try {
      execFileSync("screencapture", ["-x", "-t", "png", file]);
      const screen = decodePng(fs.readFileSync(file));
      let originX = Infinity;
      let originY = Infinity;
      for (let i = 0; i + 2 < screen.pixels.length; i += 3) {
        if (screen.pixels[i] < 60 && screen.pixels[i + 1] > 200 && screen.pixels[i + 2] < 60) {
          const at = i / 3;
          originX = Math.min(originX, at % screen.width);
          originY = Math.min(originY, Math.floor(at / screen.width));
        }
      }
      if (!Number.isFinite(originX)) throw new Error("the origin marker is not on the screen");
      return { ...screen, originX, originY };
    } finally {
      fs.rmSync(file, { force: true });
      await browser.execute(() => document.getElementById("e2e-origin-marker")?.remove());
    }
  }
  let ppm: Buffer;
  if (process.platform === "win32") {
    const file = path.join(os.tmpdir(), `hoplodex-window-${process.pid}.ppm`);
    try {
      execFileSync(
        "powershell",
        [
          "-NoProfile",
          "-ExecutionPolicy",
          "Bypass",
          "-File",
          path.join(path.dirname(fileURLToPath(import.meta.url)), "../scripts/window-shot.ps1"),
          "-Out",
          file,
          "-Format",
          "ppm",
        ],
        { stdio: "pipe" },
      );
      ppm = fs.readFileSync(file);
    } finally {
      fs.rmSync(file, { force: true });
    }
  } else {
    // 8 bits a channel: ImageMagick's default is 16, which doubles each pixel.
    ppm = execFileSync("import", ["-depth", "8", "-window", "root", "ppm:-"], {
      maxBuffer: 512 * 1024 * 1024,
    });
  }
  // "P6", width, height, 255, one whitespace byte, then the pixels.
  const fields: string[] = [];
  let at = 0;
  while (fields.length < 4) {
    while (/\s/.test(String.fromCharCode(ppm[at]))) at++;
    if (ppm[at] === 0x23) {
      while (ppm[at] !== 0x0a) at++;
      continue;
    }
    let field = "";
    while (!/\s/.test(String.fromCharCode(ppm[at]))) field += String.fromCharCode(ppm[at++]);
    fields.push(field);
  }
  at++;
  return {
    width: Number(fields[1]),
    height: Number(fields[2]),
    pixels: ppm.subarray(at),
    originX: 0,
    originY: 0,
  };
}

/** How many pixels are the page's magenta; with `inside` (in the page's own
 * coordinates), how many are outside that rectangle (grown by `slack` px). */
function magentaPixels(
  screen: Screen,
  inside?: { left: number; top: number; right: number; bottom: number },
  slack = 3,
): number {
  let count = 0;
  for (let i = 0; i + 2 < screen.pixels.length; i += 3) {
    if (screen.pixels[i] > 225 && screen.pixels[i + 1] < 45 && screen.pixels[i + 2] > 225) {
      if (inside) {
        const at = i / 3;
        const x = (at % screen.width) - screen.originX;
        const y = Math.floor(at / screen.width) - screen.originY;
        if (
          x >= inside.left - slack &&
          x <= inside.right + slack &&
          y >= inside.top - slack &&
          y <= inside.bottom + slack
        )
          continue;
      }
      count++;
    }
  }
  return count;
}

/** The rectangle the page's magenta covers, in the page's own coordinates. */
function magentaBounds(screen: Screen): {
  left: number;
  top: number;
  right: number;
  bottom: number;
} {
  const found = { left: Infinity, top: Infinity, right: -Infinity, bottom: -Infinity };
  for (let i = 0; i + 2 < screen.pixels.length; i += 3) {
    if (screen.pixels[i] > 225 && screen.pixels[i + 1] < 45 && screen.pixels[i + 2] > 225) {
      const at = i / 3;
      const x = (at % screen.width) - screen.originX;
      const y = Math.floor(at / screen.width) - screen.originY;
      found.left = Math.min(found.left, x);
      found.right = Math.max(found.right, x);
      found.top = Math.min(found.top, y);
      found.bottom = Math.max(found.bottom, y);
    }
  }
  return found;
}

// --- The spec ---------------------------------------------------------------

describe("User Story 1 (007) - Preview a Document Without Leaving HoploDex", () => {
  before(async () => {
    await chooseMainCollection();
    await unlock(process.env.HOPLODEX_E2E_SEED_PASSPHRASE!);
    await recordPreviewEvents();
    await openRecord(GLOCK);
  });

  it("SC-001: a PDF of about 10 MB is ready within 1 s", async () => {
    const file = writeLargePdf(path.join(scratchDocuments(), "large"));
    expect(fs.statSync(file).size).toBeGreaterThan(9.5 * 1024 * 1024);
    const owner = { kind: "firearm", id: await glockId() };
    await invokeCommand("add_document_from_path", { owner, path: file });
    await reopenGlock();

    const mark = await eventMark();
    const clickedAt = await openDocument("Large appraisal.pdf");
    const ready = await waitForEvent("preview:pdf-ready", mark);

    console.log(`SC-001: click to preview:pdf-ready ${Math.round(ready.at - clickedAt)} ms`);
    expect(ready.at - clickedAt).toBeLessThan(1000);
    await closeViewer();
  });

  it("opens a PDF, F6 into it and back, to the TIFF, PageDown and +, to the text, Escape (keyboard only)", async function () {
    this.timeout(120000);
    skipWithoutRealInput(this);
    const opener = 'button.hd-doc__name[title="Preview Purchase receipt.pdf"]';

    let mark = await eventMark();
    await realClick(opener);
    await waitForEvent("preview:pdf-ready", mark);
    expect(await viewerTitle()).toBe("Purchase receipt.pdf");

    // F6 puts the focus in the surface, so the main web view loses it; F6 in
    // the surface brings it back to the viewer's first control.
    await realKey("F6");
    await browser.waitUntil(() => browser.execute(() => !document.hasFocus()), {
      timeout: 5000,
      timeoutMsg: "F6 never moved the focus into the surface",
    });
    mark = await eventMark();
    await realKey("F6");
    await waitForEvent("preview:focus-chrome", mark);
    expect(
      await browser.execute(
        () => document.activeElement?.closest(".hd-dialog__content--xl") !== null,
      ),
    ).toBe(true);

    // → to the protected PDF, which the surface keeps, and → to the TIFF,
    // which closes the surface.
    mark = await eventMark();
    await realKey("Right");
    await browser.waitUntil(async () => (await viewerTitle()) === "Appraisal (protected).pdf");
    const protectedReady = await waitForEvent("preview:pdf-ready", mark);
    await realKey("Right");
    await browser.waitUntil(async () => (await viewerTitle()) === "Appraisal scan.tif");
    expect(
      await invokeFailure("focus_preview", { previewId: protectedReady.payload.previewId }),
    ).toMatchObject({ code: "PREVIEW_CLOSED" });

    await browser.waitUntil(
      () =>
        browser.execute(() => {
          const img = document.querySelector<HTMLImageElement>(
            'img[alt="Appraisal scan.tif, page 1 of 4"]',
          );
          return Boolean(img?.complete && img.naturalWidth > 0);
        }),
      { timeout: 10000, timeoutMsg: "page 1 of the TIFF never rendered" },
    );
    const percent = () =>
      browser.execute(
        () =>
          [...document.querySelectorAll(".hd-dialog__content--xl button")]
            .map((b) => b.textContent?.trim() ?? "")
            .find((text) => /^\d+%$/.test(text)) ?? "",
      );
    const pageNow = () =>
      browser.execute(
        () =>
          [...document.querySelectorAll(".hd-dialog__content--xl *")]
            .map((el) => (el.children.length === 0 ? (el.textContent?.trim() ?? "") : ""))
            .find((text) => /^Page \d+ of \d+$/.test(text)) ?? "",
      );
    const before = await percent();
    await realClick('[aria-label="Appraisal scan.tif, pages"]');
    await realKey("Next"); // Page Down
    await browser.waitUntil(async () => (await pageNow()) === "Page 2 of 4", {
      timeout: 5000,
      timeoutMsg: "Page Down never moved to page 2",
    });
    await realKey("KP_Add");
    await browser.waitUntil(async () => (await percent()) !== before, {
      timeout: 5000,
      timeoutMsg: "+ never changed the zoom",
    });

    // Tab leaves the page area for the footer, whose arrows move on to the text.
    await realKey("Tab");
    await realKey("Right");
    await browser.waitUntil(async () => (await viewerTitle()) === "owners-manual-notes.txt");
    await $(".hd-dialog__content--xl pre.hd-preview__text").waitForExist();

    await realKey("Escape");
    await browser.waitUntil(async () => (await viewerTitle()) === null, { timeout: 5000 });
    expect(
      await browser.execute(
        (selector: string) => document.activeElement?.matches(selector) ?? false,
        opener,
      ),
    ).toBe(true);
  });

  it("moves through the Glock's own documents and never into the mounted Leupold's (FR-007)", async function () {
    this.timeout(180000);
    await settle();
    const listed = await browser.execute(() =>
      [...document.querySelectorAll("button.hd-doc__name")].map((b) => b.textContent?.trim()),
    );
    expect(listed).toEqual(expect.arrayContaining(SEEDED));

    await openDocument(listed[0]!);
    const visited: string[] = [];
    for (;;) {
      const title = (await viewerTitle())!;
      visited.push(title);
      if (!(await nextIsEnabled())) break;
      await nextDocument(title);
    }
    await closeViewer();

    expect(visited).toEqual(listed);
    expect(visited).not.toContain("receipt-leupold.pdf");
  });

  it("previews the Leupold's own receipt on its record", async () => {
    await goTo("Accessories");
    await $("h1=Accessories").waitForExist();
    await browser.waitUntil(
      () =>
        browser.execute((wanted: string) => {
          const button = [...document.querySelectorAll<HTMLElement>(".hd-row__name")].find((b) =>
            b.textContent?.trim().startsWith(wanted),
          );
          button?.click();
          return Boolean(button);
        }, LEUPOLD),
      { timeout: 5000, timeoutMsg: `no accessory "${LEUPOLD}" in the list` },
    );
    await $("#record-name").waitForExist();
    await settle();

    const mark = await eventMark();
    await openDocument("receipt-leupold.pdf");
    await waitForEvent("preview:pdf-ready", mark);
    await closeViewer();
    await goTo("Collection");
    await openRecord(GLOCK);
  });

  it("SC-002: leaves no copy of a PDF, TIFF or text document on disk, during or after a preview", async function () {
    this.timeout(180000);
    const pdfMarker = marker("PDF");
    const tiffMarker = marker("TIFF");
    const textMarker = marker("TXT");
    // The marker sits where nothing reads it: after a PDF's %%EOF and a TIFF's
    // last byte, so each is still a document, and a copy would hold it.
    await attach(
      "Marker.pdf",
      "application/pdf",
      pdfWith("BT /F1 18 Tf 72 700 Td (Marker) Tj ET", `%${pdfMarker}\n`),
    );
    await attach(
      "Marker.tif",
      "image/tiff",
      Buffer.concat([
        fs.readFileSync(path.join(FIXTURES, "one-page.tif")),
        Buffer.from(tiffMarker),
      ]),
    );
    await attach("Marker.txt", "text/plain", Buffer.from(`${textMarker}\n`));
    const markers = [pdfMarker, tiffMarker, textMarker];
    expect(filesHolding(markers)).toEqual([]); // nothing before the first preview

    const mark = await eventMark();
    await openDocument("Marker.pdf");
    await waitForEvent("preview:pdf-ready", mark);
    expect(filesHolding(markers)).toEqual([]);
    await closeViewer();
    expect(filesHolding(markers)).toEqual([]);

    await openDocument("Marker.tif");
    await $('.hd-dialog__content--xl img[alt="Marker.tif"]').waitForExist({ timeout: 10000 });
    expect(filesHolding(markers)).toEqual([]);
    await closeViewer();
    expect(filesHolding(markers)).toEqual([]);

    await openDocument("Marker.txt");
    await $(".hd-dialog__content--xl pre.hd-preview__text").waitForExist();
    expect(filesHolding(markers)).toEqual([]);
    await closeViewer();
    expect(filesHolding(markers)).toEqual([]);

    // The scan can see a marker at all: a file that holds one is found.
    const control = path.join(scratchDocuments(), "control.txt");
    fs.writeFileSync(control, pdfMarker);
    expect(filesHolding([pdfMarker])).toEqual([fs.realpathSync(control)]);
    fs.rmSync(control);
  });

  it("FR-006, SC-003: a truncated and a bit-flipped PDF reach the surface, and the app stays up", async function () {
    this.timeout(120000);
    for (const name of ["three-pages-truncated.pdf", "three-pages-bitflipped.pdf"]) {
      await attach(name, "application/pdf", fs.readFileSync(path.join(FIXTURES, name)));
      const mark = await eventMark();
      await openDocument(name);
      await waitForEvent("preview:pdf-ready", mark);
      // The viewer reports it in its own words; HoploDex says nothing of its own.
      expect(await viewerStatuses()).not.toContainEqual(
        expect.stringMatching(/couldn't be previewed/),
      );
      await closeViewer();
    }

    // Still up, and still previewing.
    await openDocument("owners-manual-notes.txt");
    await $(".hd-dialog__content--xl pre.hd-preview__text").waitForExist();
    await closeViewer();
  });

  // The footer's "Open in another app…" is User Story 2's (T093); this check is
  // its (T086).
  it("FR-006: 'Open in another app…' stays in the footer beside a damaged PDF", async () => {
    const mark = await eventMark();
    await openDocument("three-pages-truncated.pdf");
    await waitForEvent("preview:pdf-ready", mark);
    const enabled = await browser.execute(() => {
      const button = [
        ...document.querySelectorAll<HTMLButtonElement>(".hd-dialog__content--xl footer button"),
      ].find((b) => b.textContent?.trim() === "Open in another app…");
      return Boolean(button && !button.disabled);
    });
    expect(enabled).toBe(true);
    await closeViewer();
  });

  it("the PDF surface lies exactly over the viewer's page area, and not over the footer", async function () {
    this.timeout(120000);
    // A page that is all magenta, which nothing else in the app is, so that
    // wherever the surface is on the screen shows.
    await attach("Magenta.pdf", "application/pdf", pdfWith("1 0 1 rg 0 0 612 792 re f"));
    const mark = await eventMark();
    await openDocument("Magenta.pdf");
    await waitForEvent("preview:pdf-ready", mark);
    // A toast hides the surface, which is a web view above the page (the
    // attach above left one): wait for them to go.
    await browser.waitUntil(async () => !(await $(".hd-toast").isExisting()), {
      timeout: 15000,
      timeoutMsg: "the toasts never went",
    });
    await browser.pause(1500); // the dialog's entrance, and the viewer's first paint
    const area = await browser.execute(() => {
      const r = document.querySelector(".hd-preview__surface")!.getBoundingClientRect();
      return { left: r.left, top: r.top, right: r.right, bottom: r.bottom };
    });
    const screen = await windowScreenshot();
    expect(magentaPixels(screen)).toBeGreaterThan(5000);
    // Linux GTK allocated the surface its natural size, not the size asked
    // for, so it ran to the window's edges over the footer.
    expect(magentaPixels(screen, area)).toBeLessThan(100);
    if (process.platform === "darwin") {
      // WebKit's viewer shows the page at its own size, centred, from just
      // under the top and cut off at the bottom (it is taller than the
      // area): so it must touch both ends and be centred, rather than sit
      // offset inside the area.
      const page = magentaBounds(screen);
      expect(page.top - area.top).toBeGreaterThanOrEqual(0);
      expect(page.top - area.top).toBeLessThan(40);
      expect(area.bottom - page.bottom).toBeGreaterThanOrEqual(0);
      expect(area.bottom - page.bottom).toBeLessThan(10);
      expect(Math.abs((page.left + page.right) / 2 - (area.left + area.right) / 2)).toBeLessThan(
        20,
      );
    } else {
      // And fills it (the page, less the viewer's toolbar and gutters, is most
      // of the area), rather than sitting offset inside it.
      const areaPixels = (area.right - area.left) * (area.bottom - area.top);
      expect(magentaPixels(screen) - magentaPixels(screen, area)).toBeGreaterThan(0.7 * areaPixels);
    }
    await closeViewer();
  });

  it("a lock with a PDF shown leaves nothing of it on the screen once the chooser is up", async function () {
    this.timeout(120000);
    const mark = await eventMark();
    await openDocument("Magenta.pdf");
    await waitForEvent("preview:pdf-ready", mark);
    await browser.waitUntil(async () => !(await $(".hd-toast").isExisting()), {
      timeout: 15000,
      timeoutMsg: "the toasts never went",
    });
    await browser.pause(1000); // the viewer's first paint
    expect(magentaPixels(await windowScreenshot())).toBeGreaterThan(5000);

    // The viewer's own lock button, since the dialog covers the top bar's.
    await clickEl('.hd-dialog__content--xl button[aria-label="Lock now"]');
    await waitForChooser();
    await browser.pause(300);

    expect(await $('[role="dialog"]').isExisting()).toBe(false);
    expect(magentaPixels(await windowScreenshot())).toBeLessThan(100);

    await unlock(process.env.HOPLODEX_E2E_SEED_PASSPHRASE!);
    await openRecord(GLOCK);
  });

  it("the idle lock fires with a PDF shown and no input", async function () {
    this.timeout(180000);
    await switchDatabase();
    // The idle lock's minute lasts 3 s in this app (wdio.conf.ts does it for
    // US9's spec; this one asks for it itself).
    process.env.HOPLODEX_E2E_IDLE_MINUTE_SECONDS = "3";
    await relaunchApp();
    await waitForChooser();
    await chooseMainCollection();
    await unlock(process.env.HOPLODEX_E2E_SEED_PASSPHRASE!);
    await recordPreviewEvents();

    await chooseMenuItem("button.hd-db-menu", "Database settings…");
    await $('[role="dialog"]').waitForExist();
    await selectOption("After", "1 minute");
    await clickButton("Save");
    await browser.waitUntil(async () => !(await $('[role="dialog"]').isExisting()), {
      timeout: 5000,
      timeoutMsg: "the settings never closed",
    });

    await openRecord(GLOCK);
    const mark = await eventMark();
    await openDocument("Purchase receipt.pdf");
    await waitForEvent("preview:pdf-ready", mark);
    expect(await viewerTitle()).toBe("Purchase receipt.pdf");

    // No input from here: the clock runs out while the PDF is on the screen.
    await browser.waitUntil(async () => $(".hd-chooser__title").isExisting(), {
      timeout: 30000,
      interval: 500,
      timeoutMsg: "the idle lock never locked with a PDF shown",
    });
    expect(await $('[role="dialog"]').isExisting()).toBe(false);
    delete process.env.HOPLODEX_E2E_IDLE_MINUTE_SECONDS;
  });

  it("with PDF preview off, PDFs say they can't be previewed on this computer, and the TIFF and text still preview", async function () {
    this.timeout(180000);
    delete process.env.HOPLODEX_E2E_IDLE_MINUTE_SECONDS;
    process.env.HOPLODEX_E2E_PDF_PREVIEW = "off";
    await relaunchApp();
    await waitForChooser();
    await chooseMainCollection();
    await unlock(process.env.HOPLODEX_E2E_SEED_PASSPHRASE!);
    await waitForCollection();
    await openRecord(GLOCK);

    const meta = await browser.execute(
      () =>
        [...document.querySelectorAll("li.hd-doc")].find(
          (li) =>
            li.querySelector("button.hd-doc__name")?.textContent?.trim() === "Purchase receipt.pdf",
        )?.textContent ?? "",
    );
    expect(meta).toContain("can't be previewed on this computer");

    await openDocument("Purchase receipt.pdf");
    await browser.waitUntil(
      async () =>
        (await viewerStatuses()).some((text) =>
          text.startsWith("PDFs can't be previewed on this computer."),
        ),
      { timeout: 10000, timeoutMsg: "the viewer never said PDFs can't be previewed" },
    );
    await closeViewer();

    await openDocument("Appraisal scan.tif");
    await $('.hd-dialog__content--xl img[alt="Appraisal scan.tif, page 1 of 4"]').waitForExist({
      timeout: 10000,
    });
    await closeViewer();

    await openDocument("owners-manual-notes.txt");
    await $(".hd-dialog__content--xl pre.hd-preview__text").waitForExist();
    await closeViewer();
    delete process.env.HOPLODEX_E2E_PDF_PREVIEW;
  });
});

// --- User Story 2 (007) ------------------------------------------------------

/** Where the app puts the copies it hands to another app (documents.rs,
 * OPENED_DOCUMENTS_DIR), in the run's isolated cache folder. */
const openedRoot = process.env.HOPLODEX_E2E_CACHE_HOME
  ? path.join(
      process.env.HOPLODEX_E2E_CACHE_HOME,
      "io.github.exodious.HoploDex",
      "opened-documents",
    )
  : undefined;

function filesUnder(dir: string): string[] {
  if (!fs.existsSync(dir)) return [];
  return fs
    .readdirSync(dir, { withFileTypes: true })
    .flatMap((entry) =>
      entry.isDirectory() ? filesUnder(path.join(dir, entry.name)) : [path.join(dir, entry.name)],
    );
}

/** The lines of a seam's log file (HOPLODEX_E2E_CONSENT_LOG: the title of each
 * native confirmation the E2E build would have shown; HOPLODEX_E2E_OPENED_LOG:
 * the path of each copy it would have handed to another app). None if the
 * file isn't there yet. */
function logLines(variable: "HOPLODEX_E2E_CONSENT_LOG" | "HOPLODEX_E2E_OPENED_LOG"): string[] {
  const file = process.env[variable];
  if (!file) throw new Error(`${variable} is not set (wdio.conf.ts, T092)`);
  return fs.existsSync(file)
    ? fs
        .readFileSync(file, "utf-8")
        .split(/\r?\n/)
        .filter((line) => line.trim() !== "")
    : [];
}

/** Starts the app again with the consent seam answering `answer`: the E2E
 * build reads HOPLODEX_E2E_CONSENT (`open` or `cancel`) when it starts, from
 * the environment `relaunchApp` passes on (wdio.conf.ts sets the default,
 * `open`, which a spec may override before a relaunch). Back at the Glock's
 * record with the collection open. */
async function relaunchAnswering(answer: "open" | "cancel") {
  process.env.HOPLODEX_E2E_CONSENT = answer;
  await relaunchApp();
  await waitForChooser();
  await chooseMainCollection();
  await unlock(process.env.HOPLODEX_E2E_SEED_PASSPHRASE!);
  await waitForCollection();
  await openRecord(GLOCK);
}

/** Presses a button of a document's row in the list by its text. */
async function clickRowButton(documentName: string, label: string) {
  const clicked = await browser.execute(
    (wanted: string, text: string) => {
      const row = [...document.querySelectorAll<HTMLElement>("li.hd-doc")].find(
        (li) => li.querySelector("button.hd-doc__name")?.textContent?.trim() === wanted,
      );
      const button = [...(row?.querySelectorAll<HTMLButtonElement>("button") ?? [])].find(
        (b) => b.textContent?.trim() === text,
      );
      button?.click();
      return Boolean(button && !button.disabled);
    },
    documentName,
    label,
  );
  expect(clicked).toBe(true);
}

/** Every toast's text. */
async function toastTexts(): Promise<string[]> {
  return browser.execute(() =>
    [...document.querySelectorAll(".hd-toast")].map((el) => el.textContent?.trim() ?? ""),
  );
}

async function documentNames(): Promise<string[]> {
  return browser.execute(() =>
    [...document.querySelectorAll("button.hd-doc__name")].map((b) => b.textContent?.trim() ?? ""),
  );
}

describe("User Story 2 (007) - Open a Document in Another App, After Asking", () => {
  before(async () => {
    await relaunchAnswering("cancel");
  });

  after(() => {
    // The default for any spec after this one.
    process.env.HOPLODEX_E2E_CONSENT = "open";
  });

  it("with the confirmation cancelled, writes nothing and starts nothing, and says nothing", async function () {
    if (!openedRoot) return this.skip();
    const asked = logLines("HOPLODEX_E2E_CONSENT_LOG").length;
    const opened = logLines("HOPLODEX_E2E_OPENED_LOG").length;

    await clickRowButton("Purchase receipt.pdf", "Open in another app…");

    // The seam logged the confirmation it answered, with the document's name.
    await browser.waitUntil(() => logLines("HOPLODEX_E2E_CONSENT_LOG").length > asked, {
      timeout: 10000,
      timeoutMsg: "the consent seam never logged the confirmation",
    });
    expect(logLines("HOPLODEX_E2E_CONSENT_LOG").slice(asked).join("\n")).toContain(
      "Purchase receipt.pdf",
    );
    await settle();

    expect(filesUnder(openedRoot)).toEqual([]);
    expect(logLines("HOPLODEX_E2E_OPENED_LOG")).toHaveLength(opened);
    // A cancel is the user's own answer: no toast, success or error.
    expect(await toastTexts()).not.toContainEqual(expect.stringContaining("Purchase receipt.pdf"));
    await expect($(".hd-toast--error")).not.toExist();
  });

  it("with the confirmation accepted, writes the copy and names it in the opened log", async function () {
    if (!openedRoot) return this.skip();
    await relaunchAnswering("open");
    const asked = logLines("HOPLODEX_E2E_CONSENT_LOG").length;
    const opened = logLines("HOPLODEX_E2E_OPENED_LOG").length;

    await clickRowButton("Purchase receipt.pdf", "Open in another app…");

    await browser.waitUntil(() => logLines("HOPLODEX_E2E_OPENED_LOG").length > opened, {
      timeout: 10000,
      timeoutMsg: "the opener never logged the copy",
    });
    expect(logLines("HOPLODEX_E2E_CONSENT_LOG").length).toBe(asked + 1);
    const copies = filesUnder(openedRoot).filter(
      (file) => path.basename(file) === "Purchase receipt.pdf",
    );
    expect(copies).toHaveLength(1);
    expect(logLines("HOPLODEX_E2E_OPENED_LOG").at(-1)).toBe(copies[0]);
    expect(fs.readFileSync(copies[0]).subarray(0, 5).toString("latin1")).toBe("%PDF-");

    await browser.waitUntil(
      async () => (await toastTexts()).includes("Opened Purchase receipt.pdf in another app."),
      { timeout: 5000, timeoutMsg: "no 'Opened … in another app.' toast" },
    );
  });

  it("opens the viewer on 'can't be previewed here' for a Word document, with Open in another app…", async () => {
    await openDocument("Bill of sale.docx");

    await browser.waitUntil(
      async () =>
        (await viewerStatuses()).includes(
          "Bill of sale.docx can't be previewed here. Word documents open in another app.",
        ),
      { timeout: 10000, timeoutMsg: "the viewer never said the Word document can't be previewed" },
    );
    const offered = await browser.execute(() =>
      [...document.querySelectorAll<HTMLButtonElement>(".hd-dialog__content--xl button")]
        .filter((b) => b.textContent?.trim() === "Open in another app…")
        .map((b) => !b.disabled),
    );
    expect(offered.length).toBeGreaterThan(0);
    expect(offered.every(Boolean)).toBe(true);
    await closeViewer();
  });

  it("disables Open in another app… for a pre-007 row that isn't a document type", async () => {
    const state = await browser.execute(() => {
      const row = [...document.querySelectorAll<HTMLElement>("li.hd-doc")].find(
        (li) => li.querySelector("button.hd-doc__name")?.textContent?.trim() === "Old scan.jpg",
      );
      const button = [...(row?.querySelectorAll<HTMLButtonElement>("button") ?? [])].find(
        (b) => b.textContent?.trim() === "Open in another app…",
      );
      return button ? { disabled: button.disabled, title: button.title } : null;
    });

    expect(state).toMatchObject({ disabled: true });
    expect(state!.title).not.toBe("");
  });

  it("refuses a dropped .exe before any command, and attaches nothing", async () => {
    const before = await documentNames();

    // A native drop reaches the page as Tauri's `tauri://drag-drop` event with the
    // paths (WebDriver can't drag from the desktop). The file needn't exist: the
    // refusal comes before anything reads it.
    await browser.execute(() => {
      const internals = (
        window as unknown as {
          __TAURI_INTERNALS__: { invoke: (cmd: string, args: unknown) => Promise<unknown> };
        }
      ).__TAURI_INTERNALS__;
      void internals.invoke("plugin:event|emit", {
        event: "tauri://drag-drop",
        payload: { paths: ["/tmp/hoplodex-e2e-setup.exe"], position: { x: 10, y: 10 } },
      });
    });

    await browser.waitUntil(
      async () =>
        (await toastTexts()).includes(
          "hoplodex-e2e-setup.exe wasn't attached. Documents can be PDF, TIFF, text, CSV, RTF, Word, spreadsheet or OpenDocument files.",
        ),
      { timeout: 5000, timeoutMsg: "the dropped .exe was never refused with the toast" },
    );
    await settle();
    expect(await documentNames()).toEqual(before);
  });
});

// --- User Story 3 (007) ------------------------------------------------------

/** Opens Database settings, and closes them again with Cancel. */
async function openDatabaseSettings() {
  await chooseMenuItem("button.hd-db-menu", "Database settings…");
  await $('[role="dialog"]').waitForExist();
  await settle();
}

async function closeDatabaseSettings() {
  await clickButton("Cancel");
  await $('[role="dialog"]').waitForExist({ reverse: true });
}

/** Whether the "Open documents" radio labelled `label` is the checked one. */
async function openingChecked(label: string): Promise<boolean> {
  return browser.execute((wanted: string) => {
    const input = [
      ...document.querySelectorAll<HTMLInputElement>(
        '[role="dialog"] [role="radiogroup"] input[type="radio"]',
      ),
    ].find((r) => r.closest("label")?.textContent?.trim() === wanted);
    return Boolean(input?.checked);
  }, label);
}

/** Chooses `label` in the Documents fieldset of Database settings and waits
 * until it is the checked one. The setting is saved as soon as it is chosen,
 * so the dialog is closed again at once. */
async function setOpening(label: "Preview in HoploDex" | "Open in another app") {
  await openDatabaseSettings();
  await choose(label);
  await browser.waitUntil(() => openingChecked(label), {
    timeout: 10000,
    timeoutMsg: `"${label}" never became the setting`,
  });
  await closeDatabaseSettings();
}

/** Clicks a document's name in the list, whatever the setting makes it do. */
async function clickDocumentName(name: string) {
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
}

/** The `title` of a document's name button. */
async function nameTitle(name: string): Promise<string | null> {
  return browser.execute(
    (wanted: string) =>
      [...document.querySelectorAll<HTMLElement>("button.hd-doc__name")]
        .find((b) => b.textContent?.trim() === wanted)
        ?.getAttribute("title") ?? null,
    name,
  );
}

describe("User Story 3 (007) - Choose How Documents Open", () => {
  const SETTING_TITLE = "Open documents in another app?";

  before(async () => {
    await relaunchAnswering("open");
  });

  after(async () => {
    // The setting is this computer's, so a spec that failed half way leaves
    // it where it was put: put it back (changing to "preview" never asks).
    await invokeCommand("set_document_opening", { value: "preview" });
    process.env.HOPLODEX_E2E_CONSENT = "open";
  });

  it("US3-1: starts at Preview in HoploDex, and the name previews", async () => {
    await openDatabaseSettings();
    expect(await openingChecked("Preview in HoploDex")).toBe(true);
    expect(await openingChecked("Open in another app")).toBe(false);
    await closeDatabaseSettings();

    expect(await nameTitle("owners-manual-notes.txt")).toBe("Preview owners-manual-notes.txt");
  });

  it("US3-2: changing to Open in another app asks, in the consent log, and takes effect", async function () {
    const asked = logLines("HOPLODEX_E2E_CONSENT_LOG").length;

    await setOpening("Open in another app");

    const lines = logLines("HOPLODEX_E2E_CONSENT_LOG");
    expect(lines).toHaveLength(asked + 1);
    expect(lines.at(-1)).toContain(SETTING_TITLE);
    expect(await nameTitle("Bill of sale.docx")).toBe("Open Bill of sale.docx in another app");
    // The setting is saved at once, without the dialog's Save.
    await openDatabaseSettings();
    expect(await openingChecked("Open in another app")).toBe(true);
    await closeDatabaseSettings();
  });

  it("US3-3: opening two documents by name asks once, and opens both", async function () {
    if (!openedRoot) return this.skip();
    const asked = logLines("HOPLODEX_E2E_CONSENT_LOG").length;
    const opened = logLines("HOPLODEX_E2E_OPENED_LOG").length;

    await clickDocumentName("Purchase receipt.pdf");
    await browser.waitUntil(() => logLines("HOPLODEX_E2E_OPENED_LOG").length > opened, {
      timeout: 10000,
      timeoutMsg: "the first document was never opened in another app",
    });
    await clickDocumentName("Bill of sale.docx");
    await browser.waitUntil(() => logLines("HOPLODEX_E2E_OPENED_LOG").length > opened + 1, {
      timeout: 10000,
      timeoutMsg: "the second document was never opened in another app",
    });

    expect(logLines("HOPLODEX_E2E_OPENED_LOG")).toHaveLength(opened + 2);
    const consent = logLines("HOPLODEX_E2E_CONSENT_LOG");
    expect(consent).toHaveLength(asked + 1);
    expect(consent.at(-1)).toContain("Purchase receipt.pdf");
    // The name opens in another app: no viewer.
    expect(await viewerTitle()).toBeNull();
  });

  it("US3-4: after a lock and an unlock, the next open asks again", async function () {
    if (!openedRoot) return this.skip();
    await switchDatabase();
    await chooseMainCollection();
    await unlock(process.env.HOPLODEX_E2E_SEED_PASSPHRASE!);
    await openRecord(GLOCK);
    const asked = logLines("HOPLODEX_E2E_CONSENT_LOG").length;
    const opened = logLines("HOPLODEX_E2E_OPENED_LOG").length;
    // The setting is the computer's: it survived the lock.
    expect(await nameTitle("Bill of sale.docx")).toBe("Open Bill of sale.docx in another app");

    await clickDocumentName("Bill of sale.docx");

    await browser.waitUntil(() => logLines("HOPLODEX_E2E_OPENED_LOG").length > opened, {
      timeout: 10000,
      timeoutMsg: "the document was never opened in another app after the unlock",
    });
    const consent = logLines("HOPLODEX_E2E_CONSENT_LOG");
    expect(consent).toHaveLength(asked + 1);
    expect(consent.at(-1)).toContain("Bill of sale.docx");
  });

  it("US3-5: Preview on the PDF previews it, and asks nothing", async function () {
    const asked = logLines("HOPLODEX_E2E_CONSENT_LOG").length;
    const opened = logLines("HOPLODEX_E2E_OPENED_LOG").length;

    await recordPreviewEvents();
    const mark = await eventMark();
    await clickRowButton("Purchase receipt.pdf", "Preview");

    await browser.waitUntil(async () => (await viewerTitle()) === "Purchase receipt.pdf", {
      timeout: 5000,
      timeoutMsg: "the viewer never opened on the PDF",
    });
    await waitForEvent("preview:pdf-ready", mark);
    expect(logLines("HOPLODEX_E2E_CONSENT_LOG")).toHaveLength(asked);
    expect(logLines("HOPLODEX_E2E_OPENED_LOG")).toHaveLength(opened);
    await closeViewer();
  });

  it("US3-6: changing back to Preview in HoploDex asks nothing, and the name previews again", async () => {
    const asked = logLines("HOPLODEX_E2E_CONSENT_LOG").length;

    await setOpening("Preview in HoploDex");

    expect(logLines("HOPLODEX_E2E_CONSENT_LOG")).toHaveLength(asked);
    expect(await nameTitle("owners-manual-notes.txt")).toBe("Preview owners-manual-notes.txt");
    await openDocument("owners-manual-notes.txt");
    await $(".hd-dialog__content--xl pre.hd-preview__text").waitForExist();
    await closeViewer();
  });
});
