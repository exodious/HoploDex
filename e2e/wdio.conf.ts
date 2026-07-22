import path from "node:path";
import { fileURLToPath } from "node:url";
import { execSync, spawn, spawnSync, type ChildProcess } from "node:child_process";
import type { Options } from "@wdio/types";
import { browser } from "@wdio/globals";

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

export const config: Options.Testrunner = {
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

  beforeSession: () => {
    spawnSync(
      "cargo",
      [
        "build",
        "--release",
        "--features",
        "custom-protocol",
        "--manifest-path",
        "src-tauri/Cargo.toml",
      ],
      { cwd: repoRoot, stdio: "inherit" },
    );
    const nativeDriver = findNativeDriver();
    const args = nativeDriver ? ["--native-driver", nativeDriver] : [];
    tauriDriver = spawn("tauri-driver", args, {
      stdio: [null, process.stdout, process.stderr],
    });
  },

  afterSession: () => {
    tauriDriver?.kill();
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
  },
};
