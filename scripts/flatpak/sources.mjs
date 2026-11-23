// The offline source lists against the lockfiles (011 US3-2, 011 FR-015,
// 011 SC-004; 011 data-model.md "Offline source lists", 011 research.md §3).
//
// `flatpak/cargo-sources.json` and `flatpak/node-sources.json` are generated
// by `npm run flatpak:sources` (flatpak-builder-tools' generators, which need
// Python and the network). This module is the cheap check that runs in every
// `npm test`: it reads a lockfile and its list and says, without Python or the
// network, whether the list still covers exactly the lockfile's packages with
// the lockfile's hashes, and whether every entry that is not a package (the
// `inline`, `script` and `shell` entries) is one the pinned generators write,
// where they write it (code scanning alert build-ci/cwe-345): such an entry
// writes or runs something in the build directory, beside the checkout, before
// the build. Every path is a parameter, so the tests point it at fixtures and at
// the real files.

import fs from "node:fs";
import path from "node:path";

/** What a failure tells the developer to do. */
const REGENERATE = "Run `npm run flatpak:sources` in the dev container and commit both lists.";

/** Source types the generators write that are configuration, not a package. */
const CONFIG_TYPES = new Set(["inline", "script", "shell"]);
/** Where `flatpak-node-generator` puts the tools it unpacks (the node list's `dest`). */
const TOOL_CACHE = "flatpak-node/cache/";
/** Where it unpacks esbuild's platform binaries, and the shell entry's `dest` for each. */
const ESBUILD_CACHE = `${TOOL_CACHE}esbuild`;
/** What `flatpak-cargo-generator.py` writes as `cargo/config` for crates.io crates only. */
const CARGO_CONFIG =
  '[source.vendored-sources]\ndirectory = "cargo/vendor"\n\n' +
  '[source.crates-io]\nreplace-with = "vendored-sources"\n';
/** `flatpak-node-generator`'s node-gyp headers script and the command that runs it (`--xdg-layout`). */
const NODE_HEADERS_SCRIPT = [
  'version=$(node --version | sed "s/^v//")',
  'nodedir=$(dirname "$(dirname "$(which node)")")',
  'mkdir -p "flatpak-node/cache/node-gyp/$version"',
  'ln -s "$nodedir/include" "flatpak-node/cache/node-gyp/$version/include"',
  'echo 11 > "flatpak-node/cache/node-gyp/$version/installVersion"',
];
const NODE_HEADERS_COMMAND = "bash flatpak-node/setup_sdk_node_headers.sh";

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

/** Whether `source` has exactly these keys, so no other one (`base64`, `url`) changes what it does. */
function hasKeys(source, ...keys) {
  const own = Object.keys(source);
  return own.length === keys.length + 1 && ["type", ...keys].every((k) => own.includes(k));
}

/** Whether `source` has no key but these (and `type`), so none (`path`, `base64`) changes what it fetches. */
function onlyKeys(source, ...keys) {
  return Object.keys(source).every((k) => k === "type" || keys.includes(k));
}

/**
 * Whether `source` is an object whose every field has the type the generators
 * write: flatpak-builder skips a field of another type (an array `dest` would
 * put the source at the build directory's root, over the checkout), while
 * `String()` and `RegExp.test` here would read it as text.
 */
function isWellTyped(source) {
  if (typeof source !== "object" || source === null || Array.isArray(source)) return false;
  return Object.entries(source).every(([key, value]) => {
    if (key === "commands" || key === "only-arches") {
      return Array.isArray(value) && value.every((v) => typeof v === "string");
    }
    return key === "strip-components" ? Number.isInteger(value) : typeof value === "string";
  });
}

/** The first entry of a list that is not an object with fields of the generators' types. */
function badlyTyped(list, lock, sources) {
  if (!Array.isArray(sources)) return wrongList(list, lock, "the list is not an array of sources");
  const bad = sources.find((source) => !isWellTyped(source));
  return bad === undefined
    ? null
    : wrongList(
        list,
        lock,
        `the list has a source with a field of the wrong type: ${JSON.stringify(bad)}`,
      );
}

/** The difference that names an entry the generator does not write. */
function strayEntry(source) {
  const where = [source.dest, source["dest-filename"]].filter(Boolean).join("/");
  return `the list has ${source.type === "inline" ? "an" : "a"} ${source.type} source${where ? ` at ${where}` : ""} that the generator does not write`;
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
  const match =
    /^https:\/\/static\.crates\.io\/crates\/([A-Za-z0-9_-]+)\/\1-([0-9A-Za-z.+-]+)\.crate$/.exec(
      url ?? "",
    );
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
  const typeProblem = badlyTyped(list, lock, sources);
  if (typeProblem) return typeProblem;

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

  // A crate's name and version build its url and its folder in cargo/vendor, so
  // only the characters crates.io allows: no `/`, `#` or `..` that climbs out.
  const odd = registry.find(
    (pkg) =>
      !/^[A-Za-z0-9_-]+$/.test(pkg.name ?? "") || !/^[0-9A-Za-z.+-]+$/.test(pkg.version ?? ""),
  );
  if (odd) {
    return wrongList(
      list,
      lock,
      `crate ${JSON.stringify(odd.name)} ${JSON.stringify(odd.version)} has a name or version crates.io doesn't allow`,
    );
  }

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
  // Each crate's directory, and the `.cargo-checksum.json` the generator writes there.
  const checksums = new Map(
    registry.map((pkg) => [
      `cargo/vendor/${pkg.name}-${pkg.version}`,
      JSON.stringify({ package: pkg.checksum, files: {} }),
    ]),
  );
  for (const source of sources) {
    if (CONFIG_TYPES.has(source.type)) {
      if (!isCargoConfigEntry(source, checksums)) return wrongList(list, lock, strayEntry(source));
      continue;
    }
    const crate = source.type === "archive" ? crateOfUrl(source.url) : null;
    if (crate && known.has(`${crate.name}/${crate.version}`)) {
      if (
        source.dest === `cargo/vendor/${crate.name}-${crate.version}` &&
        source["archive-type"] === "tar-gzip" &&
        hasKeys(source, "archive-type", "url", "sha256", "dest")
      ) {
        continue;
      }
      return wrongList(
        list,
        lock,
        `crate ${crate.name} ${crate.version} unpacks to ${source.dest}`,
      );
    }
    const what = crate
      ? `crate ${crate.name} ${crate.version}`
      : `${source.type} source ${source.url ?? source.dest ?? ""}`.trim();
    return wrongList(list, lock, `the list has a source for ${what}, which ${lock} does not have`);
  }
  return null;
}

/**
 * Whether an inline entry is one `flatpak-cargo-generator.py` writes for
 * crates.io crates: `cargo/config`, or a crate's `.cargo-checksum.json` with its
 * lockfile checksum and no files.
 */
function isCargoConfigEntry(source, checksums) {
  if (source.type !== "inline" || !hasKeys(source, "contents", "dest", "dest-filename")) {
    return false;
  }
  if (source.dest === "cargo" && source["dest-filename"] === "config") {
    return source.contents === CARGO_CONFIG;
  }
  const expected = checksums.get(source.dest);
  if (source["dest-filename"] !== ".cargo-checksum.json" || !expected) return false;
  try {
    return JSON.stringify(JSON.parse(source.contents)) === expected;
  } catch {
    return false;
  }
}

// ---------------------------------------------------------------------------
// npm

/** Whether `dest` is below `prefix`, with no segment that climbs out of it. */
function isBelow(dest, prefix) {
  const rest = String(dest).startsWith(prefix) ? String(dest).slice(prefix.length) : null;
  return rest !== null && rest.split("/").every((part) => part && part !== "." && part !== "..");
}

/** esbuild's platform package, as the tool cache names its folder. */
const ESBUILD_PACKAGE = /^@esbuild\/[a-z0-9-]+@\d+\.\d+\.\d+$/;

/**
 * The shell entry `flatpak-node-generator` writes for an esbuild binary it
 * unpacks: only for an archive whose folder names the lockfile package it is.
 */
function esbuildShell(archive, wanted) {
  const pkg = String(archive.dest).slice(`${ESBUILD_CACHE}/.package/`.length);
  if (
    !String(archive.dest).startsWith(`${ESBUILD_CACHE}/.package/`) ||
    !ESBUILD_PACKAGE.test(pkg) ||
    wanted.get(archive.url)?.id !== pkg
  ) {
    return null;
  }
  return JSON.stringify({
    type: "shell",
    commands: [
      'mkdir -p "bin/@esbuild"',
      `cp ".package/${pkg}/bin/esbuild" "bin/${pkg}"`,
      `ln -sf "${pkg}" "bin/esbuild-current"`,
    ],
    dest: ESBUILD_CACHE,
    "only-arches": archive["only-arches"],
  });
}

/**
 * Whether an inline, script or shell entry is one `flatpak-node-generator`
 * writes for this list: an npm cache index entry, the node-gyp headers script
 * and the command that runs it, or the copy of an esbuild binary the list
 * unpacks (`esbuildShells`).
 */
function isNodeConfigEntry(source, esbuildShells) {
  const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
  switch (source.type) {
    case "inline":
      return (
        hasKeys(source, "contents", "dest", "dest-filename") &&
        /^flatpak-node\/npm-cache\/_cacache\/index-v5\/[0-9a-f]{2}\/[0-9a-f]{2}$/.test(
          source.dest,
        ) &&
        /^[0-9a-f]+$/.test(source["dest-filename"])
      );
    case "script":
      return (
        hasKeys(source, "commands", "dest", "dest-filename") &&
        source.dest === "flatpak-node" &&
        source["dest-filename"] === "setup_sdk_node_headers.sh" &&
        same(source.commands, NODE_HEADERS_SCRIPT)
      );
    case "shell":
      if (hasKeys(source, "commands")) return same(source.commands, [NODE_HEADERS_COMMAND]);
      return (
        hasKeys(source, "commands", "dest", "only-arches") &&
        esbuildShells.has(
          JSON.stringify({
            type: "shell",
            commands: source.commands,
            dest: source.dest,
            "only-arches": source["only-arches"],
          }),
        )
      );
    default:
      return false;
  }
}

/** Where the generator puts a package's tarball in the npm cache (`file` entries). */
const NPM_CONTENT =
  /^flatpak-node\/npm-cache\/_cacache\/content-v2\/sha(512|256|1)\/[0-9a-f]{2}\/[0-9a-f]{2}$/;

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
  const typeProblem = badlyTyped(list, lock, sources);
  if (typeProblem) return typeProblem;
  // One tarball may sit at several places in node_modules: it is one source.
  const wanted = new Map();
  for (const [key, entry] of Object.entries(packages)) {
    if (key === "" || !entry.resolved || !entry.integrity) continue;
    if (!wanted.has(entry.resolved)) {
      wanted.set(entry.resolved, {
        label: `npm package ${npmName(key, entry)}@${entry.version}`,
        id: `${npmName(key, entry)}@${entry.version}`,
        integrity: entry.integrity,
      });
    }
  }

  // flatpak-node-generator unpacks esbuild's platform binary a second time, from
  // the tarball it already lists as a `file`, into its tool cache (011
  // research.md §3): such an archive is the same package, not a second source,
  // and is accepted only when its url and hash are a lockfile package's. Any
  // other archive there is a source like the rest and fails below.
  const isToolCacheCopy = (s) => {
    if (s.type !== "archive" || !isBelow(s.dest, TOOL_CACHE)) return false;
    if (
      !onlyKeys(s, "url", "sha512", "sha256", "sha1", "strip-components", "dest", "only-arches")
    ) {
      return false;
    }
    const entry = wanted.get(s.url);
    const expected = entry && parseIntegrity(entry.integrity);
    return Boolean(expected) && s[expected.algorithm] === expected.hex;
  };
  const downloads = sources.filter((s) => !CONFIG_TYPES.has(s.type) && !isToolCacheCopy(s));
  const esbuildShells = new Set(
    sources
      .filter(isToolCacheCopy)
      .map((archive) => esbuildShell(archive, wanted))
      .filter(Boolean),
  );
  for (const source of sources) {
    if (CONFIG_TYPES.has(source.type) && !isNodeConfigEntry(source, esbuildShells)) {
      return wrongList(list, lock, strayEntry(source));
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
    if (
      source.type !== "file" ||
      !onlyKeys(source, "url", "sha512", "sha256", "sha1", "dest", "dest-filename") ||
      !NPM_CONTENT.test(source.dest) ||
      !/^[0-9a-f]+$/.test(source["dest-filename"])
    ) {
      return wrongList(
        list,
        lock,
        `${wanted.get(source.url).label} is put at ${source.dest}, not in the npm cache`,
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
