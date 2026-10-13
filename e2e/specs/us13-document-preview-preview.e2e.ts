import { execFileSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import zlib from "node:zlib";
import { appPid, relaunchApp } from "../support/app";
import {
  GLOCK,
  chooseMainCollection,
  closeViewer,
  eventMark,
  openDocument,
  openRecord,
  recordPreviewEvents,
  viewerStatuses,
  viewerTitle,
  waitForEvent,
} from "../support/documentPreview";
import { writeLargePdf } from "../support/largePdf";
import { realClick, realKey, skipWithoutRealInput } from "../support/realInput";
import { macWindowShot } from "../support/screenshots";
import {
  $,
  $$,
  attachFile,
  browser,
  chooseMenuItem,
  clickEl,
  expect,
  goTo,
  invokeCommand,
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
 * and US3 are `us13-document-preview-open-elsewhere` and `-setting`, and the
 * helpers they share with this one are in `support/documentPreview.ts`.
 *
 * What happens inside the PDF surface (the viewer's own controls, every
 * hostile PDF) is the surface check's (research.md §23); this spec drives the
 * main web view and watches what HoploDex itself does: the viewer, the events
 * the backend sends it, what is left on disk, and what is on the screen.
 */

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

/** Leaves a record for the Collection page and opens the Glock again, which
 * reloads its document list. */
async function reopenGlock() {
  await goTo("Collection");
  await openRecord(GLOCK);
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

/** An error `invoke` rejects with, as the command's `CommandError`, or null. */
async function invokeFailure(
  cmd: string,
  args: Record<string, unknown>,
): Promise<{ code: string } | null> {
  return browser.execute(
    async (name: string, data: Record<string, unknown>) => {
      const internals = (
        window as unknown as {
          __TAURI_INTERNALS__: {
            invoke: (cmd: string, args: unknown, options?: unknown) => Promise<unknown>;
          };
        }
      ).__TAURI_INTERNALS__;
      const status = (await internals.invoke("get_database_status", {})) as {
        sessionId?: number | null;
      };
      try {
        await internals.invoke(
          name,
          data,
          status.sessionId == null
            ? undefined
            : { headers: { "HoploDex-Session": String(status.sessionId) } },
        );
        return null;
      } catch (error) {
        return error;
      }
    },
    cmd,
    args,
  ) as Promise<{ code: string } | null>;
}

async function glockId(): Promise<number> {
  // list_firearms names its session in the HoploDex-Session header (spec 008
  // FR-004), as the frontend's scope sends it; the id comes from the status.
  const listed = (await browser.execute(async (query: string) => {
    const internals = (
      window as unknown as {
        __TAURI_INTERNALS__: {
          invoke: (cmd: string, args: unknown, options?: unknown) => Promise<unknown>;
        };
      }
    ).__TAURI_INTERNALS__;
    const status = (await internals.invoke("get_database_status", {})) as { sessionId: number };
    return internals.invoke(
      "list_firearms",
      { input: { query } },
      { headers: { "HoploDex-Session": String(status.sessionId) } },
    );
  }, GLOCK_SERIAL)) as {
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

/** The window as the user sees it, which includes the PDF surface (a
 * WebDriver screenshot holds only the main web view): the whole X screen on
 * Linux, where the worker's window is the only one and sits at 0,0; on Windows
 * the client area of this worker's window (e2e/scripts/window-shot.ps1); on
 * macOS this worker's window cropped to the page (`macWindowShot`). So a point
 * in the page is the same point in the picture on all three, and the other
 * workers' windows can't show in it (#88). */
async function windowScreenshot(): Promise<Screen> {
  if (process.platform === "darwin") {
    // This worker's window alone, cropped to the page (macWindowShot): the
    // page's origin is the picture's.
    const file = path.join(os.tmpdir(), `hoplodex-e2e-screen-${process.pid}.png`);
    try {
      await macWindowShot(file);
      return { ...decodePng(fs.readFileSync(file)), originX: 0, originY: 0 };
    } finally {
      fs.rmSync(file, { force: true });
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
          "-ProcessId",
          String(appPid()),
          "-Out",
          file,
          "-Format",
          "ppm",
        ],
        // windowsHide: no console window of its own over the app.
        { stdio: "pipe", windowsHide: true },
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
    // Each zoom key moves its own way (research.md §14): − lowers the percent
    // and + raises it. The fit this scan opens at can lie outside 50-400% (on
    // Linux it is 789%), where the key that would move the wrong way does
    // nothing, so + first has to be left alone above 400%, and − twice puts the
    // zoom inside the range, where + raises it.
    const percentValue = async () => Number.parseInt(await percent(), 10);
    const startPercent = await percentValue();
    if (startPercent >= 400) {
      await realKey("KP_Add");
      await browser.pause(300);
      expect(await percentValue()).toBe(startPercent);
    }
    let previous = startPercent;
    for (let step = 0; step < 2; step++) {
      await realKey("KP_Subtract");
      const from = previous;
      await browser.waitUntil(async () => (await percentValue()) < from, {
        timeout: 5000,
        timeoutMsg: `− never lowered the zoom from ${from}% (it started at ${before})`,
      });
      previous = await percentValue();
    }
    await realKey("KP_Add");
    await browser.waitUntil(async () => (await percentValue()) > previous, {
      timeout: 5000,
      timeoutMsg: `+ never raised the zoom from ${previous}%`,
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
    await browser.waitUntil(async () => (await $$(".hd-toast").length) === 0, {
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
    if (process.platform === "win32") {
      // The surface held the keyboard focus. Closing it left the focus with no
      // web view on Windows, so the page's focus() fired no blur (a field was
      // never settled on leaving it) until the user clicked in the window.
      await browser.waitUntil(() => browser.execute(() => document.hasFocus()), {
        timeout: 5000,
        timeoutMsg: "the main web view never got the focus back after the viewer closed",
      });
    }
  });

  it("a lock with a PDF shown leaves nothing of it on the screen once the chooser is up", async function () {
    this.timeout(120000);
    const mark = await eventMark();
    await openDocument("Magenta.pdf");
    await waitForEvent("preview:pdf-ready", mark);
    await browser.waitUntil(async () => (await $$(".hd-toast").length) === 0, {
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
