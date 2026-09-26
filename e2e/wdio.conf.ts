import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { execSync, spawn, spawnSync, type ChildProcess } from "node:child_process";
import { browser } from "@wdio/globals";
import { SCREENSHOT_WINDOW, screenshotsEnabled } from "./support/screenshots";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..");

// Built app binary produced by `cargo build --release --features custom-protocol
// --manifest-path src-tauri/Cargo.toml` (the flag makes the app load bundled
// frontendDist assets via Tauri's custom protocol, matching what `cargo
// tauri build` does, instead of trying to hit the devUrl dev server).
const application = path.resolve(
  repoRoot,
  "src-tauri/target/release/hoplodex" + (process.platform === "win32" ? ".exe" : ""),
);

let tauriDriver: ChildProcess | undefined;
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
 * Set on this process's environment, which tauri-driver, and through it the
 * app, inherits. Specs read HOPLODEX_E2E_DOCUMENTS to type locations inside
 * the sandbox.
 */
function isolateAppData() {
  sandbox = fs.mkdtempSync(path.join(os.tmpdir(), "hoplodex-e2e-"));
  const [data, cache, config, documents] = ["data", "cache", "config", "documents"].map((name) => {
    const dir = path.join(sandbox!, name);
    fs.mkdirSync(dir, { recursive: true });
    return dir;
  });
  process.env.HOPLODEX_E2E_DOCUMENTS = documents;

  if (process.platform === "linux") {
    process.env.XDG_DATA_HOME = data;
    process.env.XDG_CACHE_HOME = cache;
    process.env.XDG_CONFIG_HOME = config;
    writeUserDirs(config, documents);
    // The document test opens a PDF through xdg-open, which would start a real
    // viewer. Mapping the type to a no-op in mimeapps.list isn't reliable (the
    // desktop environment's own defaults win), so put a stub first on PATH.
    const bin = path.join(sandbox, "bin");
    fs.mkdirSync(bin, { recursive: true });
    fs.writeFileSync(path.join(bin, "xdg-open"), "#!/bin/sh\nexit 0\n", { mode: 0o755 });
    process.env.PATH = `${bin}${path.delimiter}${process.env.PATH}`;
  } else if (process.platform === "win32") {
    process.env.APPDATA = data;
    process.env.LOCALAPPDATA = cache;
  }
}

/** Runs the human-testing seed (src-tauri/examples/human_seed.rs) with the
 * build's features, so it shares the app's build. */
function runSeed(args: string[], stdio: "inherit" | "pipe") {
  return spawnSync(
    "cargo",
    [
      "run",
      "--quiet",
      "--release",
      "--features",
      "custom-protocol,mock-keyring",
      "--manifest-path",
      "src-tauri/Cargo.toml",
      "--example",
      "human_seed",
      "--",
      ...args,
    ],
    { cwd: repoRoot, stdio: ["ignore", stdio, "inherit"], encoding: "utf-8" },
  );
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

  const config = path.join(dir, "config");
  process.env.XDG_CONFIG_HOME = config;
  writeUserDirs(config, process.env.HOPLODEX_E2E_DOCUMENTS!);
}

/**
 * A prior run's tauri-driver/WebKitWebDriver can be left holding these
 * ports if it was interrupted (Ctrl+C, crash) before `afterSession` had a
 * chance to kill it — the next run would otherwise fail to bind and every
 * session request would get rejected. Clearing them first makes repeated
 * or previously-interrupted runs self-healing.
 */
function killProcessesOnPorts(ports: number[]) {
  if (process.platform !== "linux") return;
  for (const port of ports) {
    try {
      const out = execSync(`ss -ltnp "sport = :${port}"`, { encoding: "utf-8" });
      for (const match of out.matchAll(/pid=(\d+)/g)) {
        try {
          process.kill(Number(match[1]), "SIGKILL");
        } catch {
          // already gone
        }
      }
    } catch {
      // ss unavailable, or no matching socket — nothing to clean up
    }
  }
}

/**
 * On Linux, tauri-driver wraps WebKitWebDriver, which isn't always on
 * $PATH (e.g. when only available via a Flatpak runtime). Falls back to
 * undefined so tauri-driver uses its own $PATH lookup on other platforms.
 */
function findNativeDriver(): string | undefined {
  if (process.platform !== "linux") return undefined;
  try {
    return execSync("which WebKitWebDriver", { encoding: "utf-8" }).trim();
  } catch {
    try {
      return (
        execSync("find / -maxdepth 20 -iname WebKitWebDriver -type f 2>/dev/null | head -1", {
          encoding: "utf-8",
        }).trim() || undefined
      );
    } catch {
      return undefined;
    }
  }
}

export const config: WebdriverIO.Config = {
  runner: "local",
  specs: ["./specs/**/*.e2e.ts"],
  maxInstances: 1,
  capabilities: [
    {
      // @ts-expect-error tauri-driver uses a custom capability shape, not a standard webdriver browser
      "tauri:options": { application },
      browserName: "wry",
      // WebdriverIO defaults to requesting a Bidi session (webSocketUrl:
      // true) unless this flag opts back into plain WebDriver classic —
      // tauri-driver only speaks classic and otherwise rejects the whole
      // session with "Failed to match capabilities".
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
  port: 4444,
  path: "/",

  beforeSession: (_config, _capabilities, specs) => {
    killProcessesOnPorts([4444, 4445]);
    isolateAppData();
    delete process.env.HOPLODEX_E2E_SEED_PASSPHRASE;
    spawnSync(
      "cargo",
      [
        "build",
        "--release",
        "--features",
        // mock-keyring swaps the OS keyring for an in-memory store, for
        // saved passphrases: headless environments have no way to unlock a
        // real one.
        "custom-protocol,mock-keyring",
        "--manifest-path",
        "src-tauri/Cargo.toml",
      ],
      { cwd: repoRoot, stdio: "inherit" },
    );
    if (specs.some((spec) => spec.endsWith("/e2e/screenshots/screens.e2e.ts"))) seedCollection();
    const nativeDriver = findNativeDriver();
    const args = nativeDriver ? ["--native-driver", nativeDriver] : [];
    tauriDriver = spawn("tauri-driver", args, {
      stdio: [null, process.stdout, process.stderr],
    });
  },

  afterSession: async () => {
    tauriDriver?.kill();
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
