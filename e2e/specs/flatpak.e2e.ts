import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { application, binaryLauncher, killApp, launchApp, type Launcher } from "../support/app";
import {
  FLATPAK_APP_ID,
  appDataDir,
  currentRun,
  flatpakLauncher,
  sandboxProcesses,
} from "../support/flatpak";
import { $, browser, expect, waitForChooser } from "../support/ui";

/**
 * The automated Flatpak check's journey (011 FR-021, 011 US5-2; 011
 * research.md §16). It runs only under `npm run test:flatpak`
 * (HOPLODEX_E2E_FLATPAK=1, scripts/flatpak/check.sh): wdio.conf.ts keeps it out
 * of every other run, installs the check variant before any worker starts,
 * gives this file's worker the run's throwaway home, buses and stand-in
 * services (support/flatpak.ts), and starts the app in its sandbox with the
 * shipped permissions plus `--share=network` for the WebDriver port. The
 * stand-ins a test needs of its own are started and stopped in that test.
 *
 * Its own setup, not another file's: it needs the container's Flatpak
 * installation and buses, so it shares nothing with the other specs
 * (DEVELOPMENT.md, "E2E files").
 */
describe("011 US5 - The Flatpak Is Tested in Its Sandbox, Away From Real Data", () => {
  before(async () => {
    await waitForChooser();
  });

  it("011 US1-1, 011 US5-2: the check variant starts in its sandbox and shows the database chooser, its window titled HoploDex", async () => {
    await expect($(".hd-chooser__title")).toExist();
    expect(await browser.getTitle()).toBe("HoploDex");
    // Inside a Flatpak sandbox: `hoplodex` running /app/bin/hoplodex, the file
    // in the sandbox's own mount namespace, which `flatpak ps` and /proc show
    // from outside.
    const app = sandboxProcesses().find(
      (candidate) => candidate.command[0] === "hoplodex" && candidate.exe === "/app/bin/hoplodex",
    );
    expect(app).toBeDefined();
  });

  it("011 SC-007: the app's data went under the run's throwaway home, not the real one", () => {
    const { home } = currentRun();
    expect(path.isAbsolute(home)).toBe(true);
    expect(path.relative(os.homedir(), home).startsWith("..")).toBe(true);
    // The sandbox made its data folder there as soon as it started...
    expect(fs.existsSync(appDataDir(home))).toBe(true);
    // ...and nothing was made in the real home for it.
    expect(fs.existsSync(path.join(os.homedir(), ".var", "app", FLATPAK_APP_ID))).toBe(false);
  });

  it("011 SC-006: a cold start reaches the chooser within a second of the same build outside the sandbox", async () => {
    // The outside build is `cargo build --profile e2e` of the same sources;
    // scripts/flatpak/check.sh builds it for this comparison.
    if (!fs.existsSync(application)) {
      throw new Error(
        `${application} is not built; scripts/flatpak/check.sh builds it for this comparison ` +
          "(011 research.md §18)",
      );
    }
    /** Milliseconds from starting the app to the chooser being on screen. */
    async function startToChooser(launcher: Launcher): Promise<number> {
      await killApp();
      const started = performance.now();
      await launchApp(launcher);
      await browser.reloadSession();
      await $(".hd-chooser__title").waitForExist({ timeout: 10000 });
      return performance.now() - started;
    }
    /**
     * What a `.deb` install pays before it starts that the `e2e` build in this
     * container doesn't: the WebKit sandbox probe (src-tauri/src/services/
     * preview/sandbox_probe.rs), which a container skips and the Flatpak runs.
     * Milliseconds for one run of it, forced on as the app does.
     */
    function probeMilliseconds(): number {
      const started = performance.now();
      const probe = spawnSync(application, ["--webkit-sandbox-probe"], {
        env: { ...process.env, WEBKIT_FORCE_SANDBOX: "1" },
        stdio: "ignore",
        timeout: 10000,
      });
      if (probe.status !== 0) {
        throw new Error(`the WebKit sandbox probe outside the sandbox exited with ${probe.status}`);
      }
      return performance.now() - started;
    }
    const flatpak = flatpakLauncher();
    // One start of each, not timed, so both find the programs and libraries
    // in the file cache; then one of each, timed.
    await startToChooser(binaryLauncher);
    await startToChooser(flatpak);
    // The probe runs before the app starts, so outside the sandbox it adds to
    // the start as a `.deb` would have it (011 research.md §18).
    const probe = probeMilliseconds();
    const outside = (await startToChooser(binaryLauncher)) + probe;
    const inside = await startToChooser(flatpak);
    console.log(
      `cold start to the chooser: Flatpak ${Math.round(inside)} ms, ` +
        `outside the sandbox ${Math.round(outside)} ms ` +
        `(including a ${Math.round(probe)} ms WebKit sandbox probe), ` +
        `difference ${Math.round(inside - outside)} ms`,
    );
    expect(inside - outside).toBeLessThanOrEqual(1000);
    // The last start was the Flatpak's, which the tests that follow use.
  });
});
