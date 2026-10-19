// The host guard (011 US5-3, 011 SC-007; 011 research.md §15, §16): every
// Flatpak script runs this first, through env.sh. `flatpak` on a host would use
// the developer's own Flatpak installation, so the scripts run only
//   - in the dev container started with `scripts/dev-container.sh --flatpak`, or
//   - on a host, against a throwaway `FLATPAK_USER_DIR` (empty, or one this guard
//     marked before) with `--i-know-this-is-a-host` passed.
// `decide()` is a pure function of its inputs; the CLI at the bottom supplies the
// real ones.

import fs from "node:fs";
import os from "node:os";
import nodePath from "node:path";
import { fileURLToPath } from "node:url";

/** The dev container's hostname (CLAUDE.md, "Run tests in the dev container"). */
export const CONTAINER_HOSTNAME = "hoplodex-dev";
/** A file the dev container's Dockerfile creates; a host does not have it. */
export const CONTAINER_MARKER_FILE = "/etc/hoplodex-dev-container";
/** The file in a throwaway installation that says the guard accepted it. */
export const MARKER_NAME = ".hoplodex-throwaway-flatpak";
export const HOST_FLAG = "--i-know-this-is-a-host";
/**
 * A caller's request to accept the dev container only (011 T104: check.sh). It can
 * only make the guard stricter, so it is not a way past it.
 */
export const CONTAINER_ONLY_FLAG = "--container-only";

// Flatpak paths are POSIX paths whatever OS runs this file (the unit test runs on
// Windows and macOS too), so every path decision goes through `path.posix`.
const path = nodePath.posix;

const SYSTEM_INSTALLATION = "/var/lib/flatpak";

/**
 * @typedef {{
 *   exists(p: string): boolean,
 *   stat(p: string): "missing" | "directory" | "other",
 *   readdir(p: string): string[],
 *   realpath(p: string): string,
 * }} Reader
 */

const refuse = (message) => ({ ok: false, message });

/** Whether `a` is `b` or inside it. */
function within(a, b) {
  const rel = path.relative(b, a);
  return rel === "" || (!rel.startsWith("..") && !path.isAbsolute(rel));
}

/**
 * @param {{ env: Record<string, string | undefined>, hostname: string, argv: string[], home: string, fs: Reader }} inputs
 * @returns {{ ok: true, flatpakUserDir: string, stateFolder: string, writeMarker: boolean } | { ok: false, message: string }}
 */
export function decide({ env, hostname, argv, home, fs: reader }) {
  // All three of the container's markers; a host can share any one of them.
  const inContainer =
    env.HOPLODEX_DEV_CONTAINER === "1" &&
    hostname === CONTAINER_HOSTNAME &&
    reader.exists(CONTAINER_MARKER_FILE);

  if (inContainer) {
    if (!env.HOPLODEX_FLATPAK_STATE) {
      return refuse(
        "The dev container was started without Flatpak support. " +
          "Run this through `scripts/dev-container.sh --flatpak <command>`.",
      );
    }
    return {
      ok: true,
      flatpakUserDir: env.FLATPAK_USER_DIR || path.join(home, ".local/share/flatpak"),
      stateFolder: env.HOPLODEX_FLATPAK_STATE,
      writeMarker: false,
    };
  }

  if (argv.includes(CONTAINER_ONLY_FLAG)) {
    return refuse(
      "This command runs only in the dev container, never on a host (011 FR-023): the check needs " +
        "the container's own stand-ins, which a host does not have, namely folders outside any home " +
        "that the container's user can write (/mnt/hoplodex-check, /srv/hoplodex-check), private " +
        "D-Bus buses with a headless keyring and scripted portals, and an Xvfb display. " +
        "Run it through `scripts/dev-container.sh --flatpak <command>`. " +
        "`npm run bundle:flatpak` does build on a host, against a throwaway FLATPAK_USER_DIR.",
    );
  }

  const hostHelp =
    "This is not the dev container, and `flatpak` here would use your own Flatpak installation. " +
    "Run it through `scripts/dev-container.sh --flatpak <command>`, or on a host set " +
    `FLATPAK_USER_DIR to an empty or throwaway folder and pass ${HOST_FLAG}.`;

  const given = env.FLATPAK_USER_DIR;
  if (!given || !argv.includes(HOST_FLAG)) return refuse(hostHelp);

  if (!path.isAbsolute(given)) {
    return refuse(`FLATPAK_USER_DIR must be an absolute path, not "${given}".`);
  }
  const dir = reader.realpath(path.resolve(given));

  const dataHome =
    env.XDG_DATA_HOME && path.isAbsolute(env.XDG_DATA_HOME)
      ? env.XDG_DATA_HOME
      : path.join(home, ".local/share");
  const realInstallations = [
    path.join(home, ".local/share/flatpak"),
    path.join(dataHome, "flatpak"),
    SYSTEM_INSTALLATION,
  ];
  for (const real of realInstallations) {
    for (const candidate of new Set([real, reader.realpath(real)])) {
      // Inside it, or holding it, is as bad as it.
      if (within(dir, candidate) || within(candidate, dir)) {
        return refuse(
          `FLATPAK_USER_DIR (${given}) is, resolves to, or holds a real Flatpak installation (${real}). ` +
            "Name a different, empty folder.",
        );
      }
    }
  }

  const kind = reader.stat(dir);
  if (kind === "other") return refuse(`FLATPAK_USER_DIR (${given}) is not a folder.`);
  if (kind === "directory") {
    const entries = reader.readdir(dir);
    if (entries.length > 0 && !entries.includes(MARKER_NAME)) {
      return refuse(
        `FLATPAK_USER_DIR (${given}) is not empty and was not made by these scripts (no ${MARKER_NAME} in it). ` +
          "Name an empty folder.",
      );
    }
  }

  // The state folder (flatpak-builder's downloads, build folders and repository):
  // the one the caller names, else a sibling of the throwaway installation, so that
  // a host without podman needs no variable beyond FLATPAK_USER_DIR (011 T104).
  let stateFolder = `${dir}-state`;
  if (env.HOPLODEX_FLATPAK_STATE) {
    if (!path.isAbsolute(env.HOPLODEX_FLATPAK_STATE)) {
      return refuse(
        `HOPLODEX_FLATPAK_STATE must be an absolute path, not "${env.HOPLODEX_FLATPAK_STATE}".`,
      );
    }
    stateFolder = reader.realpath(path.resolve(env.HOPLODEX_FLATPAK_STATE));
  }
  for (const real of realInstallations) {
    for (const candidate of new Set([real, reader.realpath(real)])) {
      if (within(stateFolder, candidate) || within(candidate, stateFolder)) {
        return refuse(
          `The state folder (${stateFolder}) is, is inside, or holds a real Flatpak installation (${real}). ` +
            "Name a different folder in HOPLODEX_FLATPAK_STATE.",
        );
      }
    }
  }
  if (within(stateFolder, dir)) {
    return refuse(
      `The state folder (${stateFolder}) is the throwaway installation or inside it; name a folder beside it.`,
    );
  }
  return { ok: true, flatpakUserDir: dir, stateFolder, writeMarker: true };
}

/** The real file system, for the CLI. A path that doesn't exist resolves through its nearest existing parent. */
const realReader = {
  exists: (p) => fs.existsSync(p),
  stat: (p) => {
    try {
      return fs.statSync(p).isDirectory() ? "directory" : "other";
    } catch {
      return "missing";
    }
  },
  readdir: (p) => fs.readdirSync(p),
  realpath: (p) => {
    const rest = [];
    let current = path.resolve(p);
    for (;;) {
      try {
        return path.join(fs.realpathSync(current), ...rest);
      } catch {
        const parent = path.dirname(current);
        if (parent === current) return path.resolve(p);
        rest.unshift(path.basename(current));
        current = parent;
      }
    }
  },
};

// The CLI: prints the accepted FLATPAK_USER_DIR on the first line, `marker` or
// `no-marker` on the second (does the caller write the marker file) and the state
// folder on the third; on a refusal
// prints the message to stderr and exits 1.
if (process.argv[1] && nodePath.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const result = decide({
    env: process.env,
    hostname: os.hostname(),
    argv: process.argv.slice(2),
    home: os.homedir(),
    fs: realReader,
  });
  if (!result.ok) {
    process.stderr.write(`${result.message}\n`);
    process.exit(1);
  }
  process.stdout.write(
    `${result.flatpakUserDir}\n${result.writeMarker ? "marker" : "no-marker"}\n${result.stateFolder}\n`,
  );
}
