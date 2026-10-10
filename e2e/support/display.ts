import { XvfbDisplayServer, type DisplayDaemon } from "@wdio/display-server";

/**
 * A virtual X display of the worker's own (Linux only), so each WebdriverIO
 * worker's app renders on a screen no other worker shares: real input
 * (support/realInput.ts) moves the pointer and sends keys to whatever window
 * is on the display, and the app never shows on the developer's desktop.
 * Xvfb is started through `@wdio/display-server`, which the testrunner uses
 * for its own shared display too (wdio.conf.ts turns that one off). Its
 * environment, `DISPLAY` and the x11 session variables, is set on this
 * process, which the app and e2e/scripts/x11-input.py inherit. run-e2e.mjs
 * drops `WAYLAND_DISPLAY` so the testrunner doesn't point GTK at the
 * developer's compositor instead.
 */

let daemon: DisplayDaemon | undefined;

/** Starts Xvfb on a free display number and points `DISPLAY` at it. Room for
 * a full-page screenshot's taller window, in true color. */
export async function startDisplay() {
  if (process.platform !== "linux") return;
  if (daemon) throw new Error("the worker's display is already running");
  // Not DisplayServerManager: it starts nothing when DISPLAY is already set,
  // as it is when the run is started from a desktop session, and every
  // worker needs a display of its own regardless.
  daemon = await new XvfbDisplayServer().startDaemon({ width: 1920, height: 4200, depth: 24 });
  Object.assign(process.env, daemon.env);
}

/** Ends the worker's display. The next session's startDisplay() replaces
 * the variables it set. */
export async function stopDisplay() {
  const stopping = daemon;
  daemon = undefined;
  await stopping?.stop();
}
