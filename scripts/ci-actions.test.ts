// @vitest-environment node
// What the CI composite actions guard: the npm every job and the dev container
// install (code scanning alert build-ci/cwe-829), and the paths whose pull
// requests run the Flatpak check (code scanning alert build-ci/cwe-693,
// 011 FR-021).
import fs from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

const ROOT = path.resolve(import.meta.dirname, "..");
const read = (file: string) => fs.readFileSync(path.join(ROOT, file), "utf8");

describe("code scanning alert build-ci/cwe-829 - one exact npm, moved by hand", () => {
  it("the Dockerfile's NPM_VERSION is an exact npm 12, and the setup action installs it", () => {
    const docker = /^ARG NPM_VERSION=(.*)$/m.exec(read("Dockerfile"))?.[1];
    expect(docker).toMatch(/^12\.\d+\.\d+$/);
    const setup = [
      ...read(".github/actions/setup/action.yml").matchAll(/npm install --global npm@(\S+)/g),
    ].map((m) => m[1]);
    expect(setup).toEqual([docker]);
  });

  it("no workflow or action installs npm by a range", () => {
    const files = [".github/workflows", ".github/actions/setup", ".github/actions/scope"].flatMap(
      (dir) => fs.readdirSync(path.join(ROOT, dir)).map((f) => `${dir}/${f}`),
    );
    for (const file of files.filter((f) => f.endsWith(".yml"))) {
      for (const [, version] of read(file).matchAll(/\bnpm@(\S+)/g)) {
        expect(version, file).toMatch(/^\d+\.\d+\.\d+$/);
      }
    }
  });
});

describe("code scanning alert build-ci/cwe-693 - FLATPAK_PATHS covers what the Flatpak check protects", () => {
  const scope = read(".github/actions/scope/action.yml");
  const flatpakPaths = new RegExp(/^\s*FLATPAK_PATHS: '(.*)'$/m.exec(scope)?.[1] ?? "(?!)");

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
