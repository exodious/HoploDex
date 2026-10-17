import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

/**
 * Real input on macOS, through `src-tauri/examples/real_input.swift` (the
 * tool the window controls and PDF surface checks use): the keys a person
 * presses, the menu bar they use and the quit event the Dock sends, for the
 * app's quit routes (#107), which
 * WebDriver can't reach and which differ on each OS. Needs Accessibility for
 * whatever runs the suite, as the macOS checks in DEVELOPMENT.md do. A test
 * that uses it starts with {@link skipWithoutMacInput}.
 *
 * Every function here is addressed to the process `pid` (a key by
 * `CGEvent.postToPid`, the menu bar and the quit events through the
 * Accessibility API and Apple Events), because the suite's workers run apps
 * side by side and a new one takes the front. Only the real_input tool's
 * `activate`, which {@link pressCommandQ} and {@link chooseMenuBarItem} call
 * first, and its pointer and `key` commands (not used here) go to the
 * frontmost app or the screen.
 */

const SOURCE = fileURLToPath(new URL("../../src-tauri/examples/real_input.swift", import.meta.url));

/** Whether this computer is a Mac. */
export const HAS_MAC_INPUT = process.platform === "darwin";

/** Skips the test, or from a `before` hook the whole block, anywhere but
 * macOS. Call it from a `function`, not an arrow, to have Mocha's `this`. */
export function skipWithoutMacInput(context: Mocha.Context) {
  if (!HAS_MAC_INPUT) context.skip();
}

let tool: string | undefined;

/** The compiled tool, built once per worker into a scratch folder. */
function realInput(): string {
  if (!tool) {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), "hoplodex-real-input-"));
    tool = path.join(dir, "real_input");
    execFileSync("swiftc", ["-O", "-o", tool, SOURCE], { stdio: "inherit" });
  }
  return tool;
}

/** Makes `pid`'s app the frontmost one, so the menu bar and a key sent to the
 * frontmost app reach it (addressed to the frontmost app, not to `pid`: it
 * can lose the front again at once, to another app that launches). */
function bringToFront(pid: number) {
  execFileSync(realInput(), ["activate", String(pid)]);
}

/** Presses ⌘Q in the app with process id `pid`. The key is posted to that
 * process alone (`CGEvent.postToPid`), not to whichever app is frontmost when
 * it arrives: another worker's app, launched meanwhile, takes the front, and
 * a global ⌘Q would quit it instead (#122). */
export function pressCommandQ(pid: number) {
  bringToFront(pid);
  // 12 is the virtual key code of Q.
  execFileSync(realInput(), ["key-to", String(pid), "12", "cmd"]);
}

/** Quits the app with process id `pid` as the Dock's Quit does: with the
 * quit Apple Event, addressed to that one process (`terminate:` on its side). */
export function sendQuitEvent(pid: number) {
  const result = execFileSync(realInput(), ["quit", String(pid)])
    .toString()
    .trim();
  if (result !== "sent") throw new Error("the quit event wasn't sent");
}

/** Quits the app with process id `pid` as loginwindow does when a log out,
 * restart or shut down asks every application to quit: the quit Apple Event
 * with its `kAEQuitReason` (`kAEReallyLogOut`, "rlgo"), sent to that one
 * process (003 FR-039, #107). */
export function sendLogOutQuitEvent(pid: number) {
  const result = execFileSync(realInput(), ["quit", String(pid), "rlgo"])
    .toString()
    .trim();
  if (result !== "sent") throw new Error(`the log out's quit event wasn't sent: ${result}`);
}

/** The title of the application menu's Quit item in the app with process id
 * `pid`: "Quit" and the process's localized name, which is the executable's in
 * the E2E build and the bundle's in a release. */
export function quitMenuItemTitle(pid: number): string {
  const paths = execFileSync(realInput(), ["menu", String(pid)]).toString();
  const title = /(?:^|> )(Quit [^>\n]*?)(?: \[disabled\])?$/m.exec(paths)?.[1];
  if (!title) throw new Error(`the menu bar has no Quit item:\n${paths}`);
  return title;
}

/** Chooses the menu bar item titled `title` in the app with process id `pid`
 * through the Accessibility API, as VoiceOver would (addressed to that
 * process; the app is brought to the front first). */
export function chooseMenuBarItem(pid: number, title: string) {
  bringToFront(pid);
  const result = execFileSync(realInput(), ["press", String(pid), title])
    .toString()
    .trim();
  if (result !== "pressed") throw new Error(`the menu bar has no "${title}" item`);
}
