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

/**
 * This worker's port and app. They live on `globalThis`, not in module
 * variables, because WebdriverIO 10 loads `wdio.conf.ts` (and everything it
 * imports) apart from the spec files, so this module is evaluated twice in a
 * worker: the config's copy launches the app and gives the worker its port,
 * and a spec's copy, which `relaunchApp()` belongs to, would otherwise start
 * with the first port and no app. It would then clear the first worker's port
 * (killing that worker's app), start a second app there, and leave the first
 * one running while the new session still went to the worker's own port (#86).
 * One state per process is what every copy needs.
 */
interface WorkerApp {
  /** Where this worker's app's WebDriver server listens. */
  port: number;
  app: ChildProcess | undefined;
}

const stateKey = Symbol.for("io.github.exodious.HoploDex.e2e.workerApp");
const globals = globalThis as { [stateKey]?: WorkerApp };
const state: WorkerApp = (globals[stateKey] ??= { port: FIRST_PORT, app: undefined });

/**
 * Gives this worker a WebDriver port of its own, so the apps of workers
 * running side by side don't collide, and returns it. `cid` is WebdriverIO's
 * worker id, "<capability>-<worker>"; a run numbers its workers from 0 and
 * never reuses a number, so the port is the first one plus the worker's.
 */
export function assignWorkerPort(cid: string): number {
  const worker = Number(cid.split("-")[1]);
  if (!Number.isInteger(worker) || worker < 0) throw new Error(`unexpected worker id "${cid}"`);
  return setPort(FIRST_PORT + worker);
}

/** Sets the port the app's WebDriver server will listen on, and returns it.
 * The `wdio session` launcher (session.ts) picks one clear of the workers'. */
export function setPort(next: number): number {
  state.port = next;
  return state.port;
}

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
    } else if (process.platform === "win32") {
      // Proto, local address, foreign address, state, PID. A listening row's
      // foreign address is a zero one; the state's name is in the system's
      // language.
      const out = execFileSync("netstat", ["-ano", "-p", "TCP"], { encoding: "utf-8" });
      pids = out
        .split(/\r?\n/)
        .map((line) => line.trim().split(/\s+/))
        .filter(([, local, foreign]) => local?.endsWith(`:${taken}`) && /:0$/.test(foreign ?? ""))
        .map((columns) => columns[columns.length - 1])
        .filter((pid) => pid !== "0");
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
 * (support/sandbox.ts), which only the app gets; a launch without one would use
 * the developer's real Application Support folder, so it refuses. */
function appEnv(): NodeJS.ProcessEnv {
  const env: NodeJS.ProcessEnv = { ...process.env, TAURI_WEBDRIVER_PORT: String(state.port) };
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
  if (running(state.app)) throw new Error("the app is already running");
  clearPort(state.port);
  const child = spawn(application, [], {
    env: appEnv(),
    stdio: ["ignore", "inherit", "inherit"],
  });
  state.app = child;
  const deadline = Date.now() + timeout;
  for (;;) {
    if (exited(child)) {
      throw new Error(`the app exited at launch (${child.exitCode ?? child.signalCode})`);
    }
    try {
      const status = await fetch(`http://127.0.0.1:${state.port}/status`);
      if (status.ok) return;
    } catch {
      // not listening yet
    }
    if (Date.now() > deadline) throw new Error("the app's WebDriver server never answered");
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
}

/** The process id of this worker's app, so a capture of the screen finds this
 * worker's window and not another worker's (#88). */
export function appPid(): number {
  const child = state.app;
  if (!running(child) || child.pid === undefined) throw new Error("the app isn't running");
  return child.pid;
}

/** Waits for the app's process to end, failing after `timeout` ms. */
export async function waitForAppToQuit(timeout = 15000) {
  const child = state.app;
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
  const child = state.app;
  state.app = undefined;
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
