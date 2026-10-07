// Checks that a shipped build has none of the embedded WebDriver server
// (tauri-plugin-wdio-webdriver), which E2E builds carry behind the `e2e`
// Cargo feature (#29). The server has no authentication and runs any script
// in the page, so any local process could read an unlocked collection
// through it. The same goes for the E2E-only switches that change what the app
// does (007: HOPLODEX_E2E_PDF_PREVIEW turns PDF preview off), each held out of
// a shipped build by the same feature and checked by its name in the binary.
//
//   node scripts/check-no-webdriver.mjs            the dependency graph only
//   node scripts/check-no-webdriver.mjs BINARY...  also these built binaries
//
// The dependency check is `npm run audit:webdriver`, part of `npm run audit`;
// build-appimage.sh checks the release binary it built. Each check is also run
// the other way round (the `e2e` graph, an E2E binary when one is built) so
// that it can't pass because the crate or its marker was renamed.
//
// Node rather than a shell script, so that npm can run it on Windows too,
// where it runs scripts with cmd.exe (#27). No dependencies of its own.

import { existsSync, readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const crate = "tauri-plugin-wdio-webdriver";
// What an E2E build carries and a shipped one must not: the variable the
// server reads its port from, compiled into it, and the E2E-only switches the
// app reads (007, research.md §4 and ui contract §10). Each is a string in the
// binary only while its `#[cfg(feature = "e2e")]` code is built.
const markers = [
  { name: "TAURI_WEBDRIVER_PORT", what: "the embedded WebDriver server" },
  { name: "HOPLODEX_E2E_PDF_PREVIEW", what: "the E2E switch that turns PDF preview off" },
].map((marker) => ({ ...marker, bytes: Buffer.from(marker.name) }));

function hasCrate(features) {
  const tree = spawnSync(
    "cargo",
    [
      "tree",
      "--manifest-path",
      path.join(repoRoot, "src-tauri", "Cargo.toml"),
      "--edges",
      "normal",
      "--prefix",
      "none",
      "--features",
      features,
    ],
    { encoding: "utf8", maxBuffer: 64 * 1024 * 1024, stdio: ["ignore", "pipe", "inherit"] },
  );
  if (tree.error || tree.status !== 0) {
    console.error(`check-no-webdriver: cargo tree failed (features: ${features})`);
    process.exit(2);
  }
  return tree.stdout.split(/\r?\n/).some((line) => line.startsWith(`${crate} `));
}

const binaries = process.argv.slice(2);
let ok = true;
function fail(message) {
  console.error(`check-no-webdriver: ${message}`);
  ok = false;
}

if (hasCrate("custom-protocol")) {
  fail(`a shipped build (features: custom-protocol) depends on ${crate}`);
}
if (!hasCrate("custom-protocol,e2e")) {
  fail(`an E2E build no longer depends on ${crate}; update this check`);
}

for (const binary of binaries) {
  const contents = readFileSync(binary);
  for (const marker of markers) {
    if (contents.includes(marker.bytes)) {
      fail(`${binary} contains ${marker.what} (${marker.name})`);
    }
  }
}
if (binaries.length > 0) {
  const name = process.platform === "win32" ? "hoplodex.exe" : "hoplodex";
  const e2eBinary = path.join(repoRoot, "src-tauri", "target", "e2e", name);
  if (existsSync(e2eBinary)) {
    const contents = readFileSync(e2eBinary);
    for (const marker of markers) {
      if (!contents.includes(marker.bytes)) {
        fail(`${e2eBinary} has no ${marker.name} either; update this check`);
      }
    }
  }
}

if (!ok) process.exit(1);
const also = binaries.length > 0 ? ` (and in ${binaries.join(" ")})` : "";
console.log(`Checked: no embedded WebDriver server or E2E switch in a shipped build${also}`);
