import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

/**
 * Real input on macOS, through `src-tauri/examples/real_input.swift` (the
 * tool the window controls and PDF surface checks use): the keys a person
 * presses and the menu bar they use, for the app's quit routes (#107), which
 * WebDriver can't reach and which differ on each OS. Needs Accessibility for
 * whatever runs the suite, as the macOS checks in DEVELOPMENT.md do. A test
 * that uses it starts with {@link skipWithoutMacInput}.
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

/** Makes `pid`'s app the frontmost one, so a key press reaches it. */
function bringToFront(pid: number) {
  execFileSync(realInput(), ["activate", String(pid)]);
}

/** Presses ⌘Q in the app with process id `pid`. */
export function pressCommandQ(pid: number) {
  bringToFront(pid);
  // 12 is the virtual key code of Q.
  execFileSync(realInput(), ["key", "12", "cmd"]);
}

/** Chooses the menu bar item titled `title` in the app with process id `pid`
 * through the Accessibility API, as VoiceOver would. */
export function chooseMenuBarItem(pid: number, title: string) {
  bringToFront(pid);
  const result = execFileSync(realInput(), ["press", String(pid), title])
    .toString()
    .trim();
  if (result !== "pressed") throw new Error(`the menu bar has no "${title}" item`);
}
