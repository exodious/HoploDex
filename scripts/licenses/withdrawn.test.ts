// @vitest-environment node
// 010 research.md §2, contracts/notices.md "Withdrawn data": the build's check
// for a withdrawn data file searches the emitted chunks for the entry's marker,
// the file's opening as a bundle encodes it. This test reads the real
// node_modules and fails when a package update encodes the file differently,
// instead of the check quietly turning itself off.
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { WITHDRAWN } from "./policy.mjs";

const modules = resolve(__dirname, "../../node_modules");

/** The module Vite bundles for a data file: a `.json` becomes its `.json.mjs` twin. */
function moduleFor(file: string): string {
  return resolve(modules, file.endsWith(".json") ? `${file}.mjs` : file);
}

describe("010 research.md §2 - each withdrawn marker occurs in its file's real module", () => {
  if (!existsSync(modules)) {
    it.skip("node_modules is not installed here; run npm ci to check the withdrawn markers", () => {});
    return;
  }
  it("lists at least one withdrawn file", () => {
    expect(WITHDRAWN.length).toBeGreaterThan(0);
  });
  for (const entry of WITHDRAWN) {
    it(`010 contracts/notices.md: the marker for ${entry.file} is in its module`, () => {
      const path = moduleFor(entry.file);
      expect(existsSync(path), `${path} is missing: the package moved or renamed the file`).toBe(
        true,
      );
      expect(
        readFileSync(path, "utf8").includes(entry.marker),
        `${entry.file}'s marker is not in ${path}: the package encodes the list differently, so policy.mjs's marker needs updating`,
      ).toBe(true);
    });
  }
});
