import {
  GLOCK,
  clickRowButton,
  closeViewer,
  logLines,
  openDocument,
  openedRoot,
  recordPreviewEvents,
  eventMark,
  waitForEvent,
  relaunchAnswering,
  viewerTitle,
  chooseMainCollection,
  openRecord,
} from "../support/documentPreview";
import {
  $,
  browser,
  choose,
  chooseMenuItem,
  clickButton,
  expect,
  invokeCommand,
  settle,
  switchDatabase,
  unlock,
} from "../support/ui";

/**
 * End-to-end coverage of specs/007-document-preview's User Story 3 (spec.md,
 * contracts/ui-document-preview.md): choosing how documents open, in Database
 * settings, against the real built app on the human-testing seed's Glock 19
 * Gen5 (the harness seeds the collection for this spec, wdio.conf.ts). The
 * helpers are in `support/documentPreview.ts`. Its sibling specs are the
 * preview itself (`us13-document-preview-preview`) and opening in another app
 * (`us13-document-preview-open-elsewhere`).
 */

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

  it("US3-2: changing to Open in another app asks, in the consent log, and takes effect; US3-1, US3-3, US3-5 and US3-6 are its steps", async function () {
    // US3-1: it starts at Preview in HoploDex, and the name previews.
    await openDatabaseSettings();
    expect(await openingChecked("Preview in HoploDex")).toBe(true);
    expect(await openingChecked("Open in another app")).toBe(false);
    await closeDatabaseSettings();
    expect(await nameTitle("owners-manual-notes.txt")).toBe("Preview owners-manual-notes.txt");

    // US3-2: changing to Open in another app asks once, in the consent log.
    let asked = logLines("HOPLODEX_E2E_CONSENT_LOG").length;
    await setOpening("Open in another app");

    const lines = logLines("HOPLODEX_E2E_CONSENT_LOG");
    expect(lines).toHaveLength(asked + 1);
    expect(lines.at(-1)).toContain(SETTING_TITLE);
    expect(await nameTitle("Bill of sale.docx")).toBe("Open Bill of sale.docx in another app");
    // The setting is saved at once, without the dialog's Save.
    await openDatabaseSettings();
    expect(await openingChecked("Open in another app")).toBe(true);
    await closeDatabaseSettings();

    // US3-3: opening two documents by name asks once, and opens both.
    if (openedRoot) {
      asked = logLines("HOPLODEX_E2E_CONSENT_LOG").length;
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
    }

    // US3-5: Preview on the PDF previews it, and asks nothing.
    asked = logLines("HOPLODEX_E2E_CONSENT_LOG").length;
    const openedBefore = openedRoot ? logLines("HOPLODEX_E2E_OPENED_LOG").length : 0;

    await recordPreviewEvents();
    const mark = await eventMark();
    await clickRowButton("Purchase receipt.pdf", "Preview");

    await browser.waitUntil(async () => (await viewerTitle()) === "Purchase receipt.pdf", {
      timeout: 5000,
      timeoutMsg: "the viewer never opened on the PDF",
    });
    await waitForEvent("preview:pdf-ready", mark);
    expect(logLines("HOPLODEX_E2E_CONSENT_LOG")).toHaveLength(asked);
    if (openedRoot) expect(logLines("HOPLODEX_E2E_OPENED_LOG")).toHaveLength(openedBefore);
    await closeViewer();

    // US3-6: changing back to Preview in HoploDex asks nothing, and the name
    // previews again.
    await setOpening("Preview in HoploDex");

    expect(logLines("HOPLODEX_E2E_CONSENT_LOG")).toHaveLength(asked);
    expect(await nameTitle("owners-manual-notes.txt")).toBe("Preview owners-manual-notes.txt");
    await openDocument("owners-manual-notes.txt");
    await $(".hd-dialog__content--xl pre.hd-preview__text").waitForExist();
    await closeViewer();
  });

  it("US3-4: after a lock and an unlock, the next open asks again", async function () {
    if (!openedRoot) return this.skip();
    // The journey above leaves the setting at Preview: set it again, and open
    // one document, so the lock has an open this session to forget.
    await setOpening("Open in another app");
    let asked = logLines("HOPLODEX_E2E_CONSENT_LOG").length;
    let opened = logLines("HOPLODEX_E2E_OPENED_LOG").length;
    await clickDocumentName("Bill of sale.docx");
    await browser.waitUntil(() => logLines("HOPLODEX_E2E_OPENED_LOG").length > opened, {
      timeout: 10000,
      timeoutMsg: "the document was never opened in another app before the lock",
    });
    expect(logLines("HOPLODEX_E2E_CONSENT_LOG")).toHaveLength(asked + 1);

    await switchDatabase();
    await chooseMainCollection();
    await unlock(process.env.HOPLODEX_E2E_SEED_PASSPHRASE!);
    await openRecord(GLOCK);
    asked = logLines("HOPLODEX_E2E_CONSENT_LOG").length;
    opened = logLines("HOPLODEX_E2E_OPENED_LOG").length;
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
});
