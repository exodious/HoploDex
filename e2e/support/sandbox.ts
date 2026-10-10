import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { repoRoot, seedBinary } from "./build";

/**
 * The throwaway sandbox every E2E app runs in, shared by the test runner
 * (wdio.conf.ts, one sandbox per spec file) and the `wdio session` launcher
 * (e2e/session.ts), so the two isolate identically (constitution 1.2.0,
 * research.md §21). Nothing here reads or writes the developer's real
 * databases, config directory or keyring; DEVELOPMENT.md, "Test isolation",
 * says what that covers.
 *
 * The functions set variables on this process's environment, which the app,
 * launched from here (support/app.ts), inherits.
 */

let sandbox: string | undefined;

/** The current sandbox's directory, if there is one. */
export function sandboxPath(): string | undefined {
  return sandbox;
}

/** Where the documents folder, and so the suggested location for a new
 * database, points during a session (research.md §19). */
function writeUserDirs(configDir: string, documents: string) {
  fs.mkdirSync(configDir, { recursive: true });
  fs.writeFileSync(path.join(configDir, "user-dirs.dirs"), `XDG_DOCUMENTS_DIR="${documents}"\n`);
}

/**
 * Points the app at a throwaway data, config, cache and documents directory
 * for this session, so each spec starts at a first run and E2E runs never
 * see the developer's real databases, their recent list or their Documents
 * folder (constitution 1.2.0, research.md §21). A `user-dirs.dirs` in the
 * scratch config directory moves the documents folder into the sandbox too,
 * so even accepting the suggested location for a new database stays inside
 * it.
 *
 * Specs read HOPLODEX_E2E_DOCUMENTS to type locations inside the sandbox.
 * Returns the sandbox's directory.
 */
export function isolateAppData(): string {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "hoplodex-e2e-"));
  sandbox = root;
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
  // remembered passphrase outlives a relaunch (research.md §10).
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
  /** A computer with no keyring service (FR-019). */
  noKeyring?: boolean;
  /** A computer whose PDF viewer can't be used safely (007 FR-003a): PDFs
   * are not previewable, TIFF and text still are. */
  pdfOff?: boolean;
  /** How many seconds the idle lock's minute lasts. */
  idleMinuteSeconds?: number;
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
  if (settings.noKeyring) {
    process.env.HOPLODEX_E2E_KEYRING = "unavailable";
  } else {
    delete process.env.HOPLODEX_E2E_KEYRING;
  }
  if (settings.pdfOff) {
    process.env.HOPLODEX_E2E_PDF_PREVIEW = "off";
  } else {
    delete process.env.HOPLODEX_E2E_PDF_PREVIEW;
  }
  process.env.HOPLODEX_E2E_CONSENT = "open";
  if (settings.idleMinuteSeconds !== undefined) {
    process.env.HOPLODEX_E2E_IDLE_MINUTE_SECONDS = String(settings.idleMinuteSeconds);
  } else {
    delete process.env.HOPLODEX_E2E_IDLE_MINUTE_SECONDS;
  }
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
 */
export function seedCollection(): string {
  if (!sandbox) throw new Error("seedCollection() needs isolateAppData() first");
  const dir = path.join(sandbox, "seed");
  if (runSeed(["--dir", dir], "inherit").status !== 0) {
    throw new Error("seeding the screenshot collection failed");
  }
  const printed = runSeed(["--print-passphrase"], "pipe");
  if (printed.status !== 0) throw new Error("reading the seed's passphrase failed");
  const passphrase = printed.stdout.trim();
  process.env.HOPLODEX_E2E_SEED_PASSPHRASE = passphrase;

  process.env.HOPLODEX_E2E_CONFIG_HOME = path.join(dir, "config");
  return passphrase;
}

/** Removes the sandbox. The app and its webview may still be shutting down,
 * and write cache files as they go, so removing the directory once can leave
 * some behind. */
export async function removeSandbox() {
  const root = sandbox;
  if (!root) return;
  for (let attempt = 0; attempt < 5 && fs.existsSync(root); attempt++) {
    await new Promise((resolve) => setTimeout(resolve, 500));
    fs.rmSync(root, { recursive: true, force: true });
  }
  sandbox = undefined;
}
