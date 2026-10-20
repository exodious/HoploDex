import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import {
  OWNER_MARKER,
  RELEASE_FIXTURE_0_1_0,
  SANDBOX_PREFIX,
  SHADER_CACHE,
  applySessionSettings,
  copyReleaseFixture,
  findStaleSandboxes,
  isolateAppData,
  removeSandbox,
  sha256Of,
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

// 011 FR-017: the store shots' sandbox is at the same path on every run, so
// the chooser's path in the shot is too.
describe("a sandbox at a fixed path", () => {
  const DEAD = 2_000_000_001;
  let parent: string;
  beforeEach(() => {
    parent = fs.mkdtempSync(path.join(os.tmpdir(), "hoplodex-fixed-test-"));
  });
  afterEach(() => {
    fs.rmSync(parent, { recursive: true, force: true });
  });

  it("is made at that path, owned by this process, and removeSandbox removes it", async () => {
    const dir = path.join(parent, "shots");
    expect(isolateAppData({ fixedDir: dir })).toBe(dir);
    expect(fs.readFileSync(path.join(dir, OWNER_MARKER), "utf-8").trim()).toBe(String(process.pid));
    expect(process.env.HOPLODEX_E2E_DOCUMENTS?.startsWith(dir)).toBe(true);
    await removeSandbox();
    expect(fs.existsSync(dir)).toBe(false);
  });

  it("replaces what a run that ended left there, and nothing else", () => {
    const dir = path.join(parent, "shots");
    fs.mkdirSync(dir);
    fs.writeFileSync(path.join(dir, OWNER_MARKER), `${DEAD}\n`);
    fs.writeFileSync(path.join(dir, "old"), "x");
    expect(isolateAppData({ fixedDir: dir })).toBe(dir);
    expect(fs.existsSync(path.join(dir, "old"))).toBe(false);

    // A folder that isn't a sandbox is left alone and refused.
    const other = path.join(parent, "mine");
    fs.mkdirSync(other);
    fs.writeFileSync(path.join(other, "precious"), "x");
    expect(() => isolateAppData({ fixedDir: other })).toThrow(/not an E2E sandbox/);
    expect(fs.readFileSync(path.join(other, "precious"), "utf-8")).toBe("x");
  });

  it("refuses a path another running process owns", () => {
    const dir = path.join(parent, "shots");
    fs.mkdirSync(dir);
    fs.writeFileSync(path.join(dir, OWNER_MARKER), `${process.ppid}\n`);
    expect(() => isolateAppData({ fixedDir: dir })).toThrow(/running process/);
    expect(fs.existsSync(path.join(dir, OWNER_MARKER))).toBe(true);
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
    // document-preview.e2e.ts's screenshots: plain files with the prefix, in the same folder.
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

// 009 US3: the session settings the upgrade journeys use.
describe("the upgrade phase setting", () => {
  it("sets HOPLODEX_E2E_UPGRADE_PHASE_SECONDS, and clears it when a session doesn't", () => {
    applySessionSettings({ upgradePhaseSeconds: 4 });
    expect(process.env.HOPLODEX_E2E_UPGRADE_PHASE_SECONDS).toBe("4");

    applySessionSettings({ upgradePhaseSeconds: 0 });
    expect(process.env.HOPLODEX_E2E_UPGRADE_PHASE_SECONDS).toBe("0");

    // Whatever the developer's own environment holds never reaches the app.
    process.env.HOPLODEX_E2E_UPGRADE_PHASE_SECONDS = "99";
    applySessionSettings();
    expect(process.env.HOPLODEX_E2E_UPGRADE_PHASE_SECONDS).toBeUndefined();
  });

  it("is independent of the idle minute", () => {
    applySessionSettings({ idleMinuteSeconds: 3 });
    expect(process.env.HOPLODEX_E2E_IDLE_MINUTE_SECONDS).toBe("3");
    expect(process.env.HOPLODEX_E2E_UPGRADE_PHASE_SECONDS).toBeUndefined();
  });
});

// 009 US3: the stored 0.1.0 database, copied into the sandbox for the
// upgrade journeys. The source is a parameter; a stand-in file is used here,
// so these tests don't depend on the real fixture.
describe("copying the stored release database into the sandbox", () => {
  let source: string;
  const bytes = Buffer.from("not really a database, but bytes all the same");

  beforeEach(() => {
    source = path.join(os.tmpdir(), `hoplodex-fixture-test-${process.pid}-${Date.now()}`);
    fs.writeFileSync(source, bytes);
  });
  afterEach(() => {
    fs.rmSync(source, { force: true });
  });

  it("copies the bytes to <folder>/<name>.hoplodex and returns the path and SHA-256", () => {
    const root = isolateAppData();
    const folder = path.join(root, "documents", "Older");
    const copy = copyReleaseFixture(folder, "Older collection", source);

    expect(copy.path).toBe(path.join(folder, "Older collection.hoplodex"));
    expect(fs.readFileSync(copy.path)).toEqual(bytes);
    expect(copy.sha256).toBe(createHash("sha256").update(bytes).digest("hex"));
    expect(copy.sha256).toBe(sha256Of(copy.path));
    // The source is left as it was.
    expect(fs.readFileSync(source)).toEqual(bytes);
  });

  it("makes two independent copies and never replaces a file", () => {
    const root = isolateAppData();
    const first = copyReleaseFixture(path.join(root, "a"), "Same", source);
    const second = copyReleaseFixture(path.join(root, "b"), "Same", source);
    expect(second.path).not.toBe(first.path);
    expect(second.sha256).toBe(first.sha256);

    fs.writeFileSync(first.path, "changed");
    expect(() => copyReleaseFixture(path.join(root, "a"), "Same", source)).toThrow();
    expect(fs.readFileSync(first.path, "utf-8")).toBe("changed");
  });

  it("copies only into the sandbox, under a plain file name", () => {
    expect(() => copyReleaseFixture(os.tmpdir(), "x", source)).toThrow(/isolateAppData/);
    const root = isolateAppData();
    const outside = fs.mkdtempSync(path.join(os.tmpdir(), "hoplodex-outside-test-"));
    try {
      expect(() => copyReleaseFixture(outside, "x", source)).toThrow(/not inside the sandbox/);
      expect(() => copyReleaseFixture(path.join(root, ".."), "x", source)).toThrow(
        /not inside the sandbox/,
      );
      expect(() => copyReleaseFixture(root, "../escape", source)).toThrow(/plain file name/);
      expect(() => copyReleaseFixture(root, "", source)).toThrow(/plain file name/);
      expect(fs.readdirSync(outside)).toEqual([]);
    } finally {
      fs.rmSync(outside, { recursive: true, force: true });
    }
  });

  it("defaults to the 0.1.0 database stored under src-tauri/tests/fixtures/releases", () => {
    expect(RELEASE_FIXTURE_0_1_0.split(path.sep).slice(-5)).toEqual([
      "src-tauri",
      "tests",
      "fixtures",
      "releases",
      "0.1.0.hoplodex",
    ]);
  });
});
