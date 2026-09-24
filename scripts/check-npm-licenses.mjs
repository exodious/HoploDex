// npm license audit: `npm run audit:licenses` (with cargo-deny's license
// check for the Rust side). See "License audit" in DEVELOPMENT.md.
//
// Reads package-lock.json and checks every package that isn't dev-only, since
// those are what Vite bundles into the shipped app. Each one's SPDX license
// expression must be satisfiable from ALLOWED, the GPLv3-compatible licenses
// that src-tauri/deny.toml also allows. devDependencies aren't distributed,
// so their licenses don't affect releasing under GPL-3.0; `--all` lists them
// anyway without failing.
//
// No dependencies of its own, so the audit doesn't grow the tree it audits.

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

// Keep in step with [licenses] allow in src-tauri/deny.toml.
const ALLOWED = new Set([
  "0BSD",
  "Apache-2.0",
  "Apache-2.0 WITH LLVM-exception",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "BSL-1.0",
  "CC0-1.0",
  "GPL-3.0-only",
  "GPL-3.0-or-later",
  "ISC",
  "LGPL-2.1-or-later",
  "LGPL-3.0-only",
  "LGPL-3.0-or-later",
  "MIT",
  "MIT-0",
  "MPL-2.0",
  "Unicode-3.0",
  "Unlicense",
  "Zlib",
]);

// Licenses allowed only for particular packages, with the reason.
const EXCEPTIONS = [
  {
    // The OFL isn't GPL-compatible for code, but fonts ship as separate
    // .woff2 files, not combined with the program, and the OFL allows bundling
    // them with any software. The OFL notice must ship with them.
    license: "OFL-1.1",
    packages: /^@fontsource(-variable)?\//,
  },
];

// Evaluates an SPDX expression (OR, AND, WITH, parentheses) against a
// predicate for single license identifiers.
function satisfies(expression, isAllowed) {
  const tokens = expression.match(/\(|\)|[^\s()]+/g) ?? [];
  let pos = 0;
  const peek = () => tokens[pos];
  function primary() {
    if (peek() === "(") {
      pos++;
      const value = or();
      if (tokens[pos++] !== ")") throw new Error("unbalanced parentheses");
      return value;
    }
    let id = tokens[pos++];
    if (id === undefined) throw new Error("unexpected end");
    if (peek() === "WITH") {
      pos++;
      id = `${id} WITH ${tokens[pos++]}`;
    }
    return isAllowed(id);
  }
  function and() {
    let value = primary();
    while (peek() === "AND") {
      pos++;
      value = primary() && value;
    }
    return value;
  }
  function or() {
    let value = and();
    while (peek() === "OR") {
      pos++;
      value = and() || value;
    }
    return value;
  }
  const result = or();
  if (pos !== tokens.length) throw new Error(`unexpected "${peek()}"`);
  return result;
}

const lock = JSON.parse(readFileSync(path.join(repoRoot, "package-lock.json"), "utf8"));
const showAll = process.argv.includes("--all");
const failures = [];
const counts = new Map();

for (const [location, pkg] of Object.entries(lock.packages)) {
  if (location === "") continue;
  const name = pkg.name ?? location.slice(location.lastIndexOf("node_modules/") + 13);
  const id = `${name}@${pkg.version}`;
  const license = typeof pkg.license === "string" ? pkg.license : undefined;
  const label = license ?? JSON.stringify(pkg.license ?? null);
  const scope = pkg.dev ? "dev" : "shipped";
  if (pkg.dev && !showAll) continue;
  const key = `${scope}\t${label}`;
  counts.set(key, (counts.get(key) ?? 0) + 1);
  if (pkg.dev) continue;

  const isAllowed = (lic) =>
    ALLOWED.has(lic) || EXCEPTIONS.some((e) => e.license === lic && e.packages.test(name));
  let ok = false;
  try {
    ok = license !== undefined && satisfies(license, isAllowed);
  } catch (error) {
    failures.push(`${id}: can't parse "${license}" (${error.message})`);
    continue;
  }
  if (!ok) failures.push(`${id}: ${label}`);
}

for (const [key, count] of [...counts].sort()) {
  const [scope, label] = key.split("\t");
  console.log(`${scope.padEnd(8)}${String(count).padStart(5)}  ${label}`);
}
if (failures.length > 0) {
  console.error(`\n${failures.length} shipped npm package(s) without a GPLv3-compatible license:`);
  for (const failure of failures) console.error(`  ${failure}`);
  process.exit(1);
}
console.log("\nnpm licenses ok: every shipped package is GPLv3-compatible.");
