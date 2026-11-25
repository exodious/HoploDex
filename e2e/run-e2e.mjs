import { spawnSync } from "node:child_process";
import fs from "node:fs";
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

// --store-shots (`npm run screenshots:store`, 011 FR-017) retakes the Flatpak's
// five store shots (e2e/screenshots/store.e2e.ts; what they show is
// store-shots.json) and, when the run passes, replaces flatpak/screenshots/'s
// files with them. Nothing else retakes them: not a build, not `screenshots`.
const STORE_SPEC = "e2e/screenshots/store.e2e.ts";
const STORE_STAGING = path.join(repoRoot, "e2e/screenshots-out/store");
const STORE_TARGET = path.join(repoRoot, "flatpak/screenshots");
// --showcase-shots (`npm run screenshots:showcase`) retakes the showcase's
// shots, light and dark (e2e/screenshots/showcase.e2e.ts): the GitHub Pages
// site's (site/) and the README's. When the run passes they replace
// docs/showcase/'s images. Like the store shots, nothing else retakes them.
const SHOWCASE_SPEC = "e2e/screenshots/showcase.e2e.ts";
const SHOWCASE_STAGING = path.join(repoRoot, "e2e/screenshots-out/showcase");
const SHOWCASE_TARGET = path.join(repoRoot, "docs/showcase");
const argv = process.argv.slice(2);
const storeShots = argv.includes("--store-shots");
const showcaseShots = argv.includes("--showcase-shots");
/** The one spec, and where it writes, of --store-shots or --showcase-shots. */
const fixedShots = storeShots
  ? {
      option: "--store-shots",
      script: "screenshots:store",
      spec: STORE_SPEC,
      staging: STORE_STAGING,
    }
  : showcaseShots
    ? {
        option: "--showcase-shots",
        script: "screenshots:showcase",
        spec: SHOWCASE_SPEC,
        staging: SHOWCASE_STAGING,
      }
    : null;
if (fixedShots) {
  if (argv.length > 1) {
    console.error(`${fixedShots.option} takes no other option`);
    process.exit(2);
  }
  if (process.platform !== "linux") {
    // The shots are the Linux dev container's (Xvfb, 1200 x 800, WebKitGTK),
    // the engine the Flatpak ships.
    console.error(
      `${fixedShots.option} runs on Linux, in the dev container (docs/development/screenshots.md)`,
    );
    process.exit(2);
  }
  // The chooser's shot shows the seed's folder, so it is the same every time:
  // a tmpfs the dev container mounts for these commands (and no other).
  const { seedDir } = JSON.parse(
    fs.readFileSync(path.join(repoRoot, "e2e/screenshots/store-shots.json"), "utf8"),
  );
  if (!fs.existsSync(seedDir)) {
    console.error(
      `${seedDir} is not here: run through \`scripts/dev-container.sh --store-shots npm run ${fixedShots.script}\``,
    );
    process.exit(2);
  }
  fs.rmSync(fixedShots.staging, { recursive: true, force: true });
}

let screenshotDir;
const extraArgs = (fixedShots ? ["--screenshots=" + fixedShots.staging] : argv).filter((arg) => {
  const match = /^--screenshots(?:=(.+))?$/.exec(arg);
  if (!match) return true;
  screenshotDir = path.resolve(match[1] ?? path.join(repoRoot, "e2e/screenshots-out"));
  return false;
});
if (screenshotDir && !extraArgs.some((arg) => arg === "--spec" || arg.startsWith("--spec="))) {
  for (const spec of fixedShots ? [fixedShots.spec] : SCREENSHOT_SPECS)
    extraArgs.push("--spec", spec);
}

// On Linux, each worker starts an isolated Xvfb virtual display of its own
// and points DISPLAY at it
// (e2e/support/display.ts), so the app never renders on the developer's real
// desktop and parallel workers' windows and real input don't share a screen.
// Windows and macOS have no equivalent concern.
//
// GTK3 prefers a Wayland connection over X11 when $WAYLAND_DISPLAY is
// present in the environment, regardless of $DISPLAY — which would make
// the app connect straight to the developer's real Wayland compositor
// instead of the worker's Xvfb display. Forcing the X11 backend (and
// dropping WAYLAND_DISPLAY) makes GTK honor $DISPLAY unconditionally.
//
// Dropping WAYLAND_DISPLAY matters to WebdriverIO 10 as well: its testrunner
// treats a host with WAYLAND_DISPLAY but no DISPLAY as already having a
// display, and sets GDK_BACKEND=wayland and XDG_SESSION_TYPE=wayland for the
// run, even with displayServerEnabled: false (wdio.conf.ts). Without it, the
// workers would inherit those.
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
if (storeShots && result.status === 0) {
  // The five named in store-shots.json, and no other file, go in together.
  const { shots } = JSON.parse(
    fs.readFileSync(path.join(repoRoot, "e2e/screenshots/store-shots.json"), "utf8"),
  );
  const files = shots.map((shot) => `${shot.name}.png`);
  const missing = files.filter((file) => !fs.existsSync(path.join(STORE_STAGING, file)));
  if (missing.length > 0) {
    console.error(`The store shots run did not take ${missing.join(", ")}; nothing was copied`);
    process.exit(1);
  }
  for (const file of files) {
    fs.copyFileSync(path.join(STORE_STAGING, file), path.join(STORE_TARGET, file));
  }
  console.log(`Retook ${files.join(", ")} in ${path.relative(repoRoot, STORE_TARGET)}/`);
}
if (showcaseShots && result.status === 0) {
  // The whole set goes in together, replacing the last one, so a shot the
  // spec no longer takes doesn't linger (scripts/showcase.test.ts checks the
  // page and the README name exactly these).
  const files = fs.readdirSync(SHOWCASE_STAGING).filter((file) => file.endsWith(".png"));
  if (files.length === 0) {
    console.error("The showcase run took no shots; nothing was copied");
    process.exit(1);
  }
  fs.mkdirSync(SHOWCASE_TARGET, { recursive: true });
  for (const file of fs.readdirSync(SHOWCASE_TARGET).filter((f) => f.endsWith(".png"))) {
    fs.rmSync(path.join(SHOWCASE_TARGET, file));
  }
  for (const file of files) {
    fs.copyFileSync(path.join(SHOWCASE_STAGING, file), path.join(SHOWCASE_TARGET, file));
  }
  console.log(`Retook ${files.length} shots in ${path.relative(repoRoot, SHOWCASE_TARGET)}/`);
}
process.exit(result.status ?? 1);
