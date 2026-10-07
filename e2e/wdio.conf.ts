import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { browser } from "@wdio/globals";
import { SCREENSHOT_WINDOW, screenshotsEnabled } from "./support/screenshots";
import { assignWorkerPort, buildProfile, killApp, launchApp } from "./support/app";
import { startDisplay, stopDisplay } from "./support/display";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");

// The E2E build: `cargo build --profile e2e --features custom-protocol,e2e`
// (custom-protocol makes the app load bundled frontendDist assets, matching
// what `cargo tauri build` does, instead of trying to hit the devUrl dev
// server; `e2e` is the embedded WebDriver server, the in-memory keyring and
// the other E2E-only settings). The `e2e` profile is the release one without
// fat LTO, so a backend change rebuilds in seconds. HOPLODEX_E2E_PROFILE=release
// builds and drives the shipping profile instead, as a release check
// (DEVELOPMENT.md, "Test"). `onPrepare` builds it, and the human-testing seed,
// once before any worker starts; support/app.ts launches the binary.
//
// Spec files run in parallel workers (HOPLODEX_E2E_WORKERS, see
// `workerCount`). Each worker is its own process, with its own sandbox,
// WebDriver port and, on Linux, X display (support/display.ts).

const cargoArgs = [
  "--profile",
  buildProfile,
  "--features",
  // e2e compiles in the WebDriver server. It implies mock-keyring, which
  // swaps the OS keyring for an in-memory store, for saved passphrases:
  // headless environments have no way to unlock a real one. It also lets a
  // spec shorten the idle lock's minute.
  "custom-protocol,e2e",
  "--manifest-path",
  "src-tauri/Cargo.toml",
];

/** The human-testing seed (src-tauri/examples/human_seed.rs), built with the
 * app's profile and features in `onPrepare`. */
const seedBinary = path.resolve(
  repoRoot,
  `src-tauri/target/${buildProfile}/examples/human_seed` +
    (process.platform === "win32" ? ".exe" : ""),
);

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

let sandbox: string | undefined;

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
 * Set on this process's environment, which the app, launched from here
 * (support/app.ts), inherits. Specs read HOPLODEX_E2E_DOCUMENTS to type
 * locations inside the sandbox.
 */
function isolateAppData() {
  sandbox = fs.mkdtempSync(path.join(os.tmpdir(), "hoplodex-e2e-"));
  const [data, cache, config, documents] = ["data", "cache", "config", "documents"].map((name) => {
    const dir = path.join(sandbox!, name);
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
  process.env.HOPLODEX_E2E_KEYRING_FILE = path.join(sandbox, "keyring.json");
  // An E2E build never hands an opened document to the OS, which would start
  // a real viewer: it writes the copy's path here instead (open_document, #27).
  process.env.HOPLODEX_E2E_OPENED_LOG = path.join(sandbox, "opened.log");

  if (process.platform === "linux") {
    process.env.XDG_DATA_HOME = data;
    process.env.XDG_CACHE_HOME = cache;
    process.env.XDG_CONFIG_HOME = config;
    writeUserDirs(config, documents);
  } else if (process.platform === "darwin") {
    // macOS ignores XDG_*: Tauri builds every directory (Application Support,
    // Caches, Documents) from HOME, so the app gets a home of its own inside
    // the sandbox (#28). Only the app: cargo, run from here, keeps the real one.
    const home = path.join(sandbox, "home");
    const documents = path.join(home, "Documents");
    fs.mkdirSync(path.join(home, "Library", "Application Support"), { recursive: true });
    fs.mkdirSync(documents, { recursive: true });
    process.env.HOPLODEX_E2E_HOME = home;
    process.env.HOPLODEX_E2E_DOCUMENTS = documents;
  }
}

/** Runs the human-testing seed `onPrepare` built. */
function runSeed(args: string[], stdio: "inherit" | "pipe") {
  return spawnSync(seedBinary, args, {
    cwd: repoRoot,
    stdio: ["ignore", stdio, "inherit"],
    encoding: "utf-8",
  });
}

/**
 * Seeds the human-testing databases for the screenshot walk in
 * e2e/screenshots/, which wants a realistic collection rather than an empty
 * one. The seed writes only into a directory it creates, so it gets a new
 * `seed` folder inside the sandbox, with its databases, backups and a
 * `machine.json` whose recent list names them. The app's config directory
 * then moves there, keeping the sandbox's documents folder, and the specs
 * read the seed's fixed passphrase from HOPLODEX_E2E_SEED_PASSPHRASE.
 */
function seedCollection() {
  const dir = path.join(sandbox!, "seed");
  if (runSeed(["--dir", dir], "inherit").status !== 0) {
    throw new Error("seeding the screenshot collection failed");
  }
  const printed = runSeed(["--print-passphrase"], "pipe");
  if (printed.status !== 0) throw new Error("reading the seed's passphrase failed");
  process.env.HOPLODEX_E2E_SEED_PASSPHRASE = printed.stdout.trim();

  process.env.HOPLODEX_E2E_CONFIG_HOME = path.join(dir, "config");
}

export const config: WebdriverIO.Config = {
  runner: "local",
  specs: ["./specs/**/*.e2e.ts"],
  maxInstances: workerCount(),
  // Each worker starts its own display in beforeSession (Linux), so
  // WebdriverIO mustn't wrap the workers in xvfb-run, which it does whenever
  // DISPLAY is unset.
  autoXvfb: false,
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
    const build = spawnSync(
      "cargo",
      ["build", ...cargoArgs, "--bin", "hoplodex", "--example", "human_seed"],
      { cwd: repoRoot, stdio: "inherit" },
    );
    if (build.status !== 0) {
      const error = new Error("building the app for E2E failed");
      error.name = "SevereServiceError";
      throw error;
    }
  },

  beforeSession: async (config, _capabilities, specs, cid) => {
    isolateAppData();
    delete process.env.HOPLODEX_E2E_SEED_PASSPHRASE;
    // WebdriverIO connects with this same config object once the hook
    // returns, so the session goes to this worker's app.
    config.port = assignWorkerPort(cid);
    if (specs.some((spec) => spec.endsWith("/e2e/screenshots/screens.e2e.ts"))) seedCollection();
    // A computer with no keyring service (FR-019).
    if (specs.some((spec) => spec.endsWith("-no-keyring.e2e.ts"))) {
      process.env.HOPLODEX_E2E_KEYRING = "unavailable";
    } else {
      delete process.env.HOPLODEX_E2E_KEYRING;
    }
    // A computer whose PDF viewer can't be used safely (007 FR-003a): PDFs
    // are not previewable, TIFF and text still are. Only a spec named so
    // gets it for its whole session; a spec that needs both sets
    // HOPLODEX_E2E_PDF_PREVIEW itself and calls relaunchApp(), and a
    // developer's own value never reaches the app.
    if (specs.some((spec) => spec.endsWith("-pdf-off.e2e.ts"))) {
      process.env.HOPLODEX_E2E_PDF_PREVIEW = "off";
    } else {
      delete process.env.HOPLODEX_E2E_PDF_PREVIEW;
    }
    // The idle lock's minute lasts 3 s in the locking spec, so its test
    // doesn't wait a real minute; every other spec keeps the real one.
    if (specs.some((spec) => spec.endsWith("/us9-locking.e2e.ts"))) {
      process.env.HOPLODEX_E2E_IDLE_MINUTE_SECONDS = "3";
    } else {
      delete process.env.HOPLODEX_E2E_IDLE_MINUTE_SECONDS;
    }
    await startDisplay();
    await launchApp();
  },

  afterSession: async () => {
    await killApp();
    await stopDisplay();
    if (!sandbox) return;
    // The app and its webview are still shutting down, and write cache files
    // as they go, so removing the directory once can leave some behind.
    for (let attempt = 0; attempt < 5 && fs.existsSync(sandbox); attempt++) {
      await new Promise((resolve) => setTimeout(resolve, 500));
      fs.rmSync(sandbox, { recursive: true, force: true });
    }
    sandbox = undefined;
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
