import { execFileSync, spawn, type ChildProcess } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { browser } from "@wdio/globals";

/**
 * Starts, ends and restarts the app under test. An E2E build (the `e2e`
 * Cargo feature) carries its own WebDriver server, `tauri-plugin-wdio-webdriver`,
 * on 127.0.0.1 at `TAURI_WEBDRIVER_PORT` (#29), a port per worker, so
 * WebdriverIO talks to the app directly and the harness, not a driver, owns the app's process: it
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

/** The first worker's WebDriver port; each later worker of a run takes the
 * next one up. */
const FIRST_PORT = 4445;

/** Where this worker's app's WebDriver server listens. */
let port = FIRST_PORT;

/**
 * Gives this worker a WebDriver port of its own, so the apps of workers
 * running side by side don't collide, and returns it. `cid` is WebdriverIO's
 * worker id, "<capability>-<worker>"; a run numbers its workers from 0 and
 * never reuses a number, so the port is the first one plus the worker's.
 */
export function assignWorkerPort(cid: string): number {
  const worker = Number(cid.split("-")[1]);
  if (!Number.isInteger(worker) || worker < 0) throw new Error(`unexpected worker id "${cid}"`);
  port = FIRST_PORT + worker;
  return port;
}

let app: ChildProcess | undefined;

function exited(child: ChildProcess): boolean {
  return child.exitCode !== null || child.signalCode !== null;
}

function running(child: ChildProcess | undefined): child is ChildProcess {
  return child !== undefined && !exited(child);
}

/**
 * An app from an earlier run that was interrupted (Ctrl+C, a crash) before
 * `afterSession` ended it can still hold this worker's WebDriver port, and the
 * new app would then fail to bind it. Clearing the port first keeps repeated or
 * interrupted runs self-healing.
 */
function clearPort(taken: number) {
  let pids: string[];
  try {
    if (process.platform === "linux") {
      const out = execFileSync("ss", ["-ltnp", `sport = :${taken}`], { encoding: "utf-8" });
      pids = [...out.matchAll(/pid=(\d+)/g)].map((match) => match[1]);
    } else if (process.platform === "darwin") {
      const out = execFileSync("lsof", ["-t", `-iTCP:${taken}`, "-sTCP:LISTEN"], {
        encoding: "utf-8",
      });
      pids = out.split(/\s+/).filter(Boolean);
    } else {
      return;
    }
  } catch {
    // ss or lsof unavailable, or no matching socket: nothing to clean up
    return;
  }
  for (const pid of pids) {
    try {
      process.kill(Number(pid), "SIGKILL");
    } catch {
      // already gone
    }
  }
}

/** The app's own environment. On macOS that includes the sandbox's HOME
 * (wdio.conf.ts), which only the app gets; a launch without one would use
 * the developer's real Application Support folder, so it refuses. */
function appEnv(): NodeJS.ProcessEnv {
  const env: NodeJS.ProcessEnv = { ...process.env, TAURI_WEBDRIVER_PORT: String(port) };
  if (process.platform === "darwin") {
    const home = process.env.HOPLODEX_E2E_HOME;
    if (!home) throw new Error("HOPLODEX_E2E_HOME is unset: the app would see the real home");
    env.HOME = home;
    env.CFFIXED_USER_HOME = home;
  }
  return env;
}

/**
 * Launches the app with this process's environment (the sandbox, the
 * worker's display and the per-spec settings `wdio.conf.ts` sets) and waits
 * until its WebDriver server answers on the worker's port.
 */
export async function launchApp(timeout = 30000) {
  if (running(app)) throw new Error("the app is already running");
  clearPort(port);
  const child = spawn(application, [], {
    env: appEnv(),
    stdio: ["ignore", "inherit", "inherit"],
  });
  app = child;
  const deadline = Date.now() + timeout;
  for (;;) {
    if (exited(child)) {
      throw new Error(`the app exited at launch (${child.exitCode ?? child.signalCode})`);
    }
    try {
      const status = await fetch(`http://127.0.0.1:${port}/status`);
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
 * against the same sandbox, display and port, with a new WebDriver session
 * on it.
 */
export async function relaunchApp() {
  await killApp();
  await launchApp();
  // The old session's delete fails, since the new app never knew it, and
  // WebdriverIO only warns about that.
  await browser.reloadSession();
}
