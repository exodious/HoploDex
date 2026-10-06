import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { $, browser } from "@wdio/globals";

/**
 * Real keyboard and mouse input for a spec, through the X server
 * (e2e/scripts/x11-input.py), for WebKitGTK behaviour WebDriver's own input
 * doesn't reproduce: whether a focus shows its ring, or a relayout that only
 * a real key press forces. Linux (Xvfb) only: a test that uses it starts
 * with {@link skipWithoutRealInput}, so other platforms skip it. See "Real
 * keyboard and mouse input" in DEVELOPMENT.md.
 */

const SCRIPT = fileURLToPath(new URL("../scripts/x11-input.py", import.meta.url));

/** Whether this computer can send real input: an X server, so Linux only for
 * now (#28). */
export const HAS_REAL_INPUT = process.platform === "linux";

/** Skips the test, or from a `before` hook the whole block, where there is no
 * real input, so the run reports it pending rather than failed. Call it from
 * a `function`, not an arrow, to have Mocha's `this`, in every test that
 * sends real input or needs what such a test did. */
export function skipWithoutRealInput(context: Mocha.Context) {
  if (!HAS_REAL_INPUT) context.skip();
}

/** Where the page's (0, 0) is on the X screen, found by {@link calibrate}. */
let offset: { x: number; y: number } | null = null;

function send(...args: string[]) {
  if (!HAS_REAL_INPUT) {
    throw new Error("real input needs Linux (X11): start the test with skipWithoutRealInput");
  }
  execFileSync("python3", [SCRIPT, ...args]);
}

/** Moves the real pointer onto the window, then by a known step, and reads
 * where the page saw the step land, so page coordinates can be turned into
 * screen ones. Whether the first move reaches the page as a `mousemove` of
 * its own depends on the driver, so the reading is the first one after the
 * step. */
async function calibrate() {
  await browser.execute(() => {
    const seen = window as unknown as { __hdPointer?: [number, number] | null };
    seen.__hdPointer = null;
    window.addEventListener("mousemove", (e) => (seen.__hdPointer = [e.clientX, e.clientY]));
  });
  const pointer = () =>
    browser.execute(
      () => (window as unknown as { __hdPointer?: [number, number] | null }).__hdPointer ?? false,
    );
  send("move", "400", "400");
  await browser.pause(100);
  await browser.execute(() => {
    (window as unknown as { __hdPointer?: [number, number] | null }).__hdPointer = null;
  });
  send("move", "410", "410");
  const seen = await browser.waitUntil(pointer, {
    timeout: 3000,
    timeoutMsg: "the page saw no real pointer move",
  });
  offset = { x: 410 - seen[0], y: 410 - seen[1] };
}

/** Clicks the middle of `selector` with the real pointer. */
export async function realClick(selector: string) {
  if (!offset) await calibrate();
  const el = await $(selector);
  await el.waitForExist();
  const middle = await browser.execute((element: HTMLElement) => {
    // A click below the fold of a scrolling dialog lands on whatever is
    // there instead (a form that grew a field pushed this one down).
    element.scrollIntoView({ block: "center" });
    const r = element.getBoundingClientRect();
    return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
  }, el);
  send("move", String(Math.round(middle.x + offset!.x)), String(Math.round(middle.y + offset!.y)));
  await browser.pause(100);
  send("click");
  await browser.pause(150);
}

/** Presses real keys: X keysym names, `+` for a chord ("Tab", "Shift_L+Tab",
 * "Escape", "Return", "Down"). The pointer must be over the window, as it is
 * after {@link realClick}. */
export async function realKey(keys: string) {
  if (!offset) await calibrate();
  send("key", keys);
  await browser.pause(150);
}
