import { spawn, type ChildProcess } from "node:child_process";
import { once } from "node:events";
import { createInterface } from "node:readline";
import type { Readable } from "node:stream";

/**
 * A virtual X display of the worker's own (Linux only), so each WebdriverIO
 * worker's app renders on a screen no other worker shares: real input
 * (support/realInput.ts) moves the pointer and sends keys to whatever window
 * is on the display, and the app never shows on the developer's desktop.
 * Its environment, `DISPLAY` and the x11 session variables, is set on this
 * process, which the app and e2e/scripts/x11-input.py inherit. run-e2e.mjs
 * drops `WAYLAND_DISPLAY` so the testrunner doesn't point GTK at the
 * developer's compositor instead.
 *
 * Xvfb runs with `-noreset`. Without it, an X server resets when its last
 * client leaves and drops any connection still being set up. The app's
 * sandbox probe and the probe's web processes are often the display's only
 * clients, so the app's own GTK init could connect just as they left and
 * fail ("Failed to initialize gtk backend!"). `@wdio/display-server`'s
 * `XvfbDisplayServer`, which the testrunner uses for its own shared display
 * (wdio.conf.ts turns that one off), takes no extra arguments, so Xvfb is
 * started here.
 */

let xvfb: ChildProcess | undefined;

/** Starts Xvfb on a free display number and points `DISPLAY` at it. Room for
 * a full-page screenshot's taller window, in true color. */
export async function startDisplay() {
  if (process.platform !== "linux") return;
  if (xvfb) throw new Error("the worker's display is already running");
  // -displayfd 3: Xvfb claims the first free display and writes its number
  // there once it listens.
  const child = spawn(
    "Xvfb",
    ["-displayfd", "3", "-screen", "0", "1920x4200x24", "-nolisten", "tcp", "-noreset"],
    { stdio: ["ignore", "ignore", "ignore", "pipe"] },
  );
  xvfb = child;
  const killOnExit = () => child.kill("SIGKILL");
  process.once("exit", killOnExit);
  child.once("exit", () => process.off("exit", killOnExit));
  const failed = new Promise<never>((_, reject) => {
    child.once("error", reject);
    child.once("exit", (code, signal) => reject(new Error(`Xvfb exited (${code ?? signal})`)));
  });
  // Only the race below waits on it; a later exit is the normal stop.
  failed.catch(() => {});
  const lines = createInterface({ input: child.stdio[3] as Readable });
  try {
    const [display] = await Promise.race([
      once(lines, "line", { signal: AbortSignal.timeout(10000) }),
      failed,
    ]);
    Object.assign(process.env, {
      DISPLAY: `:${String(display).trim()}`,
      GDK_BACKEND: "x11",
      XDG_SESSION_TYPE: "x11",
    });
  } catch (error) {
    await stopDisplay();
    throw error;
  } finally {
    lines.close();
  }
}

/** Ends the worker's display. The next session's startDisplay() replaces
 * the variables it set. */
export async function stopDisplay() {
  const stopping = xvfb;
  xvfb = undefined;
  if (!stopping || stopping.pid === undefined) return;
  if (stopping.exitCode !== null || stopping.signalCode !== null) return;
  const exited = once(stopping, "exit");
  stopping.kill("SIGTERM");
  const forced = setTimeout(() => stopping.kill("SIGKILL"), 1000);
  await exited;
  clearTimeout(forced);
}
