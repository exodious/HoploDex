import crypto from "node:crypto";
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

/**
 * Points the app at a throwaway data/config/cache directory for this session.
 *
 * The E2E build uses an in-memory keyring that generates a new database key on
 * every launch, so a database left behind by one spec can never be opened by
 * the next: SQLCipher fails its HMAC check and the app never comes up, which
 * leaves the next session request hanging until it times out. A fresh
 * directory per session avoids that, and also keeps E2E runs away from the
 * developer's real database.
 *
 * Set on this process's environment, which tauri-driver, and through it the
 * app, inherits.
 */
function isolateAppData() {
  sandbox = fs.mkdtempSync(path.join(os.tmpdir(), "hoplodex-e2e-"));
  const [data, cache, config] = ["data", "cache", "config"].map((name) => {
    const dir = path.join(sandbox!, name);
    fs.mkdirSync(dir, { recursive: true });
    return dir;
  });

  if (process.platform === "linux") {
    process.env.XDG_DATA_HOME = data;
    process.env.XDG_CACHE_HOME = cache;
    process.env.XDG_CONFIG_HOME = config;
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

/**
 * Seeds the session's sandbox with the human-testing collection
 * (src-tauri/examples/human_seed.rs), for the screenshot walk in
 * e2e/screenshots/, which wants a realistic collection rather than an empty
 * one. The mock keyring normally makes a new key on every launch, so the seed
 * and the app are given the same fixed one through HOPLODEX_E2E_DB_KEY (read
 * only by mock-keyring builds).
 */
function seedCollection() {
  const key = crypto.randomBytes(32).toString("hex");
  // The seed refuses to write to XDG_DATA_HOME (it takes it for the real data
  // directory), and the sandbox is XDG_DATA_HOME now, so leave it out.
  const env: NodeJS.ProcessEnv = { ...process.env, HOPLODEX_E2E_DB_KEY: key };
  delete env.XDG_DATA_HOME;
  const seeded = spawnSync(
    "cargo",
    [
      "run",
      "--release",
      "--features",
      "custom-protocol,mock-keyring",
      "--manifest-path",
      "src-tauri/Cargo.toml",
      "--example",
      "human_seed",
      "--",
      "--dir",
      sandbox!,
    ],
    { cwd: repoRoot, stdio: "inherit", env },
  );
  if (seeded.status !== 0) throw new Error("seeding the screenshot collection failed");
  process.env.HOPLODEX_E2E_DB_KEY = key;
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
    delete process.env.HOPLODEX_E2E_DB_KEY;
    spawnSync(
      "cargo",
      [
        "build",
        "--release",
        "--features",
        // mock-keyring swaps the OS keyring for an in-memory store — CI/
        // headless environments have no way to unlock a real one (see
        // src-tauri/src/db/mod.rs).
        "custom-protocol,mock-keyring",
        "--manifest-path",
        "src-tauri/Cargo.toml",
      ],
      { cwd: repoRoot, stdio: "inherit" },
    );
    if (specs.some((spec) => spec.includes("/e2e/screenshots/"))) seedCollection();
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
