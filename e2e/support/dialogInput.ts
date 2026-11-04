import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { browser } from "@wdio/globals";
import { appPid } from "./app";
import { keyTo, pressElement } from "./macInput";
import { realKeyOutsidePage } from "./realInput";
import { passphraseLog } from "./ui";

/**
 * Real keys into the native passphrase dialog (amended 2026-11-04 to 003
 * FR-007), which no WebDriver can reach, on every OS: X11's XTest on Linux
 * (`e2e/scripts/x11-input.py`, through `realInput.ts`), `real_input.swift`'s
 * `key-to` on macOS (to the app's process alone, `macInput.ts`), and
 * `real_input.ps1`'s SendKeys on Windows (to the foreground window, which the
 * dialog is). The app must have been launched with
 * `HOPLODEX_E2E_PASSPHRASE_DIALOG=native`; what the dialog shows comes back
 * through its log (`ui.ts`, `passphraseLog`). Texts are lowercase letters,
 * digits and spaces, which every OS's helper can type.
 */

export type DialogKey = "Return" | "Escape" | "Tab" | "space" | "BackSpace";

/** macOS's virtual key codes for what the texts and keys use. */
const MAC_CODES: Record<string, number> = {
  a: 0, s: 1, d: 2, f: 3, h: 4, g: 5, z: 6, x: 7, c: 8, v: 9, b: 11, q: 12, w: 13, e: 14,
  r: 15, y: 16, t: 17, "1": 18, "2": 19, "3": 20, "4": 21, "6": 22, "5": 23, "9": 25,
  "7": 26, "8": 28, "0": 29, o: 31, u: 32, i: 34, p: 35, l: 37, j: 38, k: 40, n: 45, m: 46,
  " ": 49, Return: 36, Tab: 48, BackSpace: 51, Escape: 53, space: 49,
};

const WINDOWS_KEYS: Record<DialogKey, string> = {
  Return: "{ENTER}",
  Escape: "{ESC}",
  Tab: "{TAB}",
  space: " ",
  BackSpace: "{BACKSPACE}",
};

const WINDOWS_INPUT = fileURLToPath(
  new URL("../../src-tauri/examples/real_input.ps1", import.meta.url),
);

function windowsKeys(keys: string) {
  execFileSync("powershell", [
    "-NoProfile",
    "-ExecutionPolicy",
    "Bypass",
    "-File",
    WINDOWS_INPUT,
    "key",
    keys,
  ]);
}

function checkText(text: string) {
  if (!/^[a-z0-9 ]*$/.test(text)) {
    throw new Error(`the dialog's real input types lowercase letters, digits and spaces only`);
  }
}

/** Types `text` into the dialog's focused field. */
export async function typeInDialog(text: string) {
  checkText(text);
  if (process.platform === "linux") {
    for (const char of text) await realKeyOutsidePage(char === " " ? "space" : char);
  } else if (process.platform === "darwin") {
    const pid = appPid();
    for (const char of text) keyTo(pid, MAC_CODES[char]);
  } else if (process.platform === "win32") {
    windowsKeys(text);
  } else {
    throw new Error(`no real input for the passphrase dialog on ${process.platform}`);
  }
}

/** Presses `key` in the dialog. */
export async function pressInDialog(key: DialogKey) {
  if (process.platform === "linux") await realKeyOutsidePage(key);
  else if (process.platform === "darwin") keyTo(appPid(), MAC_CODES[key]);
  else if (process.platform === "win32") windowsKeys(WINDOWS_KEYS[key]);
  else throw new Error(`no real input for the passphrase dialog on ${process.platform}`);
}

/** The 003 FR-004 acknowledgement, the create dialog's checkbox. */
const ACKNOWLEDGEMENT =
  "I have stored this passphrase somewhere safe. If it is forgotten, nobody, including HoploDex, can open this database or recover the collection.";

/** Ticks the create dialog's acknowledgement, with the focus in its
 * confirmation field: Tab past Show passphrases to it, then Space, on Linux and
 * Windows. macOS moves the focus between text fields only, unless the user
 * turned on full keyboard access, so there it is pressed through the
 * accessibility tree, as VoiceOver would. */
export async function tickAcknowledgement() {
  if (process.platform === "darwin") {
    if (!pressElement(appPid(), ACKNOWLEDGEMENT)) throw new Error("no acknowledgement to tick");
    return;
  }
  await pressInDialog("Tab");
  await pressInDialog("Tab");
  await pressInDialog("space");
}

/** How many lines the dialog's log has, to wait for what comes after. */
export function logMark(): number {
  return passphraseLog().length;
}

/** Waits for a line after `mark` that `matches`, and returns it. */
export async function waitForLog(
  mark: number,
  matches: (line: Record<string, unknown>) => boolean,
  what: string,
  timeout = 15000,
): Promise<Record<string, unknown>> {
  let found: Record<string, unknown> | undefined;
  await browser.waitUntil(
    () => {
      found = passphraseLog().slice(mark).find(matches);
      return found !== undefined;
    },
    { timeout, interval: 100, timeoutMsg: `the passphrase dialog's log never showed ${what}` },
  );
  return found!;
}

/** What the dialog showed last after `mark`: its button, the line under each
 * field and the strength label. */
export interface DialogView {
  ready: boolean;
  current: string | null;
  passphrase: string | null;
  confirmation: string | null;
  strength: string | null;
}

/** Waits for a view after `mark` that `matches`. */
export async function waitForView(
  mark: number,
  matches: (view: DialogView) => boolean,
  what: string,
): Promise<DialogView> {
  const line = await waitForLog(mark, (l) => "view" in l && matches(l.view as DialogView), what);
  return line.view as DialogView;
}
