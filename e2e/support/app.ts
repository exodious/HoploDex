import { execFileSync, spawn, type ChildProcess } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { browser } from "@wdio/globals";

/**
 * Starts, ends and restarts the app under test. An E2E build (the `e2e`
 * Cargo feature) carries its own WebDriver server, `tauri-plugin-wdio-webdriver`,
 * on 127.0.0.1 at `TAURI_WEBDRIVER_PORT` (#29), so WebdriverIO talks to the
 * app directly and the harness, not a driver, owns the app's process: it
 * launches the app before a session, and ending a session only makes the
 * server forget it.
 */

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

/** The Cargo profile E2E builds and drives: `e2e` (release without fat LTO,
 * so a backend change rebuilds in seconds), or `release` with
 * HOPLODEX_E2E_PROFILE=release, as a release check (DEVELOPMENT.md, "Test"). */
export const buildProfile = process.env.HOPLODEX_E2E_PROFILE || "e2e";
if (buildProfile !== "e2e" && buildProfile !== "release") {
  throw new Error(`HOPLODEX_E2E_PROFILE must be "e2e" or "release", not "${buildProfile}"`);
}

export const application = path.resolve(
  repoRoot,
  `src-tauri/target/${buildProfile}/hoplodex` + (process.platform === "win32" ? ".exe" : ""),
);

/** Where the app's WebDriver server listens. */
export const WEBDRIVER_PORT = 4445;

let app: ChildProcess | undefined;

function running(child: ChildProcess | undefined): child is ChildProcess {
  return child !== undefined && child.exitCode === null && child.signalCode === null;
}

/**
 * An app from an earlier run that was interrupted (Ctrl+C, a crash) before
 * `afterSession` ended it can still hold the WebDriver port, and the new app
 * would then fail to bind it. Clearing the port first keeps repeated or
 * interrupted runs self-healing.
 */
function clearPort(port: number) {
  if (process.platform !== "linux") return;
  try {
    const out = execFileSync("ss", ["-ltnp", `sport = :${port}`], { encoding: "utf-8" });
    for (const match of out.matchAll(/pid=(\d+)/g)) {
      try {
        process.kill(Number(match[1]), "SIGKILL");
      } catch {
        // already gone
      }
    }
  } catch {
    // ss unavailable, or no matching socket: nothing to clean up
  }
}

/**
 * Launches the app with this process's environment (the sandbox and the
 * per-spec settings `wdio.conf.ts` sets) and waits until its WebDriver
 * server answers.
 */
export async function launchApp(timeout = 30000) {
  if (running(app)) throw new Error("the app is already running");
  clearPort(WEBDRIVER_PORT);
  const child = spawn(application, [], {
    env: { ...process.env, TAURI_WEBDRIVER_PORT: String(WEBDRIVER_PORT) },
    stdio: ["ignore", "inherit", "inherit"],
  });
  app = child;
  const deadline = Date.now() + timeout;
  for (;;) {
    if (!running(child)) {
      throw new Error(`the app exited at launch (${child.exitCode ?? child.signalCode})`);
    }
    try {
      const status = await fetch(`http://127.0.0.1:${WEBDRIVER_PORT}/status`);
      if (status.ok) return;
    } catch {
      // not listening yet
    }
    if (Date.now() > deadline) throw new Error("the app's WebDriver server never answered");
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
}

/** Waits for the app's process to end, failing after `timeout` ms. */
export async function waitForAppToQuit(timeout = 15000) {
  const child = app;
  if (!running(child)) return;
  await new Promise<void>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("the app never quit")), timeout);
    child.once("exit", () => {
      clearTimeout(timer);
      resolve();
    });
  });
}

/**
 * Kills the app at once, with SIGKILL, so it runs no exit handler: a
 * faithful crash. A test of what a clean exit leaves behind uses the app's
 * own quit, or e2e/scripts/quit-cleanup.py.
 */
export async function killApp() {
  const child = app;
  app = undefined;
  if (!running(child)) return;
  const exited = new Promise((resolve) => child.once("exit", resolve));
  child.kill("SIGKILL");
  await exited;
}

/**
 * Ends the app (killing it if it is still running) and starts it again
 * against the same sandbox, with a new WebDriver session on it.
 */
export async function relaunchApp() {
  await killApp();
  await launchApp();
  // The old session's delete fails, since the new app never knew it, and
  // WebdriverIO only warns about that.
  await browser.reloadSession();
}
