// @vitest-environment node
// What the CI composite actions guard: the npm every job and the dev container
// install (code scanning alert build-ci/cwe-829), and the paths whose pull
// requests run the Flatpak check (code scanning alert build-ci/cwe-693,
// 011 FR-021).
import fs from "node:fs";
import path from "node:path";
import yaml from "js-yaml";
import { describe, expect, it } from "vitest";

const ROOT = path.resolve(import.meta.dirname, "..");
const read = (file: string) => fs.readFileSync(path.join(ROOT, file), "utf8");

type Step = { id?: string; run?: string; env?: Record<string, string> };
type Action = { runs: { steps?: Step[] } };
type Workflow = { jobs: Record<string, { steps?: Step[] }> };

const actionFiles = fs
  .readdirSync(path.join(ROOT, ".github/actions"))
  .map((dir) => `.github/actions/${dir}/action.yml`);
const workflowFiles = fs
  .readdirSync(path.join(ROOT, ".github/workflows"))
  .filter((f) => f.endsWith(".yml"))
  .map((f) => `.github/workflows/${f}`);
const actionSteps = (file: string) => (yaml.load(read(file)) as Action).runs.steps ?? [];
const workflowSteps = (file: string) =>
  Object.values((yaml.load(read(file)) as Workflow).jobs).flatMap((job) => job.steps ?? []);

// The package specs of each `npm install|i|add` of npm itself in shell text.
function npmSelfInstalls(script: string): string[] {
  return script
    .split(/\n|;|&&|\|\|?/)
    .map((command) =>
      command
        .trim()
        .split(/\s+/)
        .map((t) => t.replace(/^["']|["']$/g, "")),
    )
    .filter(([cmd, sub]) => cmd === "npm" && ["install", "i", "add"].includes(sub))
    .flatMap((tokens) => tokens.slice(2).filter((t) => t === "npm" || t.startsWith("npm@")));
}

// The Dockerfile's ARG defaults and RUN scripts, comments dropped and
// continuation lines joined.
function dockerfile() {
  const instructions = read("Dockerfile")
    .split("\n")
    .filter((line) => !/^\s*#/.test(line))
    .join("\n")
    .replace(/\\\n/g, " ")
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
  const args: Record<string, string> = {};
  const runs: string[] = [];
  for (const line of instructions) {
    const keyword = line.split(/\s/, 1)[0].toUpperCase();
    const rest = line.replace(/^\S+\s*/, "");
    if (keyword === "ARG" && rest.includes("=")) {
      const [name, value] = rest.split(/=(.*)/s);
      args[name] = value;
    } else if (keyword === "RUN") {
      runs.push(rest.replace(/\$\{(\w+)\}/g, (_, name: string) => args[name] ?? ""));
    }
  }
  return { args, runs };
}

describe("code scanning alert build-ci/cwe-829 - one exact npm, moved by hand", () => {
  it("reads npm installs of npm out of shell, not comments or other packages", () => {
    expect(
      npmSelfInstalls(
        '# npm install --global npm@1.2.3\nnpm ci\nnpm i -g npm && npm install -g "npm@2.0.0"; npm install npm-run-all',
      ),
    ).toEqual(["npm", "npm@2.0.0"]);
  });

  it("the Dockerfile's NPM_VERSION is an exact npm 12, and both it and the setup action install it", () => {
    const { args, runs } = dockerfile();
    const version = args.NPM_VERSION;
    expect(version).toMatch(/^12\.\d+\.\d+$/);
    expect(runs.flatMap(npmSelfInstalls)).toEqual([`npm@${version}`]);
    const setup = actionSteps(".github/actions/setup/action.yml").flatMap((s) =>
      npmSelfInstalls(s.run ?? ""),
    );
    expect(setup).toEqual([`npm@${version}`]);
  });

  it("no workflow or action installs npm by a range", () => {
    for (const [file, steps] of [
      ...actionFiles.map((f) => [f, actionSteps(f)] as const),
      ...workflowFiles.map((f) => [f, workflowSteps(f)] as const),
    ]) {
      for (const spec of steps.flatMap((s) => npmSelfInstalls(s.run ?? ""))) {
        expect(spec, file).toMatch(/^npm@\d+\.\d+\.\d+$/);
      }
    }
  });
});

describe("code scanning alert build-ci/cwe-693 - FLATPAK_PATHS covers what the Flatpak check protects", () => {
  const scope = actionSteps(".github/actions/scope/action.yml").find((s) => s.id === "scope");
  const flatpakPaths = new RegExp(scope?.env?.FLATPAK_PATHS ?? "(?!)");

  it("covers the Flatpak's keyring store, the troubleshooting log's command and the logging module", () => {
    for (const file of [
      "src-tauri/src/services/keyring.rs",
      "src-tauri/src/services/keyring/portal.rs",
      "src-tauri/src/commands/troubleshooting_log.rs",
      "src-tauri/src/logging.rs",
      "src-tauri/src/logging/file.rs",
    ]) {
      expect(flatpakPaths.test(file), file).toBe(true);
    }
    expect(flatpakPaths.test("src-tauri/src/logging_other.rs")).toBe(false);
  });

  it("covers every path docs/development/flatpak.md lists, and each exists", () => {
    const doc = read("docs/development/flatpak.md");
    const list = /changes any of these\s+paths \(`011 FR-021`\):([\s\S]*?)\n\n/.exec(doc)?.[1];
    expect(list).toBeDefined();
    const paths: string[] = [];
    for (const part of list!.split(";")) {
      const under = /under\s+`([^`]+)`:/.exec(part);
      const items = part.slice(under ? under.index + under[0].length : 0);
      for (const [, item] of items.matchAll(/`([^`]+)`/g)) paths.push((under?.[1] ?? "") + item);
    }
    expect(paths.length).toBeGreaterThan(20);
    for (const p of paths) {
      expect(fs.existsSync(path.join(ROOT, p)), p).toBe(true);
      expect(flatpakPaths.test(p.endsWith("/") ? `${p}x.rs` : p), p).toBe(true);
    }
  });
});
