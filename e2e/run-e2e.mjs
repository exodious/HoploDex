import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
// wdio's own entry, run with this Node: node_modules/.bin/wdio is a shell
// script, which Windows can't start (it has wdio.cmd instead).
const wdioBin = path.resolve(__dirname, "../node_modules/@wdio/cli/bin/wdio.js");
const repoRoot = path.resolve(__dirname, "..");

// --screenshots[=<dir>] turns on e2e/support/screenshots.ts's shot() (default
// output e2e/screenshots-out/) and, unless --spec picks other specs, runs the
// dedicated screenshot walk instead of the test suite: the seeded walk, and
// the first-run screens, which need a sandbox with no databases.
const SCREENSHOT_SPECS = ["e2e/screenshots/screens.e2e.ts", "e2e/screenshots/first-run.e2e.ts"];
let screenshotDir;
const extraArgs = process.argv.slice(2).filter((arg) => {
  const match = /^--screenshots(?:=(.+))?$/.exec(arg);
  if (!match) return true;
  screenshotDir = path.resolve(match[1] ?? path.join(repoRoot, "e2e/screenshots-out"));
  return false;
});
if (screenshotDir && !extraArgs.some((arg) => arg === "--spec" || arg.startsWith("--spec="))) {
  for (const spec of SCREENSHOT_SPECS) extraArgs.push("--spec", spec);
}

// On Linux, each worker starts an isolated Xvfb virtual display of its own
// and points DISPLAY at it (e2e/support/display.ts), so the app never
// renders on the developer's real desktop and parallel workers' windows and
// real input don't share a screen. Windows and macOS have no equivalent
// concern.
//
// GTK3 prefers a Wayland connection over X11 when $WAYLAND_DISPLAY is
// present in the environment, regardless of $DISPLAY — which would make
// the app connect straight to the developer's real Wayland compositor
// instead of the worker's Xvfb display. Forcing the X11 backend (and
// dropping WAYLAND_DISPLAY) makes GTK honor $DISPLAY unconditionally.
const env = { ...process.env, GDK_BACKEND: "x11" };
delete env.WAYLAND_DISPLAY;
if (screenshotDir) env.HOPLODEX_SCREENSHOTS = screenshotDir;

const result = spawnSync(process.execPath, [wdioBin, "run", "e2e/wdio.conf.ts", ...extraArgs], {
  stdio: "inherit",
  env,
});

if (result.error) {
  console.error(result.error);
  process.exit(1);
}
process.exit(result.status ?? 1);
