import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { repoRoot, seedBinary } from "./build";

/**
 * The throwaway sandbox every E2E app runs in, shared by the test runner
 * (wdio.conf.ts, one sandbox per spec file) and the `wdio session` launcher
 * (e2e/session.ts), so the two isolate identically (constitution 1.2.0,
 * 003 research.md §21). Nothing here reads or writes the developer's real
 * databases, config directory or keyring; DEVELOPMENT.md, "Test isolation",
 * says what that covers.
 *
 * The functions set variables on this process's environment, which the app,
 * launched from here (support/app.ts), inherits.
 */

/**
 * The current sandbox's directory. It lives on `globalThis`, not in a module
 * variable, for the reason support/app.ts gives: WebdriverIO loads
 * `wdio.conf.ts` apart from the spec files, so this module is evaluated twice
 * in a worker, and a spec that reseeds (`locking`) must see the sandbox the config's
 * copy made.
 */
interface SandboxState {
  dir: string | undefined;
}
/** The name every sandbox directory starts with, in the system's temp folder
 * (`fs.mkdtempSync` adds six random letters and digits). */
export const SANDBOX_PREFIX = "hoplodex-e2e-";

/** The file the harness writes into a sandbox when it makes it: the process
 * id of the process that owns it. Only a directory that has one is a
 * sandbox for {@link sweepStaleSandboxes}; the prefix alone is not enough
 * (`document-preview` writes plain files named `hoplodex-e2e-screen-<pid>.png` into the same
 * folder). {@link removeSandbox} deletes it last. */
export const OWNER_MARKER = ".hoplodex-e2e-owner";

const stateKey = Symbol.for("io.github.exodious.HoploDex.e2e.sandbox");
const globals = globalThis as { [stateKey]?: SandboxState };
const state: SandboxState = (globals[stateKey] ??= { dir: undefined });

/** The current sandbox's directory, if there is one. */
export function sandboxPath(): string | undefined {
  return state.dir;
}

/** Where the documents folder, and so the suggested location for a new
 * database, points during a session (003 research.md §19). */
function writeUserDirs(configDir: string, documents: string) {
  fs.mkdirSync(configDir, { recursive: true });
  fs.writeFileSync(path.join(configDir, "user-dirs.dirs"), `XDG_DOCUMENTS_DIR="${documents}"\n`);
}

/**
 * Mesa's shader cache on Linux, kept across sandboxes and runs (in
 * node_modules, which the dev container keeps in a volume). Rendering in
 * software (the dev container's Xvfb has no GPU), Mesa compiles the shader
 * for a dialog's box-shadow the first time one is drawn: about a second in
 * which the window paints nothing, so `settle()` waits it out. With each
 * sandbox's own empty cache, every spec paid it again (#102). It holds
 * compiled shaders, nothing of the app's or the developer's.
 */
export const SHADER_CACHE = path.join(repoRoot, "node_modules", ".cache", "hoplodex-e2e-shaders");

/**
 * Points the app at a throwaway data, config, cache and documents directory
 * for this session, so each spec starts at a first run and E2E runs never
 * see the developer's real databases, their recent list or their Documents
 * folder (constitution 1.2.0, 003 research.md §21). A `user-dirs.dirs` in the
 * scratch config directory moves the documents folder into the sandbox too,
 * so even accepting the suggested location for a new database stays inside
 * it.
 *
 * Specs read HOPLODEX_E2E_DOCUMENTS to type locations inside the sandbox.
 * Returns the sandbox's directory.
 */
export function isolateAppData(): string {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), SANDBOX_PREFIX));
  state.dir = root;
  // Says whose sandbox this is, so a later run can tell it from one still in
  // use and sweep it if this process never got to removeSandbox() (#111).
  fs.writeFileSync(path.join(root, OWNER_MARKER), `${process.pid}\n`);
  const [data, cache, config, documents] = ["data", "cache", "config", "documents"].map((name) => {
    const dir = path.join(root, name);
    fs.mkdirSync(dir, { recursive: true });
    return dir;
  });
  // An E2E build takes its config and cache directories (and the app
  // identifier's folder in each) and its documents folder from these, on
  // every platform, and won't start without them (src-tauri/src/app_dirs.rs):
  // Windows' known folders ignore APPDATA and the like (#27).
  process.env.HOPLODEX_E2E_CONFIG_HOME = config;
  process.env.HOPLODEX_E2E_CACHE_HOME = cache;
  process.env.HOPLODEX_E2E_DOCUMENTS = documents;
  // The E2E build's in-memory keyring is kept here between launches, so a
  // remembered passphrase outlives a relaunch (003 research.md §10).
  process.env.HOPLODEX_E2E_KEYRING_FILE = path.join(root, "keyring.json");
  // An E2E build never hands an opened document to the OS, which would start
  // a real viewer: it writes the copy's path here instead (open_document, #27).
  process.env.HOPLODEX_E2E_OPENED_LOG = path.join(root, "opened.log");
  // Nor can a WebDriver click the native confirmation that comes before it
  // (007 research.md §16): an E2E build appends each request's title here and
  // answers from HOPLODEX_E2E_CONSENT, set per session below.
  process.env.HOPLODEX_E2E_CONSENT_LOG = path.join(root, "consent.log");

  if (process.platform === "linux") {
    process.env.XDG_DATA_HOME = data;
    process.env.XDG_CACHE_HOME = cache;
    process.env.XDG_CONFIG_HOME = config;
    writeUserDirs(config, documents);
    fs.mkdirSync(SHADER_CACHE, { recursive: true });
    process.env.MESA_SHADER_CACHE_DIR = SHADER_CACHE;
  } else if (process.platform === "darwin") {
    // macOS ignores XDG_*: Tauri builds every directory (Application Support,
    // Caches, Documents) from HOME, so the app gets a home of its own inside
    // the sandbox (#28). Only the app: cargo, run from here, keeps the real one.
    const home = path.join(root, "home");
    const homeDocuments = path.join(home, "Documents");
    fs.mkdirSync(path.join(home, "Library", "Application Support"), { recursive: true });
    fs.mkdirSync(homeDocuments, { recursive: true });
    process.env.HOPLODEX_E2E_HOME = home;
    process.env.HOPLODEX_E2E_DOCUMENTS = homeDocuments;
  }
  return root;
}

/** The per-session settings a spec can change. `true` or a number turns the
 * setting on; whatever the developer's own environment holds never reaches
 * the app. */
export interface SessionSettings {
  /** How many seconds the idle lock's minute lasts. */
  idleMinuteSeconds?: number;
  /** How many seconds each phase of an upgrade (the backup before it, and the
   * upgrade itself) is held, so a screenshot can catch it (009 research.md
   * §14; an E2E build only). */
  upgradePhaseSeconds?: number;
}

/**
 * Sets the environment variables for a session's app settings from `settings`,
 * and clears the ones it doesn't turn on. Called once per session, after
 * `isolateAppData`.
 *
 * The native confirmation before a document goes to another app is answered
 * "Open in another app" unless a spec says otherwise: it sets
 * HOPLODEX_E2E_CONSENT (`open` or `cancel`) itself and calls relaunchApp(),
 * which launches with what it set, and puts `open` back when it is done.
 */
export function applySessionSettings(settings: SessionSettings = {}) {
  delete process.env.HOPLODEX_E2E_SEED_PASSPHRASE;
  // PDF preview is on; a spec that wants it off (a viewer that can't be used
  // safely, 007 FR-003a) sets HOPLODEX_E2E_PDF_PREVIEW=off and relaunches.
  delete process.env.HOPLODEX_E2E_PDF_PREVIEW;
  process.env.HOPLODEX_E2E_CONSENT = "open";
  if (settings.idleMinuteSeconds !== undefined) {
    process.env.HOPLODEX_E2E_IDLE_MINUTE_SECONDS = String(settings.idleMinuteSeconds);
  } else {
    delete process.env.HOPLODEX_E2E_IDLE_MINUTE_SECONDS;
  }
  if (settings.upgradePhaseSeconds !== undefined) {
    process.env.HOPLODEX_E2E_UPGRADE_PHASE_SECONDS = String(settings.upgradePhaseSeconds);
  } else {
    delete process.env.HOPLODEX_E2E_UPGRADE_PHASE_SECONDS;
  }
}

/** The database 0.1.0 made, kept in the repository (009 research.md §12),
 * which the upgrade journeys copy. */
export const RELEASE_FIXTURE_0_1_0 = path.join(
  repoRoot,
  "src-tauri",
  "tests",
  "fixtures",
  "releases",
  "0.1.0.hoplodex",
);

/** The SHA-256 of the file at `file`, in hex. */
export function sha256Of(file: string): string {
  return createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

/**
 * Copies the stored 0.1.0 database (`source`, by default
 * {@link RELEASE_FIXTURE_0_1_0}) into `folder`, which must be inside the
 * current sandbox, as `<name>.hoplodex` (009 US3), and returns the copy's path
 * and SHA-256. The folder is made if it isn't there. Nothing is read from or
 * written outside the sandbox but the source, and an existing file at the
 * name is never replaced.
 */
export function copyReleaseFixture(
  folder: string,
  name: string,
  source: string = RELEASE_FIXTURE_0_1_0,
): { path: string; sha256: string } {
  if (!state.dir) throw new Error("copyReleaseFixture() needs isolateAppData() first");
  if (name === "" || name !== path.basename(name) || name === "." || name === "..") {
    throw new Error(`"${name}" is not a plain file name`);
  }
  const inside = path.relative(state.dir, path.resolve(folder));
  if (inside.startsWith("..") || path.isAbsolute(inside)) {
    throw new Error(`${folder} is not inside the sandbox`);
  }
  fs.mkdirSync(folder, { recursive: true });
  const target = path.join(folder, `${name}.hoplodex`);
  fs.copyFileSync(source, target, fs.constants.COPYFILE_EXCL);
  return { path: target, sha256: sha256Of(target) };
}

/** Runs the human-testing seed `buildApp` built. */
function runSeed(args: string[], stdio: "inherit" | "pipe") {
  return spawnSync(seedBinary, args, {
    cwd: repoRoot,
    stdio: ["ignore", stdio, "inherit"],
    encoding: "utf-8",
  });
}

/**
 * Seeds the human-testing databases for the screenshot walk in
 * e2e/screenshots/ and for 007's preview spec, which want a realistic collection rather than an empty
 * one. The seed writes only into a directory it creates, so it gets a new
 * `seed` folder inside the sandbox, with its databases, backups and a
 * `machine.json` whose recent list names them. The app's config directory
 * then moves there, keeping the sandbox's documents folder, and the specs
 * read the seed's fixed passphrase from HOPLODEX_E2E_SEED_PASSPHRASE.
 * Returns that passphrase.
 *
 * Shared collection's pending changes are kept by this build; with
 * `pendingFrom` they were kept by that version of HoploDex, in a form this
 * build can't resume (008 FR-016).
 */
export function seedCollection(options: { pendingFrom?: string } = {}): string {
  if (!state.dir) throw new Error("seedCollection() needs isolateAppData() first");
  const dir = path.join(state.dir, "seed");
  const seedArgs = ["--dir", dir];
  if (options.pendingFrom) seedArgs.push("--pending-from", options.pendingFrom);
  if (runSeed(seedArgs, "inherit").status !== 0) {
    throw new Error("seeding the screenshot collection failed");
  }
  const printed = runSeed(["--print-passphrase"], "pipe");
  if (printed.status !== 0) throw new Error("reading the seed's passphrase failed");
  const passphrase = printed.stdout.trim();
  process.env.HOPLODEX_E2E_SEED_PASSPHRASE = passphrase;

  process.env.HOPLODEX_E2E_CONFIG_HOME = path.join(dir, "config");
  return passphrase;
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/**
 * Removes the directory `root`, retrying for up to `timeoutMs` ms, and
 * returns whether it is gone. The app's processes may still be shutting down
 * (on Windows, WebView2's browser process keeps its user data folder open for
 * a moment after the app that started it is gone, and `rmSync` then throws
 * EPERM or EBUSY rather than returning), and write cache files as they go.
 *
 * The owner marker goes last, so a directory that can't be removed yet is
 * still one a later run knows to sweep (it is put back if the final step
 * fails after it went).
 */
async function removeDirectory(root: string, timeoutMs: number): Promise<boolean> {
  const marker = path.join(root, OWNER_MARKER);
  const deadline = Date.now() + timeoutMs;
  let lastError: unknown;
  for (let delay = 50; ; delay = Math.min(delay * 2, 1000)) {
    try {
      if (!fs.existsSync(root)) return true;
      for (const name of fs.readdirSync(root)) {
        if (name !== OWNER_MARKER)
          fs.rmSync(path.join(root, name), { recursive: true, force: true });
      }
      fs.rmSync(marker, { force: true });
      try {
        fs.rmdirSync(root);
      } catch (error) {
        if (!fs.existsSync(marker) && fs.existsSync(root)) {
          fs.writeFileSync(marker, `${process.pid}\n`);
        }
        throw error;
      }
      return true;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === "ENOENT") return true;
      lastError = error;
    }
    if (Date.now() >= deadline) break;
    await sleep(delay);
  }
  console.warn(
    `Could not remove the E2E sandbox ${root} after ${timeoutMs} ms; the next run will sweep it ` +
      `once this process has exited (${lastError instanceof Error ? lastError.message : lastError})`,
  );
  return false;
}

/** How long {@link removeSandbox} keeps trying. A tree-killed app's
 * WebView2 releases its folder within a second; this is for a slow disk or a
 * virus scanner, and it is only spent when something holds the folder. */
const REMOVE_TIMEOUT_MS = 30_000;

/** Removes the sandbox. It tries again for up to 30 s, and logs a sandbox it
 * could not remove rather than dropping it silently. */
export async function removeSandbox() {
  const root = state.dir;
  if (!root) return;
  state.dir = undefined;
  await removeDirectory(root, REMOVE_TIMEOUT_MS);
}

/** Whether a process with this id is running. Not being allowed to signal it
 * (EPERM) means it is. */
export function processRunning(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return (error as NodeJS.ErrnoException).code !== "ESRCH";
  }
}

/** The owner marker's process id, or undefined if `dir` has no readable one. */
function ownerOf(dir: string): number | undefined {
  try {
    const marker = path.join(dir, OWNER_MARKER);
    if (!fs.lstatSync(marker).isFile()) return undefined;
    const text = fs.readFileSync(marker, "utf-8").trim();
    return /^\d{1,10}$/.test(text) && Number(text) > 0 ? Number(text) : undefined;
  } catch {
    return undefined;
  }
}

/**
 * The sandboxes in `parent` that an earlier run left behind: a real
 * directory (not a link) directly in it, named {@link SANDBOX_PREFIX} and six
 * letters or digits, that holds an owner marker (a plain file) whose process
 * is no longer running. Anything else is not ours and is never listed: a
 * file, a folder without a marker, a sandbox whose owner still runs (another
 * worker of this run, a `npm run session` or another run entirely), and one
 * this process owns.
 */
export function findStaleSandboxes(
  parent: string = os.tmpdir(),
  isRunning: (pid: number) => boolean = processRunning,
): string[] {
  const name = new RegExp(`^${SANDBOX_PREFIX}[A-Za-z0-9]{6}$`);
  const stale: string[] = [];
  for (const entry of fs.readdirSync(parent, { withFileTypes: true })) {
    if (!entry.isDirectory() || !name.test(entry.name)) continue;
    const dir = path.join(parent, entry.name);
    const owner = ownerOf(dir);
    if (owner === undefined || owner === process.pid || isRunning(owner)) continue;
    stale.push(dir);
  }
  return stale;
}

/**
 * Removes the sandboxes earlier, interrupted runs left in the system's temp
 * folder (a Ctrl+C, a crashed worker or a closed session never reaches
 * {@link removeSandbox}), at the start of a run (#111). It logs each one it
 * removes and each one it could not (something still holds it); it never
 * throws, so a run isn't stopped by an old folder. Safe beside other runs:
 * see {@link findStaleSandboxes}.
 */
export async function sweepStaleSandboxes(
  parent: string = os.tmpdir(),
  isRunning: (pid: number) => boolean = processRunning,
  timeoutMs = 5000,
): Promise<{ removed: string[]; kept: string[] }> {
  const removed: string[] = [];
  const kept: string[] = [];
  let stale: string[] = [];
  try {
    stale = findStaleSandboxes(parent, isRunning);
  } catch (error) {
    console.warn(`Could not look for old E2E sandboxes in ${parent}: ${String(error)}`);
  }
  for (const dir of stale) {
    ((await removeDirectory(dir, timeoutMs)) ? removed : kept).push(dir);
  }
  if (removed.length > 0) {
    console.log(`Removed ${removed.length} E2E sandbox(es) an earlier run left behind.`);
  }
  return { removed, kept };
}
