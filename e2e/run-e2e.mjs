import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const wdioBin = path.resolve(__dirname, "../node_modules/.bin/wdio");
const repoRoot = path.resolve(__dirname, "..");

// --screenshots[=<dir>] turns on e2e/support/screenshots.ts's shot() (default
// output e2e/screenshots-out/) and, unless --spec picks other specs, runs the
// dedicated screenshot walk instead of the test suite.
const SCREENSHOT_SPEC = "e2e/screenshots/screens.e2e.ts";
let screenshotDir;
const extraArgs = process.argv.slice(2).filter((arg) => {
  const match = /^--screenshots(?:=(.+))?$/.exec(arg);
  if (!match) return true;
  screenshotDir = path.resolve(match[1] ?? path.join(repoRoot, "e2e/screenshots-out"));
  return false;
});
if (screenshotDir && !extraArgs.some((arg) => arg === "--spec" || arg.startsWith("--spec="))) {
  extraArgs.push("--spec", SCREENSHOT_SPEC);
}

// On Linux, run under an isolated Xvfb virtual display so the app never
// renders on the developer's real desktop (wdio's own auto-Xvfb detection
// only kicks in when $DISPLAY is unset, which isn't true when DISPLAY
// points at a real desktop session via XWayland). Windows/macOS use their
// own native WebView drivers and have no equivalent concern.
const useXvfb = process.platform === "linux";

// GTK3 prefers a Wayland connection over X11 when $WAYLAND_DISPLAY is
// present in the environment, regardless of $DISPLAY — which would make
// the app connect straight to the developer's real Wayland compositor
// instead of the isolated Xvfb display above. Forcing the X11 backend
// (and dropping WAYLAND_DISPLAY) makes GTK honor $DISPLAY unconditionally.
const env = { ...process.env, GDK_BACKEND: "x11" };
delete env.WAYLAND_DISPLAY;
if (screenshotDir) env.HOPLODEX_SCREENSHOTS = screenshotDir;

const wdioArgs = [wdioBin, "run", "e2e/wdio.conf.ts", ...extraArgs];
const result = useXvfb
  ? // Room for a full-page screenshot's taller window, in true color.
    spawnSync("xvfb-run", ["-a", "-s", "-screen 0 1920x4200x24", ...wdioArgs], {
      stdio: "inherit",
      env,
    })
  : spawnSync(wdioArgs[0], wdioArgs.slice(1), { stdio: "inherit", env });

if (result.error) {
  console.error(result.error);
  process.exit(1);
}
process.exit(result.status ?? 1);
