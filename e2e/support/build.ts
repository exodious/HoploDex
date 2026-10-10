import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { buildProfile } from "./app";

/**
 * The E2E build, shared by the test runner (`onPrepare` in wdio.conf.ts) and
 * the `wdio session` launcher (e2e/session.ts), so both drive the same
 * binary.
 *
 * `cargo build --profile e2e --features custom-protocol,e2e`
 * (custom-protocol makes the app load bundled frontendDist assets, matching
 * what `cargo tauri build` does, instead of trying to hit the devUrl dev
 * server; `e2e` is the embedded WebDriver server, the in-memory keyring and
 * the other E2E-only settings). The `e2e` profile is the release one without
 * fat LTO, so a backend change rebuilds in seconds. HOPLODEX_E2E_PROFILE=release
 * builds and drives the shipping profile instead, as a release check
 * (DEVELOPMENT.md, "Test"). The binary embeds whatever is in `dist/`, so
 * `npm run build` comes first.
 */

export const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

const cargoArgs = [
  "--profile",
  buildProfile,
  "--features",
  // e2e compiles in the WebDriver server. It implies mock-keyring, which
  // swaps the OS keyring for an in-memory store, for saved passphrases:
  // headless environments have no way to unlock a real one. It also lets a
  // spec shorten the idle lock's minute.
  "custom-protocol,e2e",
  "--manifest-path",
  "src-tauri/Cargo.toml",
];

/** The human-testing seed (src-tauri/examples/human_seed.rs), built with the
 * app's profile and features by `buildApp`. */
export const seedBinary = path.resolve(
  repoRoot,
  `src-tauri/target/${buildProfile}/examples/human_seed` +
    (process.platform === "win32" ? ".exe" : ""),
);

/** Builds the app and the human-testing seed with cargo, showing its output.
 * Returns whether the build succeeded. */
export function buildApp(): boolean {
  const build = spawnSync(
    "cargo",
    ["build", ...cargoArgs, "--bin", "hoplodex", "--example", "human_seed"],
    { cwd: repoRoot, stdio: "inherit" },
  );
  return build.status === 0;
}
