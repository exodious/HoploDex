// @vitest-environment node
// The Flatpak manifest's build rules (011 US3-5, 011 FR-002, 011 FR-019,
// 011 SC-004; 011 data-model.md "The manifest", 011 contracts/flatpak-bundle.md
// "Permissions"). The permissions are a record: adding, dropping or changing
// one changes 011 research.md §4, the contract and this list together. No
// network anywhere, and one module with three sources, is what lets the
// license audit (011 FR-019) and the offline build (011 SC-004) rest on the
// manifest instead of on a promise. Fixtures under `fixtures/manifest-*/` are
// small manifests, each wrong in one way. The check variant (011 US5-2,
// 011 FR-021; 011 data-model.md "The check manifest", 011 research.md §16) is
// the shipped manifest with three allowed changes and nothing else.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import yaml from "js-yaml";
import { describe, expect, it } from "vitest";
import * as checkManifest from "./checkManifest.mjs";

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

/** What `checkManifest.mjs` exports (011 T029): a pure derivation of the check manifest. */
const deriveCheckManifest = (manifest: Manifest): Manifest => {
  const derive = (checkManifest as { deriveCheckManifest?: (m: Manifest) => Manifest })
    .deriveCheckManifest;
  if (typeof derive !== "function") {
    throw new Error("checkManifest.mjs does not export deriveCheckManifest yet (011 T029)");
  }
  return derive(structuredClone(manifest));
};

/**
 * The check manifest the rule allows, written out independently of
 * `checkManifest.mjs`: the cargo build command's profile and features, the
 * binary's path that follows from the profile, and no `check-no-webdriver.mjs`
 * step. Everything else is the shipped manifest's.
 */
function allowedCheckManifest(shipped: Manifest): Manifest {
  const check = structuredClone(shipped);
  for (const module of asArray(check.modules) as Record<string, unknown>[]) {
    module["build-commands"] = asArray(module["build-commands"])
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

const show = (value: unknown) => (value === undefined ? "nothing" : JSON.stringify(value));

/** Every place the check manifest differs from what the rule allows, as lines naming where. */
function diffs(expected: unknown, got: unknown, where: string, found: string[] = []): string[] {
  if (Array.isArray(expected) && Array.isArray(got)) {
    if (expected.length !== got.length) {
      const left = expected.map((e) => JSON.stringify(e));
      const right = got.map((g) => JSON.stringify(g));
      for (const item of left.filter((e) => !right.includes(e))) {
        found.push(`${where}: the check manifest lacks ${item}`);
      }
      for (const item of right.filter((g) => !left.includes(g))) {
        found.push(`${where}: the check manifest adds ${item}`);
      }
      if (left.every((e) => right.includes(e)) && right.every((g) => left.includes(g))) {
        found.push(`${where}: the check manifest repeats an entry`);
      }
    } else {
      expected.forEach((item, i) => diffs(item, got[i], `${where}[${i}]`, found));
    }
  } else if (
    expected &&
    got &&
    typeof expected === "object" &&
    typeof got === "object" &&
    !Array.isArray(expected) &&
    !Array.isArray(got)
  ) {
    const keys = new Set([...Object.keys(expected), ...Object.keys(got)]);
    for (const key of keys) {
      diffs(
        (expected as Record<string, unknown>)[key],
        (got as Record<string, unknown>)[key],
        where ? `${where}.${key}` : key,
        found,
      );
    }
  } else if (JSON.stringify(expected) !== JSON.stringify(got)) {
    found.push(`${where}: the rule allows ${show(expected)}, the check manifest has ${show(got)}`);
  }
  return found;
}

/** The check manifest's problems: every difference from the shipped one the rule doesn't allow. */
const checkManifestProblems = (shipped: Manifest, check: Manifest) =>
  diffs(allowedCheckManifest(shipped), check, "");

const checkFixture = (name: string) => loadFixture(`check-${name}`);

describe("011 US5-2 - the check manifest differs from the shipped one only where the rule allows", () => {
  it("011 FR-021: checkManifest.mjs's derivation of the real manifest differs only in the allowed ways", () => {
    const shipped = load(REAL_MANIFEST);
    expect(checkManifestProblems(shipped, deriveCheckManifest(shipped))).toEqual([]);
  });

  it("011 FR-021: the derivation does not change the manifest it is given", () => {
    const shipped = load(REAL_MANIFEST);
    const before = structuredClone(shipped);
    deriveCheckManifest(shipped);
    expect(shipped).toEqual(before);
  });

  it("the real derivation builds with the e2e profile and features, from the e2e profile's binary, with no webdriver check", () => {
    const check = deriveCheckManifest(load(REAL_MANIFEST));
    const commands = (check.modules as Record<string, unknown>[])[0]["build-commands"] as string[];
    const cargo = commands.filter((c) => /\bcargo build\b/.test(c));
    expect(cargo).toHaveLength(1);
    expect(cargo[0]).toMatch(/(^|\s)--profile e2e(\s|$)/);
    expect(cargo[0]).toMatch(/(^|\s)--features custom-protocol,e2e(\s|$)/);
    expect(cargo[0]).not.toMatch(/--release/);
    expect(commands.some((c) => c.includes("check-no-webdriver.mjs"))).toBe(false);
    expect(commands.some((c) => c.includes("target/release/"))).toBe(false);
    expect(commands.some((c) => c.includes("target/e2e/hoplodex /app/bin/hoplodex"))).toBe(true);
  });

  it("a derivation made the allowed way has no problem", () => {
    expect(checkManifestProblems(loadFixture("valid"), checkFixture("ok"))).toEqual([]);
  });

  it("the allowed changes are what the independent derivation makes of the fixture", () => {
    expect(allowedCheckManifest(loadFixture("valid"))).toEqual(checkFixture("ok"));
  });

  it("fails a derivation that changed nothing, naming the cargo command", () => {
    const shipped = loadFixture("valid");
    const problems = checkManifestProblems(shipped, shipped);
    expect(problems.length).toBeGreaterThan(0);
    expect(problems.join("\n")).toContain("modules[0].build-commands:");
  });

  it("fails a derivation that adds a finish-args entry, naming finish-args", () => {
    expect(checkManifestProblems(loadFixture("valid"), checkFixture("finish-arg-added"))).toEqual([
      'finish-args: the check manifest adds "--share=network"',
    ]);
  });

  it("fails a derivation that drops another build step, naming the build command", () => {
    expect(checkManifestProblems(loadFixture("valid"), checkFixture("step-dropped"))).toEqual([
      'modules[0].build-commands: the check manifest lacks "npm run build"',
    ]);
  });

  it("fails a derivation that changes a source, naming the module's sources", () => {
    expect(checkManifestProblems(loadFixture("valid"), checkFixture("source-changed"))).toEqual([
      'modules[0].sources[2]: the rule allows "node-sources.json", the check manifest has "node-sources-other.json"',
    ]);
  });
});

// 011 FR-025, 011 FR-026, 011 US4-5; 011 research.md §4, §14. flathub.json
// keeps the file x86-64 only for a later submission; the linter's exceptions
// file holds exactly the two warnings 011 research.md §4 keeps, each with its
// reason. The format is the linter's own for local exceptions
// ({ "<app id>": { "<rule id>": "<reason>" } }), confirmed on the first
// real run (011 T066).
const APP_ID = "io.github.exodious.HoploDex";
const KEPT_EXCEPTIONS = [
  "finish-args-home-filesystem-access",
  "finish-args-login1-system-talk-name",
];

function loadJson(file: string): unknown {
  if (!fs.existsSync(file)) {
    throw new Error(`${path.relative(ROOT, file)} does not exist yet`);
  }
  return JSON.parse(fs.readFileSync(file, "utf8"));
}
const fixtureJson = (name: string, file: string) =>
  loadJson(path.join(HERE, "fixtures", name, file));

/** flathub.json's problems (011 FR-025). */
function flathubProblems(flathub: unknown): string[] {
  const problems: string[] = [];
  const object = flathub && typeof flathub === "object" ? (flathub as Record<string, unknown>) : {};
  for (const key of Object.keys(object)) {
    if (key !== "only-arches") problems.push(`flathub.json: the key ${key} is not allowed`);
  }
  if (JSON.stringify(object["only-arches"]) !== JSON.stringify(["x86_64"])) {
    problems.push(
      `flathub.json: only-arches must be ["x86_64"], it is ${show(object["only-arches"])}`,
    );
  }
  return problems;
}

/** linter-exceptions.json's problems (011 FR-026). */
function linterExceptionsProblems(exceptions: unknown): string[] {
  const problems: string[] = [];
  const top =
    exceptions && typeof exceptions === "object" ? (exceptions as Record<string, unknown>) : {};
  for (const app of Object.keys(top)) {
    if (app !== APP_ID) problems.push(`linter-exceptions.json: the app id ${app} is not ${APP_ID}`);
  }
  const own = top[APP_ID];
  if (!own || typeof own !== "object") {
    problems.push(`linter-exceptions.json: there is no entry for ${APP_ID}`);
    return problems;
  }
  const rules = own as Record<string, unknown>;
  for (const rule of Object.keys(rules)) {
    if (!KEPT_EXCEPTIONS.includes(rule)) {
      problems.push(
        `linter-exceptions.json: the exception ${rule} is not one 011 research.md §4 keeps`,
      );
    }
  }
  for (const rule of KEPT_EXCEPTIONS) {
    const reason = rules[rule];
    if (reason === undefined) {
      problems.push(`linter-exceptions.json: the exception ${rule} is missing`);
    } else if (typeof reason !== "string" || reason.trim() === "") {
      problems.push(`linter-exceptions.json: the exception ${rule} has no reason`);
    } else if (!/research\.md \u00a74/.test(reason)) {
      problems.push(
        `linter-exceptions.json: the reason for ${rule} does not cite 011 research.md §4`,
      );
    }
  }
  return problems;
}

describe("011 US4-5 - flathub.json and the linter's exceptions", () => {
  it("011 FR-025: the real flathub.json is exactly only-arches x86_64", () => {
    const flathub = loadJson(path.join(ROOT, "flatpak", "flathub.json"));
    expect(flathub).toEqual({ "only-arches": ["x86_64"] });
    expect(flathubProblems(flathub)).toEqual([]);
  });

  it("011 FR-026: the real exceptions file holds exactly the two kept exceptions, each with a reason citing 011 research.md §4", () => {
    const exceptions = loadJson(path.join(ROOT, "flatpak", "linter-exceptions.json"));
    expect(linterExceptionsProblems(exceptions)).toEqual([]);
    expect(Object.keys(exceptions as object)).toEqual([APP_ID]);
    expect(Object.keys((exceptions as Record<string, object>)[APP_ID]).sort()).toEqual(
      KEPT_EXCEPTIONS,
    );
  });

  it("passes the valid fixtures", () => {
    expect(flathubProblems(fixtureJson("flathub-ok", "flathub.json"))).toEqual([]);
    expect(linterExceptionsProblems(fixtureJson("linter-ok", "linter-exceptions.json"))).toEqual(
      [],
    );
  });

  it("fails a flathub.json that adds an architecture, naming only-arches", () => {
    expect(flathubProblems(fixtureJson("flathub-extra-arch", "flathub.json"))).toEqual([
      'flathub.json: only-arches must be ["x86_64"], it is ["x86_64","aarch64"]',
    ]);
  });

  it("fails a flathub.json with another key, naming it", () => {
    expect(flathubProblems(fixtureJson("flathub-extra-key", "flathub.json"))).toEqual([
      "flathub.json: the key skip-icons-check is not allowed",
    ]);
  });

  it("fails a third exception, naming it", () => {
    expect(
      linterExceptionsProblems(fixtureJson("linter-third-exception", "linter-exceptions.json")),
    ).toEqual([
      "linter-exceptions.json: the exception finish-args-flatpak-spawn-access is not one 011 research.md §4 keeps",
    ]);
  });

  it("fails an empty reason, naming the exception", () => {
    expect(
      linterExceptionsProblems(fixtureJson("linter-empty-reason", "linter-exceptions.json")),
    ).toEqual([
      "linter-exceptions.json: the exception finish-args-home-filesystem-access has no reason",
    ]);
  });

  it("fails a reason that does not cite 011 research.md §4, naming the exception", () => {
    expect(
      linterExceptionsProblems(fixtureJson("linter-no-citation", "linter-exceptions.json")),
    ).toEqual([
      "linter-exceptions.json: the reason for finish-args-login1-system-talk-name does not cite 011 research.md §4",
    ]);
  });

  it("fails exceptions under another app id", () => {
    expect(
      linterExceptionsProblems(fixtureJson("linter-wrong-app", "linter-exceptions.json")),
    ).toEqual([
      `linter-exceptions.json: the app id io.github.exodious.Other is not ${APP_ID}`,
      `linter-exceptions.json: there is no entry for ${APP_ID}`,
    ]);
  });
});
