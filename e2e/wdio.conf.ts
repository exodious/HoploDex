import fs from "node:fs";
import os from "node:os";
import { fileURLToPath } from "node:url";
import { browser } from "@wdio/globals";
import { SevereServiceError } from "webdriverio";
import { SCREENSHOT_WINDOW, screenshotsEnabled } from "./support/screenshots";
import { assignWorkerPort, killApp, launchApp } from "./support/app";
import { buildApp } from "./support/build";
import { startDisplay, stopDisplay } from "./support/display";
import {
  buildAndInstallCheck,
  endFlatpakRun,
  flatpakMode,
  startFlatpakRun,
  uninstallCheck,
} from "./support/flatpak";
import {
  applySessionSettings,
  isolateAppData,
  removeSandbox,
  seedCollection,
  STORE_SHOTS_SANDBOX,
  sweepStaleSandboxes,
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
 * at most 8. Each worker runs its own app (a few hundred MB for WebKitGTK).
 * On a 24-core host 8 workers slow each spec by about a tenth against 4, and
 * the run then waits on its longest spec file (#77, step 6).
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
  return Math.max(1, Math.min(8, Math.floor(os.availableParallelism() / 2)));
}

/**
 * The longest spec files, longest first (#77, step 10). WebdriverIO hands
 * spec files to free workers in the order `specs` lists them, so with the
 * glob alone, in file name order, the long ones can start last and run alone
 * after the rest are done. Starting them first lets the short ones fill the
 * other workers. WebdriverIO keeps the patterns' order and drops a file the
 * glob names again. A name that matches no file only draws a warning from
 * WebdriverIO and the run goes on, so a renamed or removed file would leave
 * the list stale unnoticed: loading this config fails instead. Re-measure
 * with the per-spec times in issue #77 when specs are split or added.
 */
const LONGEST_FIRST = [
  "./specs/document-preview.e2e.ts",
  "./specs/databases.e2e.ts",
  "./specs/accessories-browse.e2e.ts",
  "./specs/regulated-items.e2e.ts",
  "./specs/accessories-mounting.e2e.ts",
  "./specs/accessories-dispose.e2e.ts",
  "./specs/accessories-record.e2e.ts",
  "./specs/locking.e2e.ts",
  "./specs/record-firearm.e2e.ts",
  "./specs/session-isolation.e2e.ts",
  "./specs/value-insurance.e2e.ts",
  "./specs/backups.e2e.ts",
];
const missing = LONGEST_FIRST.filter(
  (spec) => !fs.existsSync(fileURLToPath(new URL(spec, import.meta.url))),
);
if (missing.length > 0) {
  throw new Error(
    `LONGEST_FIRST in e2e/wdio.conf.ts names spec files that don't exist: ${missing}`,
  );
}

/**
 * The Flatpak check (HOPLODEX_E2E_FLATPAK=1, by `npm run test:flatpak`; 011
 * research.md §16) runs one spec file against the check variant installed in
 * its sandbox, with one worker, and no other run includes that file: it needs
 * the container's Flatpak installation and buses, and builds its own app.
 */
const FLATPAK_SPEC = "./specs/flatpak.e2e.ts";
const flatpakRun = flatpakMode();
/** Whether this process installed the check variant, and so uninstalls it. */
let installedCheck = false;

export const config: WebdriverIO.Config = {
  runner: "local",
  specs: flatpakRun ? [FLATPAK_SPEC] : [...LONGEST_FIRST, "./specs/**/*.e2e.ts"],
  exclude: flatpakRun ? [] : [FLATPAK_SPEC],
  maxInstances: flatpakRun ? 1 : workerCount(),
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
  // How often waitUntil, waitForExist and the rest ask again (default 100
  // ms). A condition that holds after 5 ms is found after 20, not 100: over
  // thousands of waits that adds up (#77, step 10). The calls are local.
  waitforInterval: 20,

  // One build before any worker starts, so workers neither build nor wait on
  // cargo's lock. A failed build ends the run with no spec run and a non-zero
  // exit: WebdriverIO stops for an instance of webdriverio's
  // SevereServiceError and only logs anything else this hook throws, even an
  // Error named so (#92).
  onPrepare: async () => {
    // Sandboxes an interrupted run left in the temp folder (#111).
    await sweepStaleSandboxes();
    if (flatpakRun) {
      // The check variant is built inside flatpak-builder from the derived
      // manifest, so the host's `e2e` build is not made here. It is installed
      // before any worker starts, because a worker's session needs the app.
      try {
        installedCheck = buildAndInstallCheck().installedHere;
      } catch (error) {
        console.error(error);
        throw new SevereServiceError("building and installing the Flatpak check variant failed");
      }
      return;
    }
    if (!buildApp()) throw new SevereServiceError("building the app for E2E failed");
  },

  // A worker that ends without reaching afterSession (WebdriverIO gives up
  // on a session whose app died, say) leaves its sandbox; every worker is gone
  // by now, so the run's own leftovers are swept here, and an interrupted
  // run's at the start of the next (#111).
  onComplete: async () => {
    await sweepStaleSandboxes(undefined, undefined, 15_000);
    if (installedCheck) uninstallCheck();
  },

  beforeSession: async (config, _capabilities, specs, cid) => {
    // The store shots (`npm run screenshots:store`, 011 FR-017) run in a
    // sandbox at the same path every time, which the chooser's shot shows.
    const storeShots = specs.some((spec) => spec.endsWith("/e2e/screenshots/store.e2e.ts"));
    const sandbox = isolateAppData(storeShots ? { fixedDir: STORE_SHOTS_SANDBOX } : {});
    // The Flatpak check's home, buses and stand-in services; the app is started
    // by support/app.ts's default launcher, which is the Flatpak's under
    // HOPLODEX_E2E_FLATPAK=1.
    if (flatpakRun) await startFlatpakRun({ sandbox });
    // WebdriverIO connects with this same config object once the hook
    // returns, so the session goes to this worker's app.
    config.port = assignWorkerPort(cid);
    // Only a spec named here gets a setting for its whole session; a spec that
    // needs another sets the variable itself and calls relaunchApp()
    // (applySessionSettings in support/sandbox.ts), as document-preview does
    // for PDF preview off. The one named: the locking spec, whose idle minute
    // lasts 3 s so its test doesn't wait a real one.
    applySessionSettings({
      idleMinuteSeconds: specs.some((spec) => spec.endsWith("/locking.e2e.ts")) ? 3 : undefined,
    });
    // The screenshot walk, 007's preview specs (`document-preview`,
    // `document-open-elsewhere` and `document-open-setting`, which open the seed's Glock and its documents) and 008's session
    // isolation spec (which switches between the seed's two databases) want
    // the realistic collection.
    if (
      specs.some(
        (spec) =>
          spec.endsWith("/e2e/screenshots/screens.e2e.ts") ||
          storeShots ||
          spec.endsWith("/e2e/specs/document-preview.e2e.ts") ||
          spec.endsWith("/e2e/specs/document-open-elsewhere.e2e.ts") ||
          spec.endsWith("/e2e/specs/document-open-setting.e2e.ts") ||
          spec.endsWith("/e2e/specs/session-isolation.e2e.ts"),
      )
    ) {
      seedCollection({ fixedTimes: storeShots });
    }
    await startDisplay();
    await launchApp();
  },

  afterSession: async () => {
    await killApp();
    if (flatpakRun) await endFlatpakRun();
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
