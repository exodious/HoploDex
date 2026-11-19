// Checks a built package (#19) for what every published one must carry: the
// program, `NOTICES.txt` beside it (010's notices, which Tauri otherwise only
// embeds in the binary; src-tauri/tauri.bundle.conf.json adds the file), and
// for each kind what only it needs. The CI package jobs and the release
// workflow run it on what they built.
//
//   node scripts/check-package.mjs deb  FILE   dpkg-deb -c: the program, the
//                                              notices and apport's ignore entry (#166)
//   node scripts/check-package.mjs rpm  FILE   rpm -qlp: the program and the notices
//   node scripts/check-package.mjs nsis FILE   7z l: the program and the notices
//   node scripts/check-package.mjs dmg  FILE   (macOS) mounts it read-only: the
//                                              .app's notices and its own (ad-hoc)
//                                              signature (codesign --verify)
//
// Each takes one or more files. Node, so it runs the same on every OS. No
// dependencies of its own.

import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

/** The paths each kind's listing must hold, as `listing` normalises them. */
export const REQUIRED = {
  deb: ["usr/bin/hoplodex", "usr/lib/HoploDex/NOTICES.txt", "etc/apport/blacklist.d/hoplodex"],
  rpm: ["usr/bin/hoplodex", "usr/lib/HoploDex/NOTICES.txt"],
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

function main([kind, ...files]) {
  if (!(kind in REQUIRED) || !files.length) {
    throw new Error("usage: check-package.mjs deb|rpm|nsis|dmg FILE...");
  }
  for (const file of files) {
    const problems =
      kind === "dmg" ? checkDmg(file) : missing(kind, listing(kind, LIST[kind](file)));
    if (problems.length) {
      console.error(`${file} has no:\n${problems.map((p) => `  ${p}`).join("\n")}`);
      process.exitCode = 1;
    } else {
      console.log(`Checked ${file}: ${REQUIRED[kind].join(", ")}`);
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
