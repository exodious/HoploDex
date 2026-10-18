// npm license audit: `npm run audit:licenses` (with cargo-deny's license
// check for the Rust side). See "License audit" in DEVELOPMENT.md.
//
// Reads package-lock.json and checks every package that isn't dev-only (`dev`
// or `devOptional`), since those are what Vite bundles into the shipped app.
// Each one's SPDX license expression must be satisfiable from the policy's
// ALLOWED, the GPLv3-compatible licenses that src-tauri/deny.toml also allows
// (scripts/licenses/policy.mjs, shared with the notices generator).
// devDependencies aren't distributed, so their licenses don't affect releasing
// under GPL-3.0; `--all` lists them anyway without failing.
//
// No dependencies of its own, so the audit doesn't grow the tree it audits.

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { isAllowed } from "./licenses/policy.mjs";
import { npmPackages } from "./licenses/shipped.mjs";
import { satisfies } from "./licenses/spdx.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

const lock = JSON.parse(readFileSync(path.join(repoRoot, "package-lock.json"), "utf8"));
const showAll = process.argv.includes("--all");
const failures = [];
const counts = new Map();

for (const pkg of npmPackages(lock)) {
  const { name, license } = pkg;
  const id = `${name}@${pkg.version}`;
  const label = typeof license === "string" ? license : JSON.stringify(license ?? null);
  const scope = pkg.development ? "dev" : "shipped";
  if (pkg.development && !showAll) continue;
  const key = `${scope}\t${label}`;
  counts.set(key, (counts.get(key) ?? 0) + 1);
  if (pkg.development) continue;

  let ok = false;
  try {
    ok =
      typeof license === "string" && satisfies(license, (lic) => isAllowed(lic, { package: name }));
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
