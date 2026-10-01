import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { $, browser } from "@wdio/globals";

/**
 * Real keyboard and mouse input for a spec, through the X server
 * (e2e/scripts/x11-input.py), for WebKitGTK behaviour WebDriver's own input
 * doesn't reproduce: whether a focus shows its ring, or a relayout that only
 * a real key press forces. Linux (Xvfb) only. See "Real keyboard and mouse
 * input" in DEVELOPMENT.md.
 */

const SCRIPT = fileURLToPath(new URL("../scripts/x11-input.py", import.meta.url));

/** Where the page's (0, 0) is on the X screen, found by {@link calibrate}. */
let offset: { x: number; y: number } | null = null;

function send(...args: string[]) {
  execFileSync("python3", [SCRIPT, ...args]);
}

/** Moves the real pointer onto the window once and reads where the page saw
 * it, so page coordinates can be turned into screen ones. */
async function calibrate() {
  await browser.execute(() => {
    const seen = window as unknown as { __hdPointer?: [number, number] };
    window.addEventListener("mousemove", (e) => (seen.__hdPointer = [e.clientX, e.clientY]), {
      once: true,
    });
  });
  send("move", "400", "400");
  await browser.pause(100);
  send("move", "410", "410");
  const seen = await browser.waitUntil(
    () =>
      browser.execute(
        () => (window as unknown as { __hdPointer?: [number, number] }).__hdPointer ?? false,
      ),
    { timeout: 3000, timeoutMsg: "the page saw no real pointer move" },
  );
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
