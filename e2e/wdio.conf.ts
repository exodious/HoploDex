import os from "node:os";
import { browser } from "@wdio/globals";
import { SCREENSHOT_WINDOW, screenshotsEnabled } from "./support/screenshots";
import { assignWorkerPort, killApp, launchApp } from "./support/app";
import { buildApp } from "./support/build";
import { startDisplay, stopDisplay } from "./support/display";
import {
  applySessionSettings,
  isolateAppData,
  removeSandbox,
  seedCollection,
} from "./support/sandbox";

// The E2E build (support/build.ts) is `cargo build --profile e2e --features
// custom-protocol,e2e`; `onPrepare` builds it, and the human-testing seed,
// once before any worker starts, and support/app.ts launches the binary. Each
// session's sandbox is support/sandbox.ts's, which the `wdio session`
// launcher (session.ts) uses too.
//
// Spec files run in parallel workers (HOPLODEX_E2E_WORKERS, see
// `workerCount`). Each worker is its own process, with its own sandbox,
// WebDriver port and, on Linux, X display (support/display.ts).

/**
 * How many spec files run at once: HOPLODEX_E2E_WORKERS, or half the CPUs,
 * at most 4. Each worker runs its own app (a few hundred MB for WebKitGTK),
 * and more than 4 gains little while making every step slower.
 * HOPLODEX_E2E_WORKERS=1 runs one spec file at a time, for debugging.
 */
function workerCount(): number {
  const set = process.env.HOPLODEX_E2E_WORKERS;
  if (set) {
    const count = Number(set);
    if (!Number.isInteger(count) || count < 1) {
      throw new Error(`HOPLODEX_E2E_WORKERS must be a whole number of at least 1, not "${set}"`);
    }
    return count;
  }
  return Math.max(1, Math.min(4, Math.floor(os.availableParallelism() / 2)));
}

export const config: WebdriverIO.Config = {
  runner: "local",
  specs: ["./specs/**/*.e2e.ts"],
  maxInstances: workerCount(),
  // Each worker starts its own display in beforeSession (Linux), so the
  // testrunner mustn't start the one shared display it otherwise starts for
  // the whole run when DISPLAY is unset.
  displayServerEnabled: false,
  capabilities: [
    {
      // The embedded server ignores capabilities; the name is what the spec
      // reporter prints ("RUNNING in wry").
      browserName: "wry",
      // WebdriverIO defaults to requesting a Bidi session (webSocketUrl:
      // true) unless this flag opts back into plain WebDriver classic, the
      // only protocol the embedded server speaks.
      "wdio:enforceWebDriverClassic": true,
    },
  ],
  reporters: ["spec"],
  framework: "mocha",
  mochaOpts: {
    ui: "bdd",
    timeout: 60000,
  },
  hostname: "127.0.0.1",
  path: "/",

  // One build before any worker starts, so workers neither build nor wait on
  // cargo's lock. A failed build ends the run (WebdriverIO only logs other
  // errors from this hook).
  onPrepare: () => {
    if (!buildApp()) {
      const error = new Error("building the app for E2E failed");
      error.name = "SevereServiceError";
      throw error;
    }
  },

  beforeSession: async (config, _capabilities, specs, cid) => {
    isolateAppData();
    // WebdriverIO connects with this same config object once the hook
    // returns, so the session goes to this worker's app.
    config.port = assignWorkerPort(cid);
    // Only a spec named so gets a setting for its whole session; a spec that
    // needs more sets the variable itself and calls relaunchApp()
    // (applySessionSettings in support/sandbox.ts). Specs that are named:
    // `-no-keyring` (a computer with no keyring service, FR-019), `-pdf-off`
    // (a viewer that can't be used safely, 007 FR-003a) and the locking
    // spec, whose idle minute lasts 3 s so its test doesn't wait a real one.
    applySessionSettings({
      noKeyring: specs.some((spec) => spec.endsWith("-no-keyring.e2e.ts")),
      pdfOff: specs.some((spec) => spec.endsWith("-pdf-off.e2e.ts")),
      idleMinuteSeconds: specs.some((spec) => spec.endsWith("/us9-locking.e2e.ts")) ? 3 : undefined,
    });
    // The screenshot walk and 007's preview spec (which opens the seed's
    // Glock and its documents) want the realistic collection.
    if (
      specs.some(
        (spec) =>
          spec.endsWith("/e2e/screenshots/screens.e2e.ts") ||
          spec.endsWith("/e2e/specs/us13-document-preview.e2e.ts"),
      )
    ) {
      seedCollection();
    }
    await startDisplay();
    await launchApp();
  },

  afterSession: async () => {
    await killApp();
    await stopDisplay();
    await removeSandbox();
  },

  before: async () => {
    // The webview's first paint can lag slightly behind session creation;
    // without this, the very first interaction can hit a not-yet-painted
    // window and fail with "element click intercepted".
    await browser.waitUntil(
      async () => (await browser.execute(() => document.readyState)) === "complete",
      { timeout: 10000, timeoutMsg: "app document never reached readyState=complete" },
    );
    await browser.pause(500);
    if (screenshotsEnabled()) {
      await browser.setWindowSize(SCREENSHOT_WINDOW.width, SCREENSHOT_WINDOW.height);
      await browser.pause(300);
    }
  },
};
