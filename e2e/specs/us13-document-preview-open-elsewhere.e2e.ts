import fs from "node:fs";
import path from "node:path";
import {
  clickRowButton,
  closeViewer,
  logLines,
  openDocument,
  openedRoot,
  relaunchAnswering,
  viewerStatuses,
} from "../support/documentPreview";
import { $$, browser, expect, settle } from "../support/ui";

/**
 * End-to-end coverage of specs/007-document-preview's User Story 2 (spec.md,
 * contracts/ui-document-preview.md): opening a document in another app, after
 * the native confirmation, against the real built app on the human-testing
 * seed's Glock 19 Gen5 (the harness seeds the collection for this spec,
 * wdio.conf.ts). The seams the E2E build has for the confirmation and for the
 * other app log what they were asked; the helpers are in
 * `support/documentPreview.ts`. Its sibling specs are the preview itself
 * (`us13-document-preview-preview`) and the setting
 * (`us13-document-preview-setting`).
 */

function filesUnder(dir: string): string[] {
  if (!fs.existsSync(dir)) return [];
  return fs
    .readdirSync(dir, { withFileTypes: true })
    .flatMap((entry) =>
      entry.isDirectory() ? filesUnder(path.join(dir, entry.name)) : [path.join(dir, entry.name)],
    );
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
    await expect($$(".hd-toast--error")).toBeElementsArrayOfSize(0);
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
