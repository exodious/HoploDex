// @vitest-environment node
// scripts/release.mjs (#19): the version check, the migration records, and
// that only the last step tags, and none publishes.
import { describe, expect, it } from "vitest";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  migrationSteps,
  parseVersion,
  plan,
  recordLines,
  ROOT,
  STEPS,
  steps,
  versionProblems,
} from "./release.mjs";

const commit = "c".repeat(40);

/** A copy of the files the version and records steps read. */
function copyRepo(): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "hoplodex-release-"));
  for (const f of [
    "package.json",
    "package-lock.json",
    "src-tauri/Cargo.toml",
    "src-tauri/Cargo.lock",
    "src-tauri/tauri.conf.json",
    "flatpak/io.github.exodious.HoploDex.metainfo.xml",
    "src-tauri/src/db/mod.rs",
    "src-tauri/src/db/migrations/released.txt",
    "src-tauri/tests/fixtures/releases/0.1.0.hoplodex",
    ...migrationSteps(ROOT).map((s: string) => `src-tauri/src/db/migrations/${s}.sql`),
  ]) {
    fs.mkdirSync(path.dirname(path.join(dir, f)), { recursive: true });
    fs.copyFileSync(path.join(ROOT, f), path.join(dir, f));
  }
  return dir;
}

describe("release script", () => {
  it("reads a version with or without a pre-release suffix", () => {
    expect(parseVersion("0.1.0")).toEqual({ app: "0.1.0", final: true, tag: "v0.1.0" });
    expect(parseVersion("0.1.0-rc.1")).toMatchObject({ app: "0.1.0", final: false });
    expect(() => parseVersion("v0.1.0")).toThrow(/not a version/);
  });

  it("finds the version the repository carries, and each place that differs", () => {
    const version = JSON.parse(fs.readFileSync(path.join(ROOT, "package.json"), "utf8")).version;
    expect(versionProblems(ROOT, version)).toEqual([]);
    expect(versionProblems(ROOT, `${version}-rc.1`)).toEqual([]);
    expect(versionProblems(ROOT, "9.9.9")).toHaveLength(7);
  });

  it("writes a final release's records: each unreleased step, then its database", () => {
    const dir = copyRepo();
    const lines = recordLines(dir, "0.1.0");
    expect(lines.map((l: string) => l.split(" ")[0])).toEqual([
      ...migrationSteps(ROOT),
      "tests/fixtures/releases/0.1.0.hoplodex",
    ]);
    for (const l of lines) expect(l).toMatch(/^\S+ [0-9a-f]{64} 0\.1\.0$/);
    fs.appendFileSync(
      path.join(dir, "src-tauri/src/db/migrations/released.txt"),
      `${lines.join("\n")}\n`,
    );
    expect(() => recordLines(dir, "0.1.0")).toThrow(/already has/);
    expect(() => recordLines(dir, "0.2.0")).toThrow(/no stored database/);
    expect(() => recordLines(dir, "0.1.0-rc.1")).toThrow(/release candidate/);
  });

  it("skips the records for a release candidate", () => {
    expect(steps("all", "0.1.0")).toEqual(STEPS);
    expect(steps("all", "0.1.0-rc.1")).not.toContain("records");
  });

  it("tags only in its last step, after the workflow's own check, and never makes a release", () => {
    for (const version of ["0.1.0", "0.1.0-rc.1"]) {
      for (const s of STEPS) {
        for (const p of plan(s, { version, commit, after: null, container: true })) {
          const run = (p.run ?? []).join(" ");
          expect(run).not.toMatch(/gh release|releases/);
          if (s !== "tag") expect(run).not.toMatch(/git (tag|push)/);
        }
      }
    }
    expect(STEPS.at(-1)).toBe("tag");
    const tag = plan("tag", { version: "0.1.0-rc.1", commit, after: null, container: false });
    expect(tag.map((p: { run?: string[] }) => p.run)).toEqual([
      ["node", "scripts/ai-security-review.mjs", "reviewed", "--commit", commit],
      ["git", "tag", "-a", "v0.1.0-rc.1", "-m", "HoploDex 0.1.0-rc.1", commit],
      ["git", "push", "origin", "refs/tags/v0.1.0-rc.1"],
      undefined,
    ]);
  });

  it("runs the container's commands there, the Flatpak's with --flatpak", () => {
    const [build] = plan("flatpak", { version: "0.1.0", commit, after: null, container: true });
    expect(build.run.slice(0, 3)).toEqual(["scripts/dev-container.sh", "--flatpak", "bash"]);
    const [review] = plan("review", {
      version: "0.1.0",
      commit,
      after: "a".repeat(40),
      container: true,
    });
    expect(review.run).toEqual([
      "claude",
      `/ai-security-review release ${commit} --after ${"a".repeat(40)}`,
    ]);
  });
});
