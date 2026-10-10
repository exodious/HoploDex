import { spawnSync } from "node:child_process";
import fs from "node:fs";
import net from "node:net";
import path from "node:path";
import { launchApp, killApp, setPort } from "./support/app";
import { buildApp, repoRoot } from "./support/build";
import { startDisplay, stopDisplay } from "./support/display";
import {
  applySessionSettings,
  isolateAppData,
  removeSandbox,
  seedCollection,
} from "./support/sandbox";

/**
 * `npm run session [-- --seed] [-- --port <n>]`: a sandboxed launcher for
 * `wdio session`, WebdriverIO's CLI for driving an app step by step from the
 * shell (open, snapshot, click, exec, export a spec). It builds the E2E app,
 * starts it in the same throwaway sandbox the E2E harness gives each spec
 * (support/sandbox.ts) on a virtual display (Linux), and opens a `wdio
 * session` on the app's embedded WebDriver server with session.conf.ts. It
 * stays in the foreground until Ctrl+C, SIGTERM or `wdio session close`, then
 * closes the session, kills the app, stops the display and removes the sandbox.
 *
 * Never `wdio session open tauri <app>`: that starts the app itself, outside
 * the sandbox, on the developer's real databases. DEVELOPMENT.md, "Drive the
 * app with wdio session".
 */

/** The app's WebDriver port. Clear of the E2E workers', which take 4445 and
 * up (support/app.ts). */
const DEFAULT_PORT = 4545;

/** wdio's own entry, run with this Node, as run-e2e.mjs does:
 * node_modules/.bin/wdio is a shell script, which Windows can't start. */
const wdioBin = path.join(repoRoot, "node_modules/@wdio/cli/bin/wdio.js");

/** The session config's path relative to the repository: `wdio session`
 * resolves it from its working directory. */
const sessionConfig = "e2e/session.conf.ts";

interface Options {
  seed: boolean;
  port: number;
}

const USAGE = `Usage: npm run session -- [--seed] [--port <n>]

  --seed       also seed the human-testing collection ("Main collection" and
               "Shared collection") instead of starting at a first run
  --port <n>   the app's WebDriver port (default ${DEFAULT_PORT})`;

function parseOptions(argv: string[]): Options {
  const options: Options = { seed: false, port: DEFAULT_PORT };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === "--seed") {
      options.seed = true;
    } else if (arg === "--port" || arg.startsWith("--port=")) {
      const value = arg === "--port" ? argv[++i] : arg.slice("--port=".length);
      const port = Number(value);
      if (!Number.isInteger(port) || port < 1024 || port > 65535) {
        throw new Error(`--port must be a whole number from 1024 to 65535, not "${value}"`);
      }
      options.port = port;
    } else if (arg === "--help" || arg === "-h") {
      console.log(USAGE);
      process.exit(0);
    } else {
      throw new Error(`unknown argument "${arg}"\n${USAGE}`);
    }
  }
  return options;
}

/**
 * Refuses a port something else already uses. The app's launch clears its
 * port first (support/app.ts: an app left by an interrupted run would
 * otherwise block it), which is right for the E2E workers' ports and wrong
 * for one the developer chose: a leftover WebDriver server is replaced, any
 * other listener is not touched.
 */
async function checkPort(port: number) {
  const listening = await new Promise<boolean>((resolve) => {
    const socket = net.connect({ port, host: "127.0.0.1" });
    socket.once("connect", () => {
      socket.destroy();
      resolve(true);
    });
    socket.once("error", () => resolve(false));
  });
  if (!listening) return;
  try {
    const reply = await fetch(`http://127.0.0.1:${port}/status`, {
      signal: AbortSignal.timeout(2000),
    });
    const body = (await reply.json()) as { value?: { ready?: unknown } };
    if (typeof body.value?.ready === "boolean") return; // a WebDriver server: a leftover app
  } catch {
    // not a WebDriver server
  }
  throw new Error(`port ${port} is in use by something else; pick another with --port`);
}

/** Runs `wdio session <args>` from the repository root and returns its exit
 * status and what it printed. */
function wdioSession(args: string[], stdio: "inherit" | "pipe") {
  return spawnSync(process.execPath, [wdioBin, "session", ...args], {
    cwd: repoRoot,
    stdio: ["ignore", stdio, stdio === "inherit" ? "inherit" : "pipe"],
    encoding: "utf-8",
  });
}

/** Whether `wdio session` still has the default session. */
function sessionOpen(): boolean {
  const listed = wdioSession(["list", "--json"], "pipe");
  if (listed.status !== 0) return false;
  return /"name"\s*:\s*"default"/.test(listed.stdout);
}

/** Resolves on Ctrl+C or SIGTERM (or SIGHUP, a closed terminal). */
function interrupted(): Promise<string> {
  return new Promise((resolve) => {
    for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"] as const) {
      process.once(signal, () => resolve(signal));
    }
  });
}

/** Resolves once the `wdio session` is gone: closed from outside with
 * `wdio session close`, or ended by its idle timeout. */
async function sessionClosed(ended: { value: boolean }): Promise<string> {
  while (!ended.value) {
    await new Promise((resolve) => setTimeout(resolve, 3000));
    if (!ended.value && !sessionOpen()) return "the session closed";
  }
  return "ended";
}

async function main() {
  const options = parseOptions(process.argv.slice(2));

  if (!fs.existsSync(path.join(repoRoot, "dist/index.html"))) {
    throw new Error("dist/ is empty: run `npm run build` first, since the E2E build embeds it");
  }

  // GTK3 prefers a Wayland connection when $WAYLAND_DISPLAY is present,
  // whatever $DISPLAY says, which would put the app on the developer's real
  // desktop instead of the virtual display (run-e2e.mjs says more).
  if (process.platform === "linux") {
    process.env.GDK_BACKEND = "x11";
    delete process.env.WAYLAND_DISPLAY;
  }

  console.log("Building the E2E app (run `npm run build` first if the frontend changed)...");
  if (!buildApp()) throw new Error("building the app for E2E failed");

  const stopped = interrupted();
  const ended = { value: false };
  let sessionStarted = false;
  try {
    const sandbox = isolateAppData();
    applySessionSettings();
    const passphrase = options.seed ? seedCollection() : undefined;
    setPort(options.port);
    await checkPort(options.port);
    await startDisplay();
    await launchApp();

    if (wdioSession(["open", sessionConfig, "0", ...openFlags(options.port)], "inherit").status) {
      throw new Error("opening the wdio session failed");
    }
    sessionStarted = true;

    console.log(`
The app runs in a throwaway sandbox, never on your real data:
  ${sandbox}
${
  passphrase
    ? `Seeded: unlock "Main collection" or "Shared collection" with the passphrase
  ${passphrase}
`
    : `Not seeded: it starts at a first run. Create a database in the sandbox's
  documents folder, or restart with --seed.
`
}
Next, from ${repoRoot}:
  npx wdio session snapshot --interactive
  npx wdio session click 'aria/<accessible name>'    (refs such as e3 don't work here)
  npx wdio session exec -e "await browser.getTitle()"
  npx wdio session export --out e2e/specs/<name>.e2e.ts
  npx wdio session close          (also ends this launcher and removes the sandbox)

Ctrl+C here does the same.`);

    const reason = await Promise.race([stopped, sessionClosed(ended)]);
    console.log(`\nStopping (${reason}).`);
  } finally {
    ended.value = true;
    if (sessionStarted) wdioSession(["close"], "pipe");
    await killApp();
    await stopDisplay();
    await removeSandbox();
  }
}

/** `wdio session open`'s flags for a config target: the app's WebDriver
 * endpoint (session.conf.ts holds the rest), no BiDi (the embedded server
 * speaks WebDriver classic only), and an idle timeout long enough for a
 * person to look at the app between commands. */
function openFlags(port: number): string[] {
  return [
    "--hostname",
    "127.0.0.1",
    "--port",
    String(port),
    "--path",
    "/",
    "--protocol",
    "http",
    "--no-bidi",
    "--idle-timeout",
    "2h",
  ];
}

main().then(
  () => process.exit(0),
  (error: unknown) => {
    console.error(error instanceof Error ? error.message : error);
    process.exit(1);
  },
);
