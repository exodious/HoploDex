// The check variant's manifest (011 data-model.md "The check manifest", 011
// research.md §16, 011 FR-021): the shipped manifest with three changes, so the
// automated Flatpak check runs the app with the permissions the Flatpak ships
// with and a build that carries the WebDriver server.
//
//   - the cargo build command's profile and features (`--profile e2e`,
//     `--features custom-protocol,e2e`, no `--release`);
//   - the binary's path that follows from the profile (`target/e2e/`);
//   - no `check-no-webdriver.mjs` step, which fails on the `e2e` build's driver
//     by design (#29).
//
// `deriveCheckManifest` is pure. manifest.test.ts compares its result with the
// shipped manifest and fails on any other difference. The CLI below writes the
// derived manifest into the state volume (never into the checkout, and never
// committed): `node scripts/flatpak/checkManifest.mjs` prints the file's path.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import yaml from "js-yaml";

export const MANIFEST_NAME = "io.github.exodious.HoploDex.yml";

/**
 * The check manifest for `manifest`. Does not change the object it is given.
 * @param {Record<string, any>} manifest the parsed shipped manifest
 * @returns {Record<string, any>}
 */
export function deriveCheckManifest(manifest) {
  const check = structuredClone(manifest);
  for (const module of Array.isArray(check.modules) ? check.modules : []) {
    if (!Array.isArray(module["build-commands"])) continue;
    module["build-commands"] = module["build-commands"]
      .map(String)
      .filter((command) => !command.includes("check-no-webdriver.mjs"))
      .map((command) =>
        /\bcargo build\b/.test(command)
          ? command
              .replace("--release", "--profile e2e")
              .replace("--features custom-protocol", "--features custom-protocol,e2e")
          : command.replaceAll("target/release/hoplodex", "target/e2e/hoplodex"),
      );
  }
  return check;
}

/**
 * The derived manifest as it is written to the state volume. flatpak-builder
 * resolves a manifest's relative paths from the manifest's own folder, and the
 * state volume is not the checkout, so the module's `dir` source and the two
 * source lists are named by absolute path. This is the writer's business, not
 * the derivation's: the object `deriveCheckManifest` returns still differs from
 * the shipped one only where the rule allows.
 * @param {Record<string, any>} check
 * @param {string} shippedFolder the folder holding the shipped manifest
 * @returns {Record<string, any>}
 */
export function withAbsoluteSources(check, shippedFolder) {
  const out = structuredClone(check);
  for (const module of Array.isArray(out.modules) ? out.modules : []) {
    if (!Array.isArray(module.sources)) continue;
    module.sources = module.sources.map((source) => {
      if (typeof source === "string") return path.resolve(shippedFolder, source);
      if (source && source.type === "dir" && typeof source.path === "string") {
        return { ...source, path: path.resolve(shippedFolder, source.path) };
      }
      return source;
    });
  }
  return out;
}

/**
 * Reads the shipped manifest in `flatpakFolder`, writes the check manifest to
 * `<stateFolder>/check/` and returns the file's path.
 * @param {{ flatpakFolder: string, stateFolder: string }} folders
 * @returns {string}
 */
export function writeCheckManifest({ flatpakFolder, stateFolder }) {
  const shipped = yaml.load(fs.readFileSync(path.join(flatpakFolder, MANIFEST_NAME), "utf8"));
  const check = withAbsoluteSources(deriveCheckManifest(shipped), flatpakFolder);
  const folder = path.join(stateFolder, "check");
  fs.mkdirSync(folder, { recursive: true });
  const file = path.join(folder, MANIFEST_NAME);
  fs.writeFileSync(
    file,
    `# The Flatpak check's manifest, derived from flatpak/${MANIFEST_NAME} by\n` +
      "# scripts/flatpak/checkManifest.mjs (011 research.md §16). Generated; never committed.\n" +
      yaml.dump(check, { lineWidth: -1, noRefs: true }),
  );
  return file;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const here = path.dirname(fileURLToPath(import.meta.url));
  const state = process.env.HOPLODEX_FLATPAK_STATE;
  if (!state) {
    process.stderr.write(
      "checkManifest: HOPLODEX_FLATPAK_STATE is not set; run through scripts/dev-container.sh --flatpak.\n",
    );
    process.exit(1);
  }
  try {
    const file = writeCheckManifest({
      flatpakFolder: path.resolve(here, "../../flatpak"),
      stateFolder: state,
    });
    process.stdout.write(`${file}\n`);
  } catch (error) {
    process.stderr.write(`checkManifest: ${error.message}\n`);
    process.exit(1);
  }
}
