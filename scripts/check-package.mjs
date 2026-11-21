// Checks a built package (#19) for what every published one must carry: the
// program, `NOTICES.txt` beside it (010's notices, which Tauri otherwise only
// embeds in the binary; src-tauri/tauri.bundle.conf.json adds the file), and
// for each kind what only it needs. The CI package jobs and the release
// workflow run it on what they built.
//
//   node scripts/check-package.mjs deb  FILE   dpkg-deb -c: the program, the
//                                              notices, the launcher entry and
//                                              apport's ignore entry (#166)
//   node scripts/check-package.mjs rpm  FILE   rpm -qlp: the program, the notices
//                                              and the launcher entry
//   node scripts/check-package.mjs nsis FILE   7z l: the program and the notices
//   node scripts/check-package.mjs dmg  FILE   (macOS) mounts it read-only: the
//                                              .app's notices and its own (ad-hoc)
//                                              signature (codesign --verify)
//
// Each takes one or more files. With `--rename` (the CI package jobs), each
// file that passes is then renamed to its release name, `releaseName`'s
// HoploDex-<version>-<arch>[-setup].<ext> (#209). Node, so it runs the same on
// every OS. No dependencies of its own.

import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, renameSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

/** The paths each kind's listing must hold, as `listing` normalises them. */
export const REQUIRED = {
  deb: [
    "usr/bin/hoplodex",
    "usr/lib/hoplodex/NOTICES.txt",
    "usr/share/applications/hoplodex.desktop",
    "etc/apport/blacklist.d/hoplodex",
  ],
  rpm: [
    "usr/bin/hoplodex",
    "usr/lib/hoplodex/NOTICES.txt",
    "usr/share/applications/hoplodex.desktop",
  ],
  nsis: ["hoplodex.exe", "NOTICES.txt"],
  dmg: ["HoploDex.app/Contents/MacOS/hoplodex", "HoploDex.app/Contents/Resources/NOTICES.txt"],
};

/**
 * Each tool's listing as package-relative paths: dpkg-deb's last column
 * without its `./`, rpm's absolute paths without the `/`, and 7z's `Path = `
 * lines (from `-slt`) with NSIS's `$INSTDIR` and backslashes dropped.
 */
export function listing(kind, out) {
  const lines = out.split(/\r?\n/).filter(Boolean);
  switch (kind) {
    case "deb":
      return lines.map((l) => l.trim().split(/\s+/).pop().replace(/^\.\//, ""));
    case "rpm":
      return lines.map((l) => l.trim().replace(/^\//, ""));
    case "nsis":
      return lines
        .filter((l) => l.startsWith("Path = "))
        .map((l) =>
          l
            .slice("Path = ".length)
            .replaceAll("\\", "/")
            .replace(/^\$INSTDIR\//, ""),
        );
    default:
      throw new Error(`no listing for ${kind}`);
  }
}

export function missing(kind, paths) {
  const have = new Set(paths);
  return REQUIRED[kind].filter((p) => !have.has(p));
}

/**
 * Each kind's Tauri file-name ending for the one architecture a release ships
 * it for, and the ending of our name for it.
 */
const SUFFIX = {
  deb: ["_amd64.deb", "x86_64.deb"],
  rpm: [".x86_64.rpm", "x86_64.rpm"],
  nsis: ["_x64-setup.exe", "x86_64-setup.exe"],
  dmg: ["_aarch64.dmg", "aarch64.dmg"],
};

/**
 * The release's name for a package Tauri built, one pattern for every file
 * (the Flatpak's, from scripts/flatpak/build.sh, too): HoploDex-<version>-
 * <x86_64|aarch64>, then `-setup.exe`, `.dmg`, `.deb` or `.rpm` (#209).
 * Throws on a name that isn't this version's Tauri name for that kind, the
 * architecture a release ships it for included.
 */
export function releaseName(kind, file, version) {
  const base = path.basename(file);
  const [tauri, ours] = SUFFIX[kind] ?? [];
  if (!tauri || !base.includes(version) || !base.endsWith(tauri)) {
    throw new Error(`${base} isn't Tauri's ${kind} for ${version}`);
  }
  return `HoploDex-${version}-${ours}`;
}

function run(cmd, args) {
  const r = spawnSync(cmd, args, { encoding: "utf8", maxBuffer: 64 << 20 });
  if (r.error) throw new Error(`${cmd}: ${r.error.message}`);
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(" ")} exited ${r.status}: ${r.stderr}`);
  return r.stdout;
}

const LIST = {
  deb: (file) => run("dpkg-deb", ["-c", file]),
  rpm: (file) => run("rpm", ["-qlp", file]),
  nsis: (file) => run("7z", ["l", "-slt", file]),
};

/** Mounts the image read-only, checks the app inside, and always detaches it. */
function checkDmg(file) {
  const mount = mkdtempSync(path.join(os.tmpdir(), "hoplodex-dmg-"));
  run("hdiutil", ["attach", file, "-readonly", "-nobrowse", "-noautoopen", "-mountpoint", mount]);
  try {
    const problems = missing(
      "dmg",
      REQUIRED.dmg.filter((p) => spawnSync("test", ["-f", path.join(mount, p)]).status === 0),
    );
    const app = path.join(mount, "HoploDex.app");
    const sig = spawnSync("codesign", ["--verify", "--deep", "--strict", "--verbose=2", app], {
      encoding: "utf8",
    });
    if (sig.status !== 0) problems.push(`a valid signature (codesign: ${sig.stderr.trim()})`);
    return problems;
  } finally {
    run("hdiutil", ["detach", mount]);
    rmSync(mount, { recursive: true, force: true });
  }
}

function main(args) {
  const rename = args[0] === "--rename";
  const [kind, ...files] = rename ? args.slice(1) : args;
  if (!(kind in REQUIRED) || !files.length) {
    throw new Error("usage: check-package.mjs [--rename] deb|rpm|nsis|dmg FILE...");
  }
  const conf = new URL("../src-tauri/tauri.conf.json", import.meta.url);
  const { version } = JSON.parse(readFileSync(conf, "utf8"));
  for (const file of files) {
    const problems =
      kind === "dmg" ? checkDmg(file) : missing(kind, listing(kind, LIST[kind](file)));
    if (problems.length) {
      console.error(`${file} has no:\n${problems.map((p) => `  ${p}`).join("\n")}`);
      process.exitCode = 1;
    } else {
      console.log(`Checked ${file}: ${REQUIRED[kind].join(", ")}`);
      if (rename) {
        const to = path.join(path.dirname(file), releaseName(kind, file, version));
        renameSync(file, to);
        console.log(`Renamed it ${to}`);
      }
    }
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main(process.argv.slice(2));
  } catch (e) {
    console.error(e.message);
    process.exitCode = 1;
  }
}
