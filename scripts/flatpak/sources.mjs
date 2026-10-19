// The offline source lists against the lockfiles (011 US3-2, 011 FR-015,
// 011 SC-004; 011 data-model.md "Offline source lists", 011 research.md §3).
//
// `flatpak/cargo-sources.json` and `flatpak/node-sources.json` are generated
// by `npm run flatpak:sources` (flatpak-builder-tools' generators, which need
// Python and the network). This module is the cheap check that runs in every
// `npm test`: it reads a lockfile and its list and says, without Python or the
// network, whether the list still covers exactly the lockfile's packages with
// the lockfile's hashes. Every path is a parameter, so the tests point it at
// fixtures and at the real files.

import fs from "node:fs";
import path from "node:path";

/** What a failure tells the developer to do. */
const REGENERATE = "Run `npm run flatpak:sources` in the dev container and commit both lists.";

/** Source types the generators write that are configuration, not a package. */
const CONFIG_TYPES = new Set(["inline", "script", "shell"]);
/** Where `flatpak-node-generator` puts the tools it unpacks (the node list's `dest`). */
const TOOL_CACHE = "flatpak-node/cache/";

/** The name a message gives a list: the repository path it has. */
function listName(listPath) {
  return `flatpak/${path.basename(listPath)}`;
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

function wrongList(list, lock, detail) {
  return `${list} does not match ${lock}: ${detail}. ${REGENERATE}`;
}

// ---------------------------------------------------------------------------
// Cargo

/**
 * The `[[package]]` tables of a `Cargo.lock`, as `{ name, version, source,
 * checksum }`. Read as text, with no TOML parser: the lock file writes each
 * of those four keys as `key = "value"` on a line of its own.
 */
export function parseCargoLock(text) {
  const packages = [];
  let current = null;
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (line === "[[package]]") {
      current = {};
      packages.push(current);
    } else if (line.startsWith("[")) {
      current = null;
    } else if (current) {
      const match = /^(name|version|source|checksum) = "(.*)"$/.exec(line);
      if (match) current[match[1]] = match[2];
    }
  }
  return packages;
}

/** The crate a crates.io archive URL names, or null for any other URL. */
function crateOfUrl(url) {
  const match = /^https:\/\/static\.crates\.io\/crates\/([^/]+)\/\1-(.+)\.crate$/.exec(url ?? "");
  return match ? { name: match[1], version: match[2] } : null;
}

/**
 * Compares `Cargo.lock` with `cargo-sources.json`.
 * @param {{ cargoLockPath: string, sourcesPath: string }} paths
 * @returns {string | null} the first difference, or null when the list is right
 */
export function checkCargoSources({ cargoLockPath, sourcesPath }) {
  const list = listName(sourcesPath);
  const lock = "src-tauri/Cargo.lock";
  const packages = parseCargoLock(fs.readFileSync(cargoLockPath, "utf8"));
  const sources = readJson(sourcesPath);

  for (const pkg of packages) {
    if (pkg.source?.startsWith("git+")) {
      return (
        `${list} cannot cover ${lock}: crate ${pkg.name} comes from ${pkg.source}, ` +
        "which the offline Flatpak build cannot fetch (011 research.md §3)"
      );
    }
  }

  const registry = packages.filter((pkg) => pkg.source?.startsWith("registry+"));
  const archives = sources.filter((s) => s.type === "archive");

  for (const pkg of registry) {
    const url = `https://static.crates.io/crates/${pkg.name}/${pkg.name}-${pkg.version}.crate`;
    const matching = archives.filter((s) => s.url === url);
    if (matching.length === 0) {
      const other = archives
        .map((s) => crateOfUrl(s.url))
        .find((crate) => crate?.name === pkg.name);
      return wrongList(
        list,
        lock,
        other
          ? `crate ${pkg.name} is at ${pkg.version} in ${lock}, but the list has ${other.version}`
          : `crate ${pkg.name} ${pkg.version} has no source`,
      );
    }
    if (matching.length > 1) {
      return wrongList(
        list,
        lock,
        `crate ${pkg.name} ${pkg.version} has ${matching.length} sources, not one`,
      );
    }
    if (matching[0].sha256 !== pkg.checksum) {
      return wrongList(
        list,
        lock,
        `crate ${pkg.name} ${pkg.version} has sha256 ${matching[0].sha256} in the list ` +
          `but ${pkg.checksum} in ${lock}`,
      );
    }
  }

  const known = new Set(registry.map((pkg) => `${pkg.name}/${pkg.version}`));
  for (const source of sources) {
    if (CONFIG_TYPES.has(source.type)) continue;
    const crate = source.type === "archive" ? crateOfUrl(source.url) : null;
    if (crate && known.has(`${crate.name}/${crate.version}`)) continue;
    const what = crate
      ? `crate ${crate.name} ${crate.version}`
      : `${source.type} source ${source.url ?? source.dest ?? ""}`.trim();
    return wrongList(list, lock, `the list has a source for ${what}, which ${lock} does not have`);
  }
  return null;
}

// ---------------------------------------------------------------------------
// npm

/** `sha512-<base64>` as the algorithm and the hex digest `flatpak-builder` writes. */
function parseIntegrity(integrity) {
  for (const part of String(integrity).split(/\s+/)) {
    const match = /^(sha512|sha256|sha1)-(.+)$/.exec(part);
    if (match) {
      return { algorithm: match[1], hex: Buffer.from(match[2], "base64").toString("hex") };
    }
  }
  return null;
}

/** The package a `package-lock.json` key names: the part after the last `node_modules/`. */
function npmName(key, entry) {
  return entry.name ?? key.slice(key.lastIndexOf("node_modules/") + "node_modules/".length);
}

/**
 * Compares `package-lock.json` with `node-sources.json`.
 * @param {{ packageLockPath: string, sourcesPath: string }} paths
 * @returns {string | null} the first difference, or null when the list is right
 */
export function checkNodeSources({ packageLockPath, sourcesPath }) {
  const list = listName(sourcesPath);
  const lock = "package-lock.json";
  const packages = readJson(packageLockPath).packages ?? {};
  const sources = readJson(sourcesPath);
  // flatpak-node-generator unpacks esbuild's platform binary a second time, from
  // the tarball it already lists as a `file`, into its tool cache (011
  // research.md §3): that archive is the same package, not a second source.
  const downloads = sources.filter(
    (s) =>
      !CONFIG_TYPES.has(s.type) && !(s.type === "archive" && String(s.dest).startsWith(TOOL_CACHE)),
  );

  // One tarball may sit at several places in node_modules: it is one source.
  const wanted = new Map();
  for (const [key, entry] of Object.entries(packages)) {
    if (key === "" || !entry.resolved || !entry.integrity) continue;
    if (!wanted.has(entry.resolved)) {
      wanted.set(entry.resolved, {
        label: `npm package ${npmName(key, entry)}@${entry.version}`,
        integrity: entry.integrity,
      });
    }
  }

  for (const [url, { label, integrity }] of wanted) {
    const matching = downloads.filter((s) => s.url === url);
    if (matching.length === 0) {
      return wrongList(list, lock, `${label} (${url}) has no source`);
    }
    if (matching.length > 1) {
      return wrongList(list, lock, `${label} has ${matching.length} sources, not one`);
    }
    const expected = parseIntegrity(integrity);
    const actual = expected ? matching[0][expected.algorithm] : undefined;
    if (!expected || actual !== expected.hex) {
      return wrongList(
        list,
        lock,
        `${label} has ${expected?.algorithm ?? "no known"} hash ${actual ?? "(none)"} in the list ` +
          `but ${expected?.hex ?? integrity} in ${lock}`,
      );
    }
  }

  for (const source of downloads) {
    if (!wanted.has(source.url)) {
      return wrongList(
        list,
        lock,
        `the list has a source for ${source.url ?? source.dest ?? source.type}, which ${lock} does not have`,
      );
    }
  }
  return null;
}

// ---------------------------------------------------------------------------

/**
 * Both comparisons.
 * @param {{ cargoLockPath: string, packageLockPath: string, cargoSourcesPath: string, nodeSourcesPath: string }} paths
 * @returns {string[]} one message per list that is wrong; empty when both are right
 */
export function checkSources({
  cargoLockPath,
  packageLockPath,
  cargoSourcesPath,
  nodeSourcesPath,
}) {
  return [
    checkCargoSources({ cargoLockPath, sourcesPath: cargoSourcesPath }),
    checkNodeSources({ packageLockPath, sourcesPath: nodeSourcesPath }),
  ].filter((message) => message !== null);
}
