import { spawn, type ChildProcess } from "node:child_process";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import { $, browser } from "@wdio/globals";
import { settle } from "./ui";

/**
 * Real keyboard and mouse input for a spec, through the X server
 * (e2e/scripts/x11-input.py), for WebKitGTK behaviour WebDriver's own input
 * doesn't reproduce: whether a focus shows its ring, or a relayout that only
 * a real key press forces; and for keyboard-only flows, since WebDriver's
 * key presses are synthetic and neither move focus on Tab nor type text.
 * Linux (Xvfb) only: a test that uses it starts with
 * {@link skipWithoutRealInput}, so other platforms skip it. See "Real
 * keyboard and mouse input" in DEVELOPMENT.md.
 *
 * Each call returns once the page has seen the input and settled, not after
 * a fixed pause (#77, step 10): see "Waiting for the page to see the input"
 * below. The script runs as one process for the worker's life, not one for
 * each key.
 */

const SCRIPT = fileURLToPath(new URL("../scripts/x11-input.py", import.meta.url));

/** Whether this computer can send real input: an X server, so Linux only.
 * Windows and macOS skip these tests rather than port it (#27). */
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

/** The worker's `x11-input.py serve`: a command a line in, "ok" a line out
 * once the X server has processed it. The first command starts it. */
let server: { child: ChildProcess; replies: AsyncIterator<string> } | undefined;

function startServer() {
  const child = spawn("python3", [SCRIPT, "serve"], { stdio: ["pipe", "pipe", "inherit"] });
  const replies = createInterface({ input: child.stdout! })[Symbol.asyncIterator]();
  // The script ends with its stdin, so it needn't keep the worker alive.
  child.unref();
  child.stdin!.on("error", () => {}); // the missing reply reports it
  child.on("exit", () => {
    if (server?.child === child) server = undefined;
  });
  return { child, replies };
}

async function send(...args: string[]) {
  if (!HAS_REAL_INPUT) {
    throw new Error("real input needs Linux (X11): start the test with skipWithoutRealInput");
  }
  server ??= startServer();
  server.child.stdin!.write(`${args.join(" ")}\n`);
  const { value, done } = await server.replies.next();
  if (done || value !== "ok") throw new Error(`x11-input: ${done ? "exited" : value}`);
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
  await send("move", "400", "400");
  await browser.pause(100);
  await browser.execute(() => {
    (window as unknown as { __hdPointer?: [number, number] | null }).__hdPointer = null;
  });
  await send("move", "410", "410");
  const seen = await browser.waitUntil(pointer, {
    timeout: 3000,
    timeoutMsg: "the page saw no real pointer move",
  });
  offset = { x: 410 - seen[0], y: 410 - seen[1] };
}

// --- Waiting for the page to see the input ----------------------------------
//
// After each event, wait until the page has seen it end (a capture listener
// on the window counts keyups and mouse and pointer releases, and keeps the
// pointer's last position), then settle(), as every other step does, so the
// page's handlers, React's commit and any backend call they started are done.
// That settle leaves animations running, as the fixed pause did: a test may
// look at one the input started (us7's owl blinking).
//
// A key whose press moves the focus out of this web view (F6 into the PDF
// surface) is done once its keydown is seen. Keys sent while the page has no
// focus can't be seen at all. Those fall back to the old fixed pause, as does
// a wait that times out (also when the page was replaced meanwhile).

type Seen = {
  keydown: number;
  keyup: number;
  mouseup: number;
  pointer: [number, number] | null;
};

/** Installs the page's counters if they aren't there (a new page), and
 * returns them, with whether the page has the focus. */
async function mark(): Promise<Seen & { focused: boolean }> {
  return browser.execute(() => {
    const w = window as unknown as { __hdSeen?: Seen };
    if (!w.__hdSeen) {
      const seen: Seen = { keydown: 0, keyup: 0, mouseup: 0, pointer: null };
      w.__hdSeen = seen;
      window.addEventListener("keydown", () => (seen.keydown += 1), true);
      window.addEventListener("keyup", () => (seen.keyup += 1), true);
      // Both: a pointerdown handler that calls preventDefault() (Radix menus
      // open so) suppresses mouseup, and the back and forward buttons send no
      // pointerup. One release may count twice; each wait reads a new mark.
      window.addEventListener("mouseup", () => (seen.mouseup += 1), true);
      window.addEventListener("pointerup", () => (seen.mouseup += 1), true);
      window.addEventListener("mousemove", (e) => (seen.pointer = [e.clientX, e.clientY]), true);
    }
    return { ...w.__hdSeen, focused: document.hasFocus() };
  });
}

const SEEN_TIMEOUT_MS = 1000;
const FALLBACK_PAUSE_MS = 150;

type Want =
  | { kind: "keys"; keydown: number; keyup: number }
  | { kind: "mouseup"; mouseup: number }
  | { kind: "pointer"; at: number[] };

/** Waits until the page has seen what `want` says, for at most
 * {@link SEEN_TIMEOUT_MS}: "seen", "focus-left" (the keys' presses arrived and
 * took the focus out of this web view, so their releases go elsewhere), or
 * "timeout". */
async function seenBy(want: Want): Promise<"seen" | "focus-left" | "timeout"> {
  return browser.execute(
    async (wanted: Want, limit: number) => {
      const w = window as unknown as { __hdSeen?: Seen };
      const check = (seen: Seen) => {
        if (wanted.kind === "pointer") {
          const [x, y] = wanted.at;
          return seen.pointer !== null &&
            Math.abs(seen.pointer[0] - x) <= 1 &&
            Math.abs(seen.pointer[1] - y) <= 1
            ? "seen"
            : "";
        }
        if (wanted.kind === "mouseup") return seen.mouseup >= wanted.mouseup ? "seen" : "";
        if (seen.keyup >= wanted.keyup) return "seen";
        return seen.keydown >= wanted.keydown && !document.hasFocus() ? "focus-left" : "";
      };
      const until = performance.now() + limit;
      for (;;) {
        if (!w.__hdSeen) return "timeout";
        const now = check(w.__hdSeen);
        if (now) return now;
        if (performance.now() > until) return "timeout";
        await new Promise((resolve) => setTimeout(resolve, 2));
      }
    },
    want,
    SEEN_TIMEOUT_MS,
  ) as Promise<"seen" | "focus-left" | "timeout">;
}

/** After the page has seen the input: its handlers are done, React has
 * committed, and two frames have passed. */
async function settleAfterInput() {
  await settle(10000, { animations: false });
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
  const x = Math.round(middle.x + offset!.x);
  const y = Math.round(middle.y + offset!.y);
  const before = await mark();
  await send("move", String(x), String(y));
  // The page sees the pointer arrive, unless it was there already.
  const page = [x - offset!.x, y - offset!.y];
  const there =
    before.pointer !== null &&
    Math.abs(before.pointer[0] - page[0]) <= 1 &&
    Math.abs(before.pointer[1] - page[1]) <= 1;
  if (!there && (await seenBy({ kind: "pointer", at: page })) === "timeout") {
    await browser.pause(100);
  }
  await send("click");
  if ((await seenBy({ kind: "mouseup", mouseup: before.mouseup + 1 })) === "timeout") {
    await browser.pause(FALLBACK_PAUSE_MS);
  }
  await settleAfterInput();
}

/** Presses real keys: X keysym names, `+` for a chord ("Tab", "Shift_L+Tab",
 * "Escape", "Return", "Down"). The pointer must be over the window, as it is
 * after {@link realClick}. */
export async function realKey(keys: string) {
  if (!offset) await calibrate();
  const before = await mark();
  await send("key", keys);
  if (!before.focused) {
    // The keys go to another web view (the PDF surface), which this page
    // can't see.
    await browser.pause(FALLBACK_PAUSE_MS);
    return;
  }
  const count = keys.split("+").length;
  const seen = await seenBy({
    kind: "keys",
    keydown: before.keydown + count,
    keyup: before.keyup + count,
  });
  if (seen === "timeout") await browser.pause(FALLBACK_PAUSE_MS);
  if (seen === "seen") await settleAfterInput();
}

/** Presses and releases a mouse button by its X number, with the pointer
 * where it is: 8 is the back button and 9 the forward button. */
export async function realMouseButton(button: number) {
  if (!offset) await calibrate();
  const before = await mark();
  await send("button", String(button));
  if ((await seenBy({ kind: "mouseup", mouseup: before.mouseup + 1 })) === "timeout") {
    await browser.pause(FALLBACK_PAUSE_MS);
  }
  await settleAfterInput();
}
