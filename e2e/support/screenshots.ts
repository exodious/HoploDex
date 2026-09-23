import fs from "node:fs";
import path from "node:path";
import { browser } from "@wdio/globals";

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

export function screenshotsEnabled(): boolean {
  return Boolean(outDir);
}

/**
 * Saves the window as `<name>.png` in the screenshot directory (no-op when
 * screenshots are off). `fullPage` grows the window to fit the page or the
 * open dialog, whichever scrolls further, then puts it back.
 */
export async function shot(name: string, options: { fullPage?: boolean } = {}) {
  if (!outDir) return;
  fs.mkdirSync(outDir, { recursive: true });

  // Finish entrance fades and transitions now rather than catching them
  // half-way, and wait for the bundled fonts.
  await browser.execute(async () => {
    const style = document.createElement("style");
    style.id = "hd-screenshot-freeze";
    style.textContent =
      "*, *::before, *::after { animation: none !important; transition: none !important; caret-color: transparent !important; }";
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
      await browser.setWindowSize(
        SCREENSHOT_WINDOW.width,
        Math.min(SCREENSHOT_WINDOW.height + extra, 4000),
      );
      resized = true;
    }
  }

  await browser.pause(300);
  await browser.saveScreenshot(path.join(outDir, `${name}.png`));

  if (resized) {
    await browser.setWindowSize(SCREENSHOT_WINDOW.width, SCREENSHOT_WINDOW.height);
    await browser.pause(300);
  }
  await browser.execute(() => document.getElementById("hd-screenshot-freeze")?.remove());
}
