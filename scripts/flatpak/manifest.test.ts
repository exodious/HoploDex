// @vitest-environment node
// The Flatpak manifest's build rules (011 US3-5, 011 FR-002, 011 FR-019,
// 011 SC-004; 011 data-model.md "The manifest", 011 contracts/flatpak-bundle.md
// "Permissions"). The permissions are a record: adding, dropping or changing
// one changes 011 research.md §4, the contract and this list together. No
// network anywhere, and one module with three sources, is what lets the
// license audit (011 FR-019) and the offline build (011 SC-004) rest on the
// manifest instead of on a promise. Fixtures under `fixtures/manifest-*/` are
// small manifests, each wrong in one way.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import yaml from "js-yaml";
import { describe, expect, it } from "vitest";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, "../..");
const MANIFEST_NAME = "io.github.exodious.HoploDex.yml";
const REAL_MANIFEST = path.join(ROOT, "flatpak", MANIFEST_NAME);

/** 011 contracts/flatpak-bundle.md "Permissions": the twelve, in this order. */
const FINISH_ARGS = [
  "--share=ipc",
  "--socket=wayland",
  "--socket=fallback-x11",
  "--device=dri",
  "--filesystem=home",
  "--filesystem=/media",
  "--filesystem=/run/media",
  "--filesystem=/mnt",
  "--talk-name=org.freedesktop.secrets",
  "--talk-name=org.freedesktop.ScreenSaver",
  "--talk-name=org.gnome.ScreenSaver",
  "--system-talk-name=org.freedesktop.login1",
];

const SDK_EXTENSIONS = [
  "org.freedesktop.Sdk.Extension.node24",
  "org.freedesktop.Sdk.Extension.rust-stable",
];

type Manifest = Record<string, unknown>;

function load(file: string): Manifest {
  if (!fs.existsSync(file)) {
    throw new Error(`${path.relative(ROOT, file)} does not exist yet (011 T019 writes it)`);
  }
  return yaml.load(fs.readFileSync(file, "utf8")) as Manifest;
}
const loadFixture = (name: string) =>
  load(path.join(HERE, "fixtures", `manifest-${name}`, MANIFEST_NAME));

/** Every place a string value `--share=network` sits in the parsed manifest, as a path. */
function findNetwork(node: unknown, where: string, found: string[] = []): string[] {
  if (typeof node === "string") {
    if (node.includes("--share=network")) found.push(where);
  } else if (Array.isArray(node)) {
    node.forEach((item, i) => findNetwork(item, `${where}[${i}]`, found));
  } else if (node && typeof node === "object") {
    for (const [key, value] of Object.entries(node)) {
      findNetwork(value, where ? `${where}.${key}` : key, found);
    }
  }
  return found;
}

const asArray = (value: unknown): unknown[] => (Array.isArray(value) ? value : []);

/** The build rules: every problem the manifest has, in a stable order. */
function buildRuleProblems(manifest: Manifest): string[] {
  const problems: string[] = [];

  const identity: [string, unknown, string][] = [
    ["id", manifest.id, "io.github.exodious.HoploDex"],
    ["runtime", manifest.runtime, "org.gnome.Platform"],
    ["runtime-version", String(manifest["runtime-version"]), "51"],
    ["sdk", manifest.sdk, "org.gnome.Sdk"],
    ["command", manifest.command, "hoplodex"],
  ];
  for (const [key, actual, expected] of identity) {
    if (actual !== expected) problems.push(`${key} must be "${expected}", not "${String(actual)}"`);
  }
  const extensions = asArray(manifest["sdk-extensions"]).map(String).sort();
  if (JSON.stringify(extensions) !== JSON.stringify(SDK_EXTENSIONS)) {
    problems.push(
      `sdk-extensions must be exactly ${SDK_EXTENSIONS.join(" and ")}, not ${JSON.stringify(extensions)}`,
    );
  }

  for (const where of findNetwork(manifest, "")) {
    problems.push(
      `"--share=network" is in the manifest at ${where}: the Flatpak and its build have no network (011 research.md §2, §4)`,
    );
  }

  const finishArgs = asArray(manifest["finish-args"]).map(String);
  for (const arg of finishArgs.filter((a) => !FINISH_ARGS.includes(a))) {
    problems.push(
      `finish-args has "${arg}", which is not one of the twelve permissions recorded in 011 research.md §4 ` +
        "(changing a permission changes that record, 011 contracts/flatpak-bundle.md and this test together)",
    );
  }
  for (const arg of FINISH_ARGS.filter((a) => !finishArgs.includes(a))) {
    problems.push(`finish-args lacks "${arg}", which 011 research.md §4 records`);
  }
  if (
    problems.every((p) => !p.startsWith("finish-args")) &&
    JSON.stringify(finishArgs) !== JSON.stringify(FINISH_ARGS)
  ) {
    problems.push(
      "finish-args are not in the order 011 contracts/flatpak-bundle.md lists them, or repeat one",
    );
  }

  const buildOptions = (manifest["build-options"] ?? {}) as Record<string, unknown>;
  const env = (buildOptions.env ?? {}) as Record<string, unknown>;
  for (const name of ["CARGO_NET_OFFLINE", "npm_config_offline"]) {
    if (String(env[name]) !== "true") {
      problems.push(
        `build-options.env.${name} must be "true" (011 SC-004), not ${JSON.stringify(env[name])}`,
      );
    }
  }
  if (buildOptions["build-args"] !== undefined) {
    problems.push(
      `build-options has build-args ${JSON.stringify(buildOptions["build-args"])}: the build takes none (011 research.md §2)`,
    );
  }

  const modules = asArray(manifest.modules) as Record<string, unknown>[];
  if (modules.length !== 1) {
    problems.push(
      `the manifest has ${modules.length} modules, and it must have exactly one, "hoplodex" (011 FR-019)`,
    );
  }
  const [module] = modules;
  if (module) {
    if (module.name !== "hoplodex" || module.buildsystem !== "simple") {
      problems.push(`the module must be "hoplodex" with buildsystem "simple" (011 research.md §2)`);
    }
    const sources = asArray(module.sources);
    const isDir = (s: unknown) => {
      const source = s as { type?: unknown; path?: unknown } | null;
      return (
        source !== null &&
        typeof source === "object" &&
        source.type === "dir" &&
        source.path === ".."
      );
    };
    const files = sources.filter((s) => typeof s === "string").sort();
    if (
      sources.length !== 3 ||
      sources.filter(isDir).length !== 1 ||
      JSON.stringify(files) !== JSON.stringify(["cargo-sources.json", "node-sources.json"])
    ) {
      problems.push(
        "the module's sources must be exactly { type: dir, path: .. }, cargo-sources.json and node-sources.json " +
          `(011 FR-019), not ${JSON.stringify(sources)}`,
      );
    }
    if (module["build-options"] !== undefined) {
      const moduleArgs = (module["build-options"] as Record<string, unknown>)["build-args"];
      if (moduleArgs !== undefined) {
        problems.push(
          `the module has build-args ${JSON.stringify(moduleArgs)}: the build takes none (011 research.md §2)`,
        );
      }
    }
    const cargo = asArray(module["build-commands"])
      .map(String)
      .filter((c) => /\bcargo build\b/.test(c));
    if (cargo.length !== 1) {
      problems.push(`the module must have exactly one cargo build command, not ${cargo.length}`);
    } else if (
      !/(^|\s)--offline(\s|$)/.test(cargo[0]) ||
      !/(^|\s)--locked(\s|$)/.test(cargo[0]) ||
      !/(^|\s)--features custom-protocol(\s|$)/.test(cargo[0])
    ) {
      problems.push(
        `the cargo build command must have --offline --locked --features custom-protocol (011 research.md §2), not: ${cargo[0]}`,
      );
    }
  }
  return problems;
}

describe("011 US3-5 - the manifest's build rules", () => {
  it("011 US3-5: the real manifest has no problem (red until the manifest is written)", () => {
    expect(buildRuleProblems(load(REAL_MANIFEST))).toEqual([]);
  });

  it("the recorded permissions are the contract's twelve", () => {
    expect(FINISH_ARGS).toHaveLength(12);
    expect(new Set(FINISH_ARGS).size).toBe(12);
  });

  it("a manifest that follows every rule has no problem", () => {
    expect(buildRuleProblems(loadFixture("valid"))).toEqual([]);
  });

  describe("one module; the checkout and the two lists as its only sources", () => {
    it("fails a second module", () => {
      expect(buildRuleProblems(loadFixture("second-module"))).toEqual([
        'the manifest has 2 modules, and it must have exactly one, "hoplodex" (011 FR-019)',
      ]);
    });

    it("fails a fourth source", () => {
      expect(buildRuleProblems(loadFixture("fourth-source"))).toEqual([
        "the module's sources must be exactly { type: dir, path: .. }, cargo-sources.json and node-sources.json " +
          '(011 FR-019), not [{"type":"dir","path":".."},"cargo-sources.json","node-sources.json",{"type":"file","path":"extra.txt"}]',
      ]);
    });

    it("fails a git source", () => {
      expect(buildRuleProblems(loadFixture("git-source"))).toEqual([
        "the module's sources must be exactly { type: dir, path: .. }, cargo-sources.json and node-sources.json " +
          '(011 FR-019), not [{"type":"dir","path":".."},"cargo-sources.json","node-sources.json",' +
          '{"type":"git","url":"https://github.com/exodious/HoploDex.git","tag":"v0.1.0"}]',
      ]);
    });
  });

  describe("no --share=network anywhere", () => {
    const WORDS = (where: string) =>
      `"--share=network" is in the manifest at ${where}: the Flatpak and its build have no network (011 research.md §2, §4)`;

    it("fails it in finish-args, naming the permission", () => {
      expect(buildRuleProblems(loadFixture("network-finish-args"))).toContain(
        WORDS("finish-args[12]"),
      );
    });

    it("fails it in build-options.build-args", () => {
      expect(buildRuleProblems(loadFixture("network-build-args"))).toContain(
        WORDS("build-options.build-args[0]"),
      );
    });

    it("fails it in a module's build-options", () => {
      expect(buildRuleProblems(loadFixture("network-module-build-options"))).toContain(
        WORDS("modules[0].build-options.build-args[0]"),
      );
    });
  });

  describe("finish-args equal the recorded list", () => {
    it("fails a permission added, naming it and the record", () => {
      expect(buildRuleProblems(loadFixture("finish-arg-added"))).toEqual([
        'finish-args has "--socket=pulseaudio", which is not one of the twelve permissions recorded in 011 research.md §4 ' +
          "(changing a permission changes that record, 011 contracts/flatpak-bundle.md and this test together)",
      ]);
    });

    it("fails a permission removed, naming it and the record", () => {
      expect(buildRuleProblems(loadFixture("finish-arg-removed"))).toEqual([
        'finish-args lacks "--device=dri", which 011 research.md §4 records',
      ]);
    });

    it("fails --filesystem=host, naming it and the one it replaced", () => {
      expect(buildRuleProblems(loadFixture("finish-arg-host"))).toEqual([
        'finish-args has "--filesystem=host", which is not one of the twelve permissions recorded in 011 research.md §4 ' +
          "(changing a permission changes that record, 011 contracts/flatpak-bundle.md and this test together)",
        'finish-args lacks "--filesystem=home", which 011 research.md §4 records',
      ]);
    });

    it("fails the right permissions in the wrong order", () => {
      expect(buildRuleProblems(loadFixture("finish-arg-order"))).toEqual([
        "finish-args are not in the order 011 contracts/flatpak-bundle.md lists them, or repeat one",
      ]);
    });
  });

  describe("the build is offline", () => {
    it("fails build-options without CARGO_NET_OFFLINE and npm_config_offline set to true", () => {
      expect(buildRuleProblems(loadFixture("online-env"))).toEqual([
        'build-options.env.CARGO_NET_OFFLINE must be "true" (011 SC-004), not "false"',
        'build-options.env.npm_config_offline must be "true" (011 SC-004), not undefined',
      ]);
    });

    it("fails a cargo command without --offline --locked --features custom-protocol", () => {
      expect(buildRuleProblems(loadFixture("cargo-online"))).toEqual([
        "the cargo build command must have --offline --locked --features custom-protocol (011 research.md §2), not: " +
          "cd src-tauri && cargo build --release --features custom-protocol --bin hoplodex",
      ]);
    });
  });
});
