import { spawn, type ChildProcess } from "node:child_process";

/**
 * A virtual X display of the worker's own (Linux only), so each WebdriverIO
 * worker's app renders on a screen no other worker shares: real input
 * (support/realInput.ts) moves the pointer and sends keys to whatever window
 * is on the display, and the app never shows on the developer's desktop.
 * The display is set in this process's `DISPLAY`, which the app and
 * e2e/scripts/x11-input.py inherit. run-e2e.mjs forces GTK's X11 backend so
 * the app honours it even when `WAYLAND_DISPLAY` was set.
 */

let xvfb: ChildProcess | undefined;

/** Starts Xvfb on a free display number and points `DISPLAY` at it. Room for
 * a full-page screenshot's taller window, in true color. */
export async function startDisplay() {
  if (process.platform !== "linux") return;
  if (xvfb) throw new Error("the worker's display is already running");
  // -displayfd: Xvfb picks the first free display number and writes it to
  // fd 3 once it accepts connections.
  const child = spawn(
    "Xvfb",
    ["-displayfd", "3", "-screen", "0", "1920x4200x24", "-nolisten", "tcp"],
    { stdio: ["ignore", "ignore", "pipe", "pipe"] },
  );
  xvfb = child;
  // Xvfb's stderr is noise (missing keysyms, font paths) unless it fails.
  let stderr = "";
  child.stderr!.on("data", (chunk) => (stderr += chunk));
  const display = await new Promise<string>((resolve, reject) => {
    let number = "";
    child.once("error", (error) => reject(new Error(`Xvfb didn't start: ${error.message}`)));
    child.once("exit", (code, signal) =>
      reject(new Error(`Xvfb exited at start (${code ?? signal}):\n${stderr}`)),
    );
    child.stdio[3]!.on("data", (chunk) => {
      number += chunk;
      if (number.includes("\n")) resolve(`:${number.trim()}`);
    });
  });
  process.env.DISPLAY = display;
}

/** Ends the worker's display. */
export async function stopDisplay() {
  const child = xvfb;
  xvfb = undefined;
  if (!child || child.exitCode !== null || child.signalCode !== null) return;
  const exited = new Promise((resolve) => child.once("exit", resolve));
  child.kill("SIGTERM");
  await exited;
}
