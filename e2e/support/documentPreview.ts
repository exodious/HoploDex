import fs from "node:fs";
import path from "node:path";
import { relaunchApp } from "./app";
import {
  $,
  browser,
  expect,
  pressEscape,
  settle,
  unlock,
  waitForChooser,
  waitForCollection,
} from "./ui";

/**
 * What the document preview E2E specs share (specs/007-document-preview; the
 * `document-preview`, `document-open-elsewhere` and `document-open-setting`
 * E2E files, one per user story): finding the
 * seed's Glock and its documents, opening and closing the viewer, the preview
 * events the backend sends, the seams' logs and relaunching the app with the
 * consent seam answering. Each spec file is a fresh app and sandbox that the
 * harness seeds (wdio.conf.ts); `HOPLODEX_E2E_SEED_PASSPHRASE` holds the
 * seed's passphrase.
 */

export const GLOCK = "Glock 19 Gen5";

// --- Helpers ----------------------------------------------------------------

/** Opens a record from the collection list by the start of its name. */
export async function openRecord(name: string) {
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
export async function chooseMainCollection() {
  const select = await $('button.hd-db-row__select[aria-label^="Main collection, "]');
  if (await select.isExisting()) await select.click();
}

export type LoggedEvent = {
  event: string;
  at: number;
  payload: { previewId: number; reason?: string };
};

/** Starts recording the backend's preview events in the page, with the page's
 * clock, the way the frontend's own listeners get them
 * (`plugin:event|listen`, as @tauri-apps/api's `listen` does). Safe to call
 * again: it listens once per page. */
export async function recordPreviewEvents() {
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
export async function eventMark(): Promise<number> {
  return (await loggedEvents()).length;
}

/** Waits for the first `event` recorded after `mark`. */
export async function waitForEvent(
  event: string,
  mark: number,
  timeout = 15000,
): Promise<LoggedEvent> {
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
export async function openDocument(name: string): Promise<number> {
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
export async function viewerTitle(): Promise<string | null> {
  return browser.execute(
    () =>
      document.querySelector(".hd-dialog__content--xl .hd-dialog__title")?.textContent?.trim() ??
      null,
  );
}

export async function closeViewer() {
  await pressEscape();
  await browser.waitUntil(async () => (await viewerTitle()) === null, {
    timeout: 5000,
    timeoutMsg: "the viewer never closed",
  });
}

/** The text of every `role="status"` region in the viewer. */
export async function viewerStatuses(): Promise<string[]> {
  return browser.execute(() =>
    [...document.querySelectorAll('.hd-dialog__content--xl [role="status"]')].map(
      (el) => el.textContent?.trim() ?? "",
    ),
  );
}

/** Where the app puts the copies it hands to another app (documents.rs,
 * OPENED_DOCUMENTS_DIR), in the run's isolated cache folder. */
export const openedRoot = process.env.HOPLODEX_E2E_CACHE_HOME
  ? path.join(
      process.env.HOPLODEX_E2E_CACHE_HOME,
      "io.github.exodious.HoploDex",
      "opened-documents",
    )
  : undefined;

/** The lines of a seam's log file (HOPLODEX_E2E_CONSENT_LOG: the title of each
 * native confirmation the E2E build would have shown; HOPLODEX_E2E_OPENED_LOG:
 * the path of each copy it would have handed to another app). None if the
 * file isn't there yet. */
export function logLines(
  variable: "HOPLODEX_E2E_CONSENT_LOG" | "HOPLODEX_E2E_OPENED_LOG",
): string[] {
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
export async function relaunchAnswering(answer: "open" | "cancel") {
  process.env.HOPLODEX_E2E_CONSENT = answer;
  await relaunchApp();
  await waitForChooser();
  await chooseMainCollection();
  await unlock(process.env.HOPLODEX_E2E_SEED_PASSPHRASE!);
  await waitForCollection();
  await openRecord(GLOCK);
}

/** Presses a button of a document's row in the list by its text. */
export async function clickRowButton(documentName: string, label: string) {
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
