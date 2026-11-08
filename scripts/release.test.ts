// @vitest-environment node
// scripts/release.mjs (#19): the version check, the migration records, that
// only the last step tags and none publishes, and, run on a scratch repository,
// what it refuses before running anything.
import { afterAll, describe, expect, it } from "vitest";
import yaml from "js-yaml";
import { execFileSync, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  parseVersion,
  plan,
  recordLines,
  ROOT,
  STEPS,
  steps,
  versionProblems,
} from "./release.mjs";

const commit = "c".repeat(40);
// SHA-256 of "abc" (FIPS 180-2's example) and of nothing.
const ABC = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
const EMPTY = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

const temps: string[] = [];
afterAll(() => temps.forEach((d) => fs.rmSync(d, { recursive: true, force: true })));

function writeAll(dir: string, files: Record<string, string>) {
  for (const [f, content] of Object.entries(files)) {
    fs.mkdirSync(path.dirname(path.join(dir, f)), { recursive: true });
    fs.writeFileSync(path.join(dir, f), content);
  }
}

function tempDir(): string {
  // The real path: Node runs a script from it, which release.mjs compares with argv.
  const dir = fs.realpathSync.native(fs.mkdtempSync(path.join(os.tmpdir(), "hoplodex-release-")));
  temps.push(dir);
  return dir;
}

const RELEASED = "src-tauri/src/db/migrations/released.txt";

/** Every file `version` and `tag` read, carrying `version`. */
const carrying = (version: string, released = "# none yet\n") => ({
  "package.json": JSON.stringify({ version }),
  "package-lock.json": JSON.stringify({ version, packages: { "": { version } } }),
  "src-tauri/Cargo.toml": `[package]\nname = "hoplodex"\nversion = "${version}"\n`,
  "src-tauri/Cargo.lock": `[[package]]\nname = "hoplodex"\nversion = "${version}"\n`,
  "src-tauri/tauri.conf.json": JSON.stringify({ version }),
  "flatpak/io.github.exodious.HoploDex.metainfo.xml": `<release version="${version}" />`,
  [RELEASED]: released,
});

/** A scratch repository with the script in it, since it works on its own checkout. */
function scratchRepo() {
  const dir = tempDir();
  const git = (args: string[], input?: string) =>
    execFileSync("git", args, {
      cwd: dir,
      encoding: "utf8",
      input,
      stdio: ["pipe", "pipe", "ignore"],
    }).trim();
  git(["init", "-q", "-b", "main"]);
  fs.mkdirSync(path.join(dir, "scripts"));
  fs.copyFileSync(path.join(ROOT, "scripts/release.mjs"), path.join(dir, "scripts/release.mjs"));
  return {
    dir,
    git,
    /** Commits `files` as HEAD: a raw object, so no git identity need be configured. */
    commit(files: Record<string, string>, parent = "") {
      writeAll(dir, files);
      git(["add", "-A"]);
      const who = "test <test@example.com> 0 +0000";
      const sha = git(
        ["hash-object", "-t", "commit", "-w", "--stdin"],
        `tree ${git(["write-tree"])}\n${parent && `parent ${parent}\n`}author ${who}\ncommitter ${who}\n\nA commit\n`,
      );
      git(["update-ref", "HEAD", sha]);
      return sha;
    },
    release(...args: string[]) {
      const r = spawnSync(process.execPath, ["scripts/release.mjs", ...args], {
        cwd: dir,
        encoding: "utf8",
      });
      return { status: r.status, stdout: r.stdout, stderr: r.stderr };
    },
  };
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

  it("writes a final release's records: each unreleased step in MIGRATIONS' order, then its database", () => {
    const dir = tempDir();
    writeAll(dir, {
      "src-tauri/src/db/mod.rs": [
        `("0001_old", include_str!("migrations/0001_old.sql")),`,
        `("0003_later", include_str!("migrations/0003_later.sql")),`,
        `("0002_new", include_str!("migrations/0002_new.sql")),`,
        `(`,
        `    "0004_a_step_name_long_enough_to_wrap",`,
        `    include_str!("migrations/0004_a_step_name_long_enough_to_wrap.sql"),`,
        `),`,
      ].join("\n"),
      [RELEASED]: `# <name> <sha256> <release>\n0001_old ${EMPTY} 0.1.0\n`,
      "src-tauri/src/db/migrations/0001_old.sql": "not read",
      "src-tauri/src/db/migrations/0002_new.sql": "abc",
      "src-tauri/src/db/migrations/0003_later.sql": "",
      "src-tauri/src/db/migrations/0004_a_step_name_long_enough_to_wrap.sql": "abc",
      "src-tauri/tests/fixtures/releases/0.2.0.hoplodex": "abc",
    });
    const lines = recordLines(dir, "0.2.0");
    expect(lines).toEqual([
      `0003_later ${EMPTY} 0.2.0`,
      `0002_new ${ABC} 0.2.0`,
      `0004_a_step_name_long_enough_to_wrap ${ABC} 0.2.0`,
      `tests/fixtures/releases/0.2.0.hoplodex ${ABC} 0.2.0`,
    ]);
    fs.appendFileSync(path.join(dir, RELEASED), `${lines.join("\n")}\n`);
    expect(() => recordLines(dir, "0.2.0")).toThrow(/already has/);
    expect(() => recordLines(dir, "0.3.0")).toThrow(/no stored database/);
    expect(() => recordLines(dir, "0.2.0-rc.1")).toThrow(/release candidate/);
  });

  it("leaves the records out of all, since they reach the commit through a pull request", () => {
    expect(steps("all")).toEqual(["version", "checks", "review", "gate", "tag"]);
    expect(steps("records")).toEqual(["records"]);
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
    const gate = plan("gate", { version: "0.1.0-rc.1", commit, after: null, container: false });
    expect(gate.map((p: { run?: string[] }) => p.run)).toEqual([
      ["node", "scripts/ai-security-review.mjs", "gate"],
    ]);
    const tag = plan("tag", { version: "0.1.0-rc.1", commit, after: null, container: false });
    expect(tag.map((p: { run?: string[] }) => p.run)).toEqual([
      ["node", "scripts/ai-security-review.mjs", "reviewed", "--commit", commit],
      ["git", "tag", "-a", "v0.1.0-rc.1", "-m", "HoploDex 0.1.0-rc.1", commit],
      ["git", "push", "origin", "refs/tags/v0.1.0-rc.1"],
      undefined,
    ]);
  });

  it("runs the container's commands there, and only what no workflow job runs", () => {
    const checks = (container: boolean): string[][] =>
      plan("checks", { version: "0.1.0", commit, after: null, container }).map(
        (p: { run: string[] }) => p.run,
      );
    expect(checks(true)[0].slice(0, 3)).toEqual(["scripts/dev-container.sh", "bash", "-c"]);
    expect(checks(false)[0].slice(0, 2)).toEqual(["bash", "-c"]);
    expect(checks(false)).toHaveLength(2);
    expect(checks(false).join(" ")).not.toMatch(/import_largest_test|npm run audit|flatpak/);
    expect(() => plan("flatpak", { version: "0.1.0", commit, after: null })).toThrow(/no step/);
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

// Each spawns node and git several times: slow on Windows.
describe("release script, run on a repository", { timeout: 60_000 }, () => {
  it("refuses a flag it doesn't know or one missing its value, before doing anything", () => {
    const repo = scratchRepo();
    const head = repo.commit(carrying("0.1.0"));
    for (const args of [
      ["all", "0.1.0-rc.1", "--commit", head, "--dryrun"],
      ["all", "0.1.0-rc.1", "--commit", head, "-n"],
      ["all", "0.1.0-rc.1", "--commit"],
      ["all", "0.1.0-rc.1", "extra", "--commit", head, "--dry-run"],
    ]) {
      const r = repo.release(...args);
      expect(r.status, args.join(" ")).toBe(1);
      expect(r.stdout, args.join(" ")).toBe("");
    }
  });

  it("runs the working tree's steps only on a clean tree at the commit", () => {
    const repo = scratchRepo();
    const older = repo.commit(carrying("0.1.0"));
    const head = repo.commit(carrying("0.2.0"), older);

    expect(repo.release("version", "0.2.0", "--commit", head)).toMatchObject({ status: 0 });
    for (const step of ["version", "checks", "all"]) {
      const r = repo.release(step, "0.1.0-rc.1", "--commit", older, "--dry-run");
      expect(r.status, step).toBe(1);
      expect(r.stderr, step).toContain(`HEAD is ${head}, not ${older}`);
      expect(r.stdout, step).toBe("");
    }

    fs.writeFileSync(path.join(repo.dir, "stray"), "");
    const dirty = repo.release("version", "0.2.0", "--commit", head);
    expect(dirty.status).toBe(1);
    expect(dirty.stderr).toContain("isn't clean");
  });

  it("applies the workflow's tag check to the commit, not the working tree", () => {
    const repo = scratchRepo();
    const older = repo.commit(carrying("0.1.0"));
    repo.commit(carrying("0.2.0"), older);

    expect(repo.release("tag", "0.1.0-rc.1", "--commit", older, "--dry-run").status).toBe(0);
    const wrong = repo.release("tag", "0.2.0-rc.1", "--commit", older, "--dry-run");
    expect(wrong.status).toBe(1);
    expect(wrong.stderr).toContain("doesn't name");
    expect(wrong.stdout).toBe("");
  });

  it("refuses a final tag without its stored database or released.txt line, before any step", () => {
    const repo = scratchRepo();
    const bare = repo.commit(carrying("0.1.0"));
    const none = repo.release("all", "0.1.0", "--commit", bare, "--dry-run");
    expect(none.status).toBe(1);
    expect(none.stderr).toContain("no stored database");
    expect(none.stdout).toBe("");

    const fixture = "tests/fixtures/releases/0.1.0.hoplodex";
    const unlisted = repo.commit({ [`src-tauri/${fixture}`]: "abc" }, bare);
    const noLine = repo.release("all", "0.1.0", "--commit", unlisted, "--dry-run");
    expect(noLine.status).toBe(1);
    expect(noLine.stderr).toContain("no line");

    const listed = repo.commit(
      { [RELEASED]: `0001_a ${EMPTY} 0.1.0\n${fixture} ${ABC} 0.1.0\n` },
      unlisted,
    );
    const ready = repo.release("all", "0.1.0", "--commit", listed, "--dry-run");
    expect(ready.status).toBe(0);
    expect(ready.stdout).not.toContain("== records");
    expect(ready.stdout).toContain(`$ git tag -a v0.1.0 -m "HoploDex 0.1.0" ${listed}`);
    // A dry run only prints it.
    expect(repo.git(["tag", "-l"])).toBe("");
  });
});

type Step = {
  name?: string;
  uses?: string;
  env?: Record<string, string>;
  with?: Record<string, string>;
};

function ciSteps(): Step[][] {
  const ci = yaml.load(fs.readFileSync(path.join(ROOT, ".github/workflows/ci.yml"), "utf8")) as {
    jobs: Record<string, { steps: Step[] }>;
  };
  return Object.values(ci.jobs).map((j) => j.steps);
}

const hasCargo = spawnSync("cargo", ["--version"], { stdio: "ignore" }).status === 0;

describe("the checks step's tests", () => {
  it.runIf(hasCargo)("are selected by what Cargo really has", () => {
    const { packages } = JSON.parse(
      execFileSync(
        "cargo",
        [
          "metadata",
          "--no-deps",
          "--format-version",
          "1",
          "--manifest-path",
          "src-tauri/Cargo.toml",
        ],
        { cwd: ROOT, encoding: "utf8" },
      ),
    );
    const targets = packages
      .flatMap((p: { targets: { kind: string[]; name: string }[] }) => p.targets)
      .filter((t: { kind: string[] }) => t.kind.includes("test"))
      .map((t: { name: string }) => t.name);
    const commands = plan("checks", { version: "0.1.0", commit, after: null, container: false })
      .map((p: { run: string[] }) => p.run[2])
      .join("\n");
    const named = (pattern: RegExp) => [...commands.matchAll(pattern)].map((m) => m[1]);

    const asTargets = [...named(/--test (\w+)/g), ...named(/binary\((\w+)\)/g)];
    expect(asTargets.sort()).toEqual(["performance_test", "release_upgrade_test"]);
    for (const t of asTargets) expect(targets).toContain(t);

    // A file compiled into `integration`: its tests' names start with its name
    // (docs/development/testing.md), and it is no target of its own.
    expect(named(/test\(\/\^(\w+)::\/\)/g)).toEqual(["released_migrations_test"]);
    expect(targets).toContain("integration");
    expect(targets).not.toContain("released_migrations_test");
    expect(fs.existsSync(path.join(ROOT, "src-tauri/tests/released_migrations_test.rs"))).toBe(
      true,
    );
  });
});

describe("the package jobs' artifacts", () => {
  it("are one path each, so the draft job gets a flat list of files (upload-artifact keeps the folders below several paths' common ancestor)", () => {
    const uploads = ciSteps()
      .flat()
      .filter(
        (s) => s.uses?.startsWith("actions/upload-artifact") && /^packages-/.test(s.with!.name),
      );
    expect(uploads.map((s) => s.with!.name).sort()).toEqual([
      "packages-deb",
      "packages-flatpak",
      "packages-macos",
      "packages-rpm",
      "packages-windows",
    ]);
    for (const s of uploads) expect(s.with!.path.trim(), s.with!.name).not.toContain("\n");
  });
});

describe("Quit cleanup on a release's build", () => {
  it("checks the profile its job's E2E step built, on every OS", () => {
    const jobs = ciSteps().filter((steps) => steps.some((s) => s.name === "Quit cleanup"));
    expect(jobs).toHaveLength(3);
    for (const steps of jobs) {
      const profile = (name: string) =>
        steps.find((s) => s.name === name)?.env?.HOPLODEX_E2E_PROFILE;
      expect(profile("E2E")).toBeTruthy();
      expect(profile("Quit cleanup")).toBe(profile("E2E"));
    }
  });

  // A scratch copy: its binary is never there, so it stops before starting an app.
  it.skipIf(process.platform === "win32")(
    "looks for target/<profile>/hoplodex, as e2e/support/app.ts launches it",
    () => {
      const dir = tempDir();
      const script = path.join(dir, "e2e/scripts/quit-cleanup.py");
      fs.mkdirSync(path.dirname(script), { recursive: true });
      fs.copyFileSync(path.join(ROOT, "e2e/scripts/quit-cleanup.py"), script);
      const missing = (env: Record<string, string>) => {
        const r = spawnSync("python3", [script], {
          encoding: "utf8",
          env: { ...process.env, HOPLODEX_BIN: undefined, HOPLODEX_E2E_PROFILE: undefined, ...env },
        });
        expect(r.status).toBe(1);
        return r.stderr;
      };
      const built = (profile: string) => path.join(dir, "src-tauri/target", profile, "hoplodex");
      expect(missing({})).toContain(`${built("e2e")} not found`);
      expect(missing({ HOPLODEX_E2E_PROFILE: "" })).toContain(`${built("e2e")} not found`);
      expect(missing({ HOPLODEX_E2E_PROFILE: "release" })).toContain(
        `${built("release")} not found`,
      );
      expect(
        missing({ HOPLODEX_E2E_PROFILE: "release", HOPLODEX_BIN: path.join(dir, "elsewhere") }),
      ).toContain(`${path.join(dir, "elsewhere")} not found`);
    },
  );
});
