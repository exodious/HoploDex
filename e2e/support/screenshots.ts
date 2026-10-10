import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { browser } from "@wdio/globals";
import { settle } from "./ui";

/**
 * Screenshots of the real app (WebKitGTK, the engine users get), for pull
 * requests and docs. `npm run screenshots` runs the dedicated walk in
 * e2e/screenshots/; any other spec can call `shot()` too, and it saves only
 * when the run was started with `--screenshots` (which sets
 * HOPLODEX_SCREENSHOTS to the output directory), so ordinary E2E runs are
 * unaffected.
 */

/** Window size every screenshot run uses, so before/after shots line up. It
 * is the default window size in tauri.conf.json. */
export const SCREENSHOT_WINDOW = { width: 1200, height: 800 };

const outDir = process.env.HOPLODEX_SCREENSHOTS;

/**
 * Waits until the app is idle (`settle()`) and then until every finite
 * animation and transition has run to its end, however long. `settle()`
 * leaves the decorative ones alone (a firearm drawing drawing itself in, a
 * form section's highlight, 1.4 to 1.8 s) because a test must not wait on
 * them, but a screenshot must not catch one half-way. Endless animations (the
 * chooser plate's cycle) are skipped: they never finish, and the chooser's
 * shots stop them with `settleChooserPlate()`. Two animation frames pass
 * after the last one ends, so the final state is painted.
 */
export async function settleForShot(timeout = 10000) {
  await settle(timeout);
  const state = await browser.execute(async (limit: number) => {
    const running = () =>
      document
        .getAnimations()
        .filter(
          (a) =>
            (a.playState === "running" || a.pending) &&
            a.effect?.getComputedTiming().iterations !== Infinity,
        ).length;
    return new Promise<string>((resolve) => {
      let calm = 0;
      let last = 0;
      let over = false;
      const deadline = window.setTimeout(() => {
        over = true;
        resolve(`${last} animations never finished`);
      }, limit);
      const frame = () => {
        if (over) return;
        last = running();
        calm = last ? 0 : calm + 1;
        if (calm >= 2) {
          window.clearTimeout(deadline);
          resolve("");
        } else {
          requestAnimationFrame(frame);
        }
      };
      requestAnimationFrame(frame);
    });
  }, timeout);
  if (state) throw new Error(`The page has ${state}`);
}

export function screenshotsEnabled(): boolean {
  return Boolean(outDir);
}

/** Sets the window's size and waits until the page has been laid out at it:
 * `setWindowSize` can return before the window manager has resized the window.
 * (A size the window already has is not waited on.) */
export async function resizeWindow(width: number, height: number) {
  const before = await browser.getWindowSize();
  if (before.width === width && before.height === height) return;
  const inner = () => browser.execute(() => [window.innerWidth, window.innerHeight]);
  const [wasWidth, wasHeight] = await inner();
  await browser.setWindowSize(width, height);
  await browser.waitUntil(
    async () => {
      const [w, h] = await inner();
      return w !== wasWidth || h !== wasHeight;
    },
    { timeout: 5000, timeoutMsg: `the window never resized to ${width}x${height}` },
  );
  await settle();
}

/**
 * Saves the window as `<name>.png` in the screenshot directory (no-op when
 * screenshots are off). `fullPage` grows the window to fit the page or the
 * open dialog, whichever scrolls further, then puts it back.
 */
export async function shot(name: string, options: { fullPage?: boolean } = {}) {
  if (!outDir) return;
  fs.mkdirSync(outDir, { recursive: true });

  // Let entrance fades and transitions run out rather than catching them
  // half-way; the style below only guards against one that starts later.
  await settleForShot();

  // Wait for the bundled fonts.
  await browser.execute(async () => {
    const style = document.createElement("style");
    style.id = "hd-screenshot-freeze";
    style.textContent =
      // Zero-length rather than none, so an animation that fills forwards
      // (the empty collection's drawing) shows where it ends, not where it starts.
      "*, *::before, *::after { animation-duration: 0s !important; animation-delay: 0s !important; transition: none !important; caret-color: transparent !important; }";
    document.head.append(style);
    await document.fonts.ready;
  });

  let resized = false;
  if (options.fullPage) {
    const extra = await browser.execute(() => {
      let most = document.documentElement.scrollHeight - document.documentElement.clientHeight;
      for (const el of document.querySelectorAll<HTMLElement>("*")) {
        const overflowY = getComputedStyle(el).overflowY;
        if (overflowY === "auto" || overflowY === "scroll") {
          most = Math.max(most, el.scrollHeight - el.clientHeight);
        }
      }
      return most;
    });
    if (extra > 0) {
      await resizeWindow(SCREENSHOT_WINDOW.width, Math.min(SCREENSHOT_WINDOW.height + extra, 4000));
      resized = true;
    }
  }

  // The resize relays the page out: wait for it to be painted.
  await settleForShot();
  await browser.saveScreenshot(path.join(outDir, `${name}.png`));

  if (resized) {
    await resizeWindow(SCREENSHOT_WINDOW.width, SCREENSHOT_WINDOW.height);
  }
  await browser.execute(() => document.getElementById("hd-screenshot-freeze")?.remove());
}

/** Switches the colour mode through the top bar's toggle, which the
 * collection and the chooser both have. */
export async function chooseTheme(label: "Light" | "Dark") {
  await browser.execute((title: string) => {
    document.querySelector<HTMLElement>(`.hd-topbar label[title="${title}"]`)?.click();
  }, label);
  await settleForShot();
}

/**
 * Saves the app's window as `<name>.png` from the screen (the X display on
 * Linux, the desktop through `window-shot.ps1` on Windows, `screencapture` on
 * macOS), the way the user sees it. A WebDriver screenshot holds only the main
 * web view, so it leaves out 007's PDF surface, a child of the window that the
 * viewer's dialog leaves open over the PDF's place; this one has it. On Linux
 * the window is the only one on the display and is at its corner, so the
 * picture is the display's top-left SCREENSHOT_WINDOW corner; on Windows it is
 * the window's client area; on macOS it is the page's rectangle, below the
 * title bar. A no-op when screenshots are off. Wait for the surface to be
 * there first (the `preview:pdf-ready` event).
 */
export async function shotDisplay(name: string) {
  if (!outDir) return;
  if (
    process.platform !== "linux" &&
    process.platform !== "win32" &&
    process.platform !== "darwin"
  ) {
    throw new Error(
      "shotDisplay needs the X display (Linux), the Windows desktop or screencapture (macOS)",
    );
  }
  fs.mkdirSync(outDir, { recursive: true });
  await settleForShot();
  await browser.execute(async () => {
    const style = document.createElement("style");
    style.id = "hd-screenshot-freeze";
    style.textContent =
      "*, *::before, *::after { animation-duration: 0s !important; animation-delay: 0s !important; transition: none !important; caret-color: transparent !important; }";
    document.head.append(style);
    await document.fonts.ready;
  });
  // The surface paints on its own clock, not the page's.
  await browser.pause(800);
  const file = path.join(outDir, `${name}.png`);
  if (process.platform === "win32") {
    // The window's client area, the main web view's size (see window-shot.ps1).
    execFileSync(
      "powershell",
      [
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        fileURLToPath(new URL("../scripts/window-shot.ps1", import.meta.url)),
        "-Out",
        file,
      ],
      // windowsHide: no console window of its own over the app.
      { stdio: "pipe", windowsHide: true },
    );
  } else if (process.platform === "darwin") {
    // WebDriver's rectangle is the window's frame, title bar included; the
    // page fills the rest, below it (one pixel to a point on this display).
    const frame = await browser.getWindowRect();
    const view = await browser.execute(() => ({
      width: window.innerWidth,
      height: window.innerHeight,
    }));
    const left = frame.x + Math.round((frame.width - view.width) / 2);
    const top = frame.y + (frame.height - view.height);
    execFileSync("screencapture", [
      "-x",
      "-t",
      "png",
      "-R",
      `${left},${top},${view.width},${view.height}`,
      file,
    ]);
  } else {
    const { width, height } = SCREENSHOT_WINDOW;
    execFileSync("import", ["-window", "root", "-crop", `${width}x${height}+0+0`, "+repage", file]);
  }
  await browser.execute(() => document.getElementById("hd-screenshot-freeze")?.remove());
}
