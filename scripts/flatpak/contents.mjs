// What the Flatpak bundle must carry and must not (011 research.md §13): the
// build's contents check (011 FR-017, 011 FR-018; 011 data-model.md "The
// bundle's contents"). `checkContents` is the pure check `contents.test.ts`
// (011 US3-3) calls; the CLI at the bottom lists a built app and asks `readelf`
// and the runtime for the three inputs.
//
//   node scripts/flatpak/contents.mjs <build directory>/files
//   node scripts/flatpak/contents.mjs --installed <installed app's deploy directory>/files
//
// `--installed` is for an app Flatpak installed (`flatpak info --show-location`'s
// `files/`): installing adds `files/.ref` and a launcher script, `export/bin/<app id>`,
// which Flatpak writes and the bundle does not carry; those two are set aside.
//
// prints `contents: ok`, or each problem, naming the path, and exits 1.

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const APP_ID = "io.github.exodious.HoploDex";
const ICON_SIZES = ["32x32", "64x64", "128x128", "256x256", "512x512"];

/** The paths the module installs, under the app's `files/` (011 data-model.md "The bundle's contents"). */
const INSTALLED = [
  "bin/hoplodex",
  `share/applications/${APP_ID}.desktop`,
  `share/metainfo/${APP_ID}.metainfo.xml`,
  ...ICON_SIZES.map((size) => `share/icons/hicolor/${size}/apps/${APP_ID}.png`),
  `share/licenses/${APP_ID}/NOTICES.txt`,
];

/**
 * What `flatpak build-finish` itself adds: the manifest, and the exported copies
 * of the desktop entry, the metainfo and the icons (not the binary or NOTICES.txt).
 */
const ADDED = [
  "manifest.json",
  ...INSTALLED.filter(
    (p) =>
      p.startsWith("share/applications/") ||
      p.startsWith("share/metainfo/") ||
      p.startsWith("share/icons/"),
  ).map((p) => `export/${p}`),
];

/**
 * What `flatpak-builder` itself composes into `files/` after the module's commands,
 * with `appstreamcli compose`, from the installed metainfo: the AppStream catalog
 * and the icons it names, at the sizes the composer picks (seen in the first
 * offline build, 011 T024). Generated data, no code; these exact paths only.
 */
const COMPOSED = [
  `share/app-info/xmls/${APP_ID}.xml.gz`,
  ...["48x48", "64x64", "128x128"].map(
    (size) => `share/app-info/icons/flatpak/${size}/${APP_ID}.png`,
  ),
];

/**
 * What newer flatpak-builder (1.4.8, not 1.4.4) adds on its own: the module source's
 * LICENSE, under the module's name. Optional, so both builders pass (#148).
 */
const RECORDED = [`share/licenses/${APP_ID}/hoplodex/LICENSE`];

const ALLOWED = new Set([...INSTALLED, ...ADDED, ...COMPOSED, ...RECORDED]);

/** A shared library by name: `libfoo.so`, `libfoo.so.1`, `libfoo.so.1.2.3`. */
const isSharedLibrary = (file) => /\.so(\.\d+)*$/.test(path.posix.basename(file));

/**
 * @param {{ files: string[], readelf: string, runtimeLibraries: string[] }} inputs
 *   the built app's file paths relative to its `files/` (the exported copies as
 *   `export/...`), the output of `readelf -d bin/hoplodex`, and the library file
 *   names the runtime has
 * @returns {string[]} one message per problem; empty when the bundle is right
 */
export function checkContents({ files, readelf, runtimeLibraries }) {
  const problems = [];
  const present = new Set(files);

  for (const file of files) {
    if (isSharedLibrary(file)) {
      problems.push(
        `${file} is a shared library in the bundle; the runtime provides every library (011 FR-018)`,
      );
    } else if (!ALLOWED.has(file)) {
      problems.push(`${file} is in the bundle but not in the allow list (011 FR-017)`);
    }
  }
  for (const file of INSTALLED) {
    if (!present.has(file)) problems.push(`${file} is missing from the bundle (011 FR-017)`);
  }

  const needed = [...readelf.matchAll(/\(NEEDED\)\s+Shared library: \[([^\]]+)\]/g)].map(
    (m) => m[1],
  );
  if (needed.length === 0) {
    problems.push(
      "readelf -d printed no NEEDED library for bin/hoplodex, so its output was not understood (011 FR-018)",
    );
  }
  const have = new Set(runtimeLibraries);
  for (const library of needed) {
    if (!have.has(library)) {
      problems.push(`bin/hoplodex needs ${library}, which the runtime does not have (011 FR-018)`);
    }
  }
  return problems;
}

/** Every file and symlink under `root`, as paths relative to it, with `/`. */
function listFiles(root, prefix = "") {
  if (!fs.existsSync(root)) return [];
  const found = [];
  for (const entry of fs.readdirSync(root, { withFileTypes: true })) {
    const relative = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (entry.isDirectory()) found.push(...listFiles(path.join(root, entry.name), relative));
    else found.push(relative);
  }
  return found;
}

function run(command, args) {
  const result = spawnSync(command, args, { encoding: "utf8" });
  if (result.error || result.status !== 0) {
    throw new Error(
      `${command} ${args.join(" ")} failed: ${result.error?.message ?? result.stderr.trim()}`,
    );
  }
  return result.stdout;
}

/** The names of the libraries in the GNOME runtime's `files/lib` and its multiarch folder. */
function runtimeLibraryNames() {
  const location = run("flatpak", [
    "info",
    "--user",
    "--show-location",
    "org.gnome.Platform//51",
  ]).trim();
  const lib = path.join(location, "files", "lib");
  const names = new Set();
  for (const folder of [lib, path.join(lib, "x86_64-linux-gnu")]) {
    if (fs.existsSync(folder)) for (const name of fs.readdirSync(folder)) names.add(name);
  }
  return [...names];
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  const installed = args.includes("--installed");
  const filesDir = args.find((arg) => !arg.startsWith("--"));
  if (!filesDir || !fs.existsSync(path.join(filesDir, "bin"))) {
    process.stderr.write(
      "usage: node scripts/flatpak/contents.mjs [--installed] <build or deploy directory>/files (the app's folder)\n",
    );
    process.exit(2);
  }
  try {
    // The exported copies sit beside files/, in the build directory's export/.
    const exported = listFiles(path.join(filesDir, "..", "export")).map((p) => `export/${p}`);
    // What installing adds (see above).
    const addedByInstall = new Set([".ref", `export/bin/${APP_ID}`]);
    const problems = checkContents({
      files: [...listFiles(filesDir), ...exported].filter(
        (file) => !(installed && addedByInstall.has(file)),
      ),
      readelf: run("readelf", ["-d", path.join(filesDir, "bin", "hoplodex")]),
      runtimeLibraries: runtimeLibraryNames(),
    });
    if (problems.length === 0) {
      process.stdout.write("contents: ok\n");
    } else {
      process.stderr.write(`${problems.join("\n")}\n`);
      process.exit(1);
    }
  } catch (error) {
    process.stderr.write(`contents: ${error.message}\n`);
    process.exit(1);
  }
}
