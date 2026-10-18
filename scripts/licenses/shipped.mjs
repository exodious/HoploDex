// One definition of "shipped" for the audit and the notices (010 research.md
// §5, §6): the npm packages from the lockfile (here) and the crates from
// `cargo tree`.

import { execFileSync } from "node:child_process";
import path from "node:path";

/**
 * Every package in a parsed package-lock.json except the root, with whether it
 * is development-only. A package the lockfile marks `devOptional` (an optional
 * peer installed for type checking, such as `@types/react`) is development-only
 * too: it is never bundled (010 research.md §6).
 */
export function npmPackages(lock) {
  const packages = [];
  for (const [location, pkg] of Object.entries(lock.packages)) {
    if (location === "") continue;
    packages.push({
      name: pkg.name ?? location.slice(location.lastIndexOf("node_modules/") + 13),
      version: pkg.version,
      location,
      license: pkg.license,
      development: Boolean(pkg.dev || pkg.devOptional),
    });
  }
  return packages;
}

/** The npm packages that ship in the interface: everything not development-only. */
export function npmShipped(lock) {
  return npmPackages(lock).filter((pkg) => !pkg.development);
}

/** Path-like source in `{p}`'s parenthesised part: `/home/x/src-tauri`, `C:\x\src-tauri`, `\\host\x`. */
const PATH_SOURCE = /^(?:[A-Za-z]:[\\/]|[\\/])/;

/** The `cc`/`cmake` build dependencies that a crate's build script can compile C with. */
const NATIVE_BUILD_TOOLS = ["cc", "cmake"];

function manifestArgs(root) {
  return ["--locked", "--manifest-path", path.join(root, "src-tauri", "Cargo.toml")];
}

/** Runs `cargo` in `root` and returns its stdout; the runner the CLI and the build use. */
export function runCargo(root) {
  return (args) =>
    execFileSync("cargo", args, {
      cwd: root,
      encoding: "utf8",
      maxBuffer: 512 * 1024 * 1024,
      stdio: ["ignore", "pipe", "pipe"],
    });
}

/**
 * Parses `cargo tree --prefix none -f '{p}'` or `-f '{p} {f}'` output into
 * `{ name, version, paths, features }` lines: `name v1.0.0 (source) feat1,feat2 (*)`.
 */
function treeLines(output) {
  const lines = [];
  for (const raw of output.split(/\r?\n/)) {
    const line = raw.trim();
    if (line === "") continue;
    const m = /^(\S+) v(\S+)(?: (.*))?$/.exec(line);
    if (!m) throw new Error(`cargo tree: can't read the line "${line}"`);
    const rest = m[3] ?? "";
    const groups = [...rest.matchAll(/\(([^)]*)\)/g)].map((g) => g[1]);
    const features = rest.replace(/\([^)]*\)/g, " ").trim();
    lines.push({
      name: m[1],
      version: m[2],
      paths: groups.filter((g) => PATH_SOURCE.test(g)),
      features: features === "" ? [] : features.split(","),
    });
  }
  return lines;
}

/**
 * The enabled `cc`/`cmake` build dependencies, per crate name and version, for
 * `triple`: `cargo tree -e normal,build --depth 1 --invert cc --invert cmake`
 * lists each tool and then the crates that depend on it directly. (`-e build`
 * alone follows only the root's own build edges, so it finds nothing.)
 */
function nativeBuildTools(cargo, root, triple) {
  const tree = (tools) =>
    cargo([
      "tree",
      ...manifestArgs(root),
      "-e",
      "normal,build",
      "--target",
      triple,
      "--features",
      "custom-protocol",
      "--depth",
      "1",
      ...tools.flatMap((t) => ["--invert", t]),
      "--prefix",
      "none",
      "-f",
      "{p}",
    ]);
  let outputs;
  try {
    outputs = [tree(NATIVE_BUILD_TOOLS)];
  } catch (error) {
    // cargo refuses to invert a package the graph doesn't have (no crate here
    // uses cmake): ask for each tool alone and skip the ones that aren't there.
    if (!/did not match any packages/.test(String(error?.message))) throw error;
    outputs = [];
    for (const tool of NATIVE_BUILD_TOOLS) {
      try {
        outputs.push(tree([tool]));
      } catch (inner) {
        if (!/did not match any packages/.test(String(inner?.message))) throw inner;
      }
    }
  }
  const tools = new Map();
  for (const output of outputs) {
    let current;
    for (const line of treeLines(output)) {
      if (NATIVE_BUILD_TOOLS.includes(line.name)) {
        current = line.name;
        continue;
      }
      if (current === undefined) continue;
      const key = `${line.name}@${line.version}`;
      const set = tools.get(key) ?? new Set();
      set.add(current);
      tools.set(key, set);
    }
  }
  return tools;
}

/**
 * The crates that ship for `triple` (010 research.md §5): the normal
 * dependencies of the app, from `cargo tree`'s own feature resolution, never
 * `cargo metadata`'s resolve, which merges build and development edges and
 * over-counts. `hoplodex` is left out; any other path package is a failure.
 *
 * `cargo` is the runner, `(args) => stdout`, with `args` the arguments after
 * `cargo`; `root` is the repository root. Returns `{ crates, failures }`:
 * `crates` sorted by name and version, each `{ name, version, folder, license,
 * features, nativeBuild }` with `folder` from `cargo metadata`'s
 * `manifest_path`, `features` the crate's enabled features for `triple` (from
 * the same `cargo tree` call, `-f '{p} {f}'`: `cargo metadata`'s resolve unions
 * the features of every target and of development dependencies) and
 * `nativeBuild` the `cc`/`cmake` build dependencies enabled for `triple`.
 */
export function crateShipped(triple, { cargo, root }) {
  const failures = [];
  // The same fetch `cargo build` makes a moment later; nothing once the sources are there.
  cargo(["fetch", ...manifestArgs(root), "--target", triple]);
  const tree = cargo([
    "tree",
    ...manifestArgs(root),
    "-e",
    "normal,no-proc-macro",
    "--target",
    triple,
    "--features",
    "custom-protocol",
    "--prefix",
    "none",
    "-f",
    "{p} {f}",
  ]);
  const shipped = new Map();
  for (const line of treeLines(tree)) {
    if (line.name === "hoplodex") continue;
    if (line.paths.length > 0) {
      failures.push(
        `crate ${line.name} ${line.version} (${triple}): a path package ships in the app; only crates from a registry may.`,
      );
      continue;
    }
    const key = `${line.name}@${line.version}`;
    const seen = shipped.get(key);
    shipped.set(key, {
      ...line,
      features: [...new Set([...(seen?.features ?? []), ...line.features])],
    });
  }

  const metadata = JSON.parse(cargo(["metadata", ...manifestArgs(root), "--format-version", "1"]));
  const byKey = new Map(metadata.packages.map((p) => [`${p.name}@${p.version}`, p]));
  const tools = nativeBuildTools(cargo, root, triple);

  const crates = [];
  for (const [key, { name, version, features }] of shipped) {
    const pkg = byKey.get(key);
    if (!pkg) {
      failures.push(`crate ${name} ${version} (${triple}): not in cargo metadata's package list.`);
      continue;
    }
    crates.push({
      name,
      version,
      folder: path.dirname(pkg.manifest_path),
      license: pkg.license ?? undefined,
      features: features.sort(),
      nativeBuild: [...(tools.get(key) ?? [])].sort(),
    });
  }
  crates.sort((a, b) =>
    a.name < b.name ? -1 : a.name > b.name ? 1 : a.version < b.version ? -1 : 1,
  );
  return { crates, failures };
}

/**
 * The target the notices are for (010 research.md §5): `--target`, else
 * `TAURI_ENV_TARGET_TRIPLE` (Tauri sets it for `beforeBuildCommand`), else the
 * host of `rustc -vV`.
 */
export function targetTriple({
  argv = process.argv.slice(2),
  env = process.env,
  rustc = () => execFileSync("rustc", ["-vV"], { encoding: "utf8" }),
} = {}) {
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--target" && argv[i + 1]) return argv[i + 1];
    if (argv[i].startsWith("--target=")) return argv[i].slice("--target=".length);
  }
  if (env.TAURI_ENV_TARGET_TRIPLE) return env.TAURI_ENV_TARGET_TRIPLE;
  const host = /^host:\s*(\S+)/m.exec(rustc());
  if (!host) throw new Error("rustc -vV printed no host triple");
  return host[1];
}
