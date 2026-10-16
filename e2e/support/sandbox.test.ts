import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import {
  OWNER_MARKER,
  SANDBOX_PREFIX,
  SHADER_CACHE,
  findStaleSandboxes,
  isolateAppData,
  removeSandbox,
  sweepStaleSandboxes,
} from "./sandbox";

const saved = { ...process.env };

afterEach(async () => {
  await removeSandbox();
  process.env = { ...saved };
});

describe("the E2E sandbox", () => {
  // Under software rendering, a sandbox's own empty shader cache cost every
  // spec about a second the first time a dialog was drawn (#102).
  it.runIf(process.platform === "linux")(
    "shares one Mesa shader cache, outside every sandbox, between sessions",
    async () => {
      const first = isolateAppData();
      expect(process.env.XDG_CACHE_HOME?.startsWith(first)).toBe(true);
      expect(process.env.MESA_SHADER_CACHE_DIR).toBe(SHADER_CACHE);
      await removeSandbox();

      const second = isolateAppData();
      expect(second).not.toBe(first);
      expect(process.env.MESA_SHADER_CACHE_DIR).toBe(SHADER_CACHE);
      for (const sandbox of [first, second]) {
        expect(path.relative(sandbox, SHADER_CACHE).startsWith("..")).toBe(true);
      }
      expect(SHADER_CACHE.split(path.sep)).toContain("node_modules");
    },
  );

  // #111: the marker is what lets a later run tell a sandbox from anything
  // else in the temp folder, and a sandbox still in use from an old one.
  it("is a prefixed directory in the temp folder, owned by this process, and removeSandbox removes it", async () => {
    const root = isolateAppData();
    expect(path.dirname(root)).toBe(path.resolve(os.tmpdir()));
    expect(path.basename(root).startsWith(SANDBOX_PREFIX)).toBe(true);
    expect(fs.readFileSync(path.join(root, OWNER_MARKER), "utf-8").trim()).toBe(
      String(process.pid),
    );
    // Even if the process table claimed its owner were gone, this process
    // never sweeps its own sandbox.
    expect(findStaleSandboxes(os.tmpdir(), () => false)).not.toContain(root);
    fs.writeFileSync(path.join(root, "data", "file"), "x");

    await removeSandbox();
    expect(fs.existsSync(root)).toBe(false);
  });
});

// #111: sandboxes an interrupted run left behind are swept by the next run,
// and nothing else in the temp folder is touched.
describe("sweeping old sandboxes", () => {
  const DEAD = 2_000_000_001;
  const ALIVE = 2_000_000_002;
  const running = (pid: number) => pid === ALIVE;
  let parent: string;

  /** A sandbox-shaped directory in the test's own temp folder. */
  function sandbox(suffix: string, owner: string | undefined) {
    const dir = path.join(parent, `${SANDBOX_PREFIX}${suffix}`);
    fs.mkdirSync(path.join(dir, "cache", "deep"), { recursive: true });
    fs.writeFileSync(path.join(dir, "cache", "deep", "file"), "x");
    if (owner !== undefined) fs.writeFileSync(path.join(dir, OWNER_MARKER), owner);
    return dir;
  }

  beforeEach(() => {
    parent = fs.mkdtempSync(path.join(os.tmpdir(), "hoplodex-sweep-test-"));
  });
  afterEach(() => {
    fs.rmSync(parent, { recursive: true, force: true });
  });

  it("selects only a prefixed directory with a marker whose owner is not running", () => {
    const stale = sandbox("aaaaaa", `${DEAD}\n`);
    const stillRunning = sandbox("bbbbbb", `${ALIVE}\n`);
    const ours = sandbox("cccccc", `${process.pid}\n`);
    const noMarker = sandbox("dddddd", undefined);
    const garbage = sandbox("eeeeee", "not a pid");
    const wrongName = path.join(parent, `${SANDBOX_PREFIX}screen-x`);
    fs.mkdirSync(wrongName);
    fs.writeFileSync(path.join(wrongName, OWNER_MARKER), `${DEAD}\n`);
    const otherPrefix = path.join(parent, "other-ffffff");
    fs.mkdirSync(otherPrefix);
    fs.writeFileSync(path.join(otherPrefix, OWNER_MARKER), `${DEAD}\n`);
    // us13's screenshots: plain files with the prefix, in the same folder.
    const screenshot = path.join(parent, `${SANDBOX_PREFIX}screen-${DEAD}.png`);
    fs.writeFileSync(screenshot, "png");
    // A file that happens to be named like a sandbox.
    const plainFile = path.join(parent, `${SANDBOX_PREFIX}gggggg`);
    fs.writeFileSync(plainFile, `${DEAD}\n`);
    // A marker that is a directory, not a file.
    const markerDir = path.join(parent, `${SANDBOX_PREFIX}hhhhhh`);
    fs.mkdirSync(path.join(markerDir, OWNER_MARKER), { recursive: true });

    expect(findStaleSandboxes(parent, running)).toEqual([stale]);
    for (const kept of [stillRunning, ours, noMarker, garbage, wrongName, otherPrefix]) {
      expect(fs.existsSync(kept)).toBe(true);
    }
    expect(fs.existsSync(screenshot)).toBe(true);
    expect(fs.existsSync(plainFile)).toBe(true);
    expect(fs.existsSync(markerDir)).toBe(true);
  });

  it("removes the stale sandboxes with their contents and leaves everything else", async () => {
    const stale = sandbox("aaaaaa", `${DEAD}\n`);
    const alsoStale = sandbox("zzzzzz", `${DEAD}\n`);
    const stillRunning = sandbox("bbbbbb", `${ALIVE}\n`);
    const noMarker = sandbox("dddddd", undefined);
    const screenshot = path.join(parent, `${SANDBOX_PREFIX}screen-${DEAD}.png`);
    fs.writeFileSync(screenshot, "png");

    const report = await sweepStaleSandboxes(parent, running);

    expect(report.removed.sort()).toEqual([stale, alsoStale].sort());
    expect(report.kept).toEqual([]);
    expect(fs.existsSync(stale)).toBe(false);
    expect(fs.existsSync(alsoStale)).toBe(false);
    for (const kept of [stillRunning, noMarker, screenshot]) {
      expect(fs.existsSync(kept)).toBe(true);
    }
  });

  it("treats a process that really is gone as the owner being gone", async () => {
    // A child that has exited: its pid is no longer running.
    const { spawnSync } = await import("node:child_process");
    const child = spawnSync(process.execPath, ["-e", "process.stdout.write(String(process.pid))"], {
      encoding: "utf-8",
    });
    const dir = sandbox("iiiiii", `${child.stdout}\n`);
    expect(findStaleSandboxes(parent)).toEqual([dir]);
    const self = sandbox("jjjjjj", `${process.pid}\n`);
    expect(findStaleSandboxes(parent)).not.toContain(self);
  });
});
