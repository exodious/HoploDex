// @vitest-environment node
// scripts/release.mjs (#19): the version check, the migration records, that
// only the last step tags and none publishes, and, run on a scratch repository,
// what it refuses before running anything. Then a release candidate's
// promotion: the candidate a final tag takes, and release.yml's promote job,
// run with a stand-in for gh, which stops before the draft on any problem.
import { afterAll, describe, expect, it } from "vitest";
import yaml from "js-yaml";
import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  candidateFor,
  candidateProblems,
  parseVersion,
  plan,
  releaseFiles,
  releaseNotes,
  recordLines,
  ROOT,
  screenshotProblems,
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

const fixtureOf = (app: string) => `tests/fixtures/releases/${app}.hoplodex`;

/** src-tauri/src/db/mod.rs's `MIGRATIONS` entries for `steps`. */
const migrations = (...steps: string[]) => ({
  "src-tauri/src/db/mod.rs": steps
    .map((s) => `    ("${s}", include_str!("migrations/${s}.sql")),`)
    .join("\n"),
});

/** A version's stored database and its released.txt lines. */
const recorded = (app: string) => ({
  [`src-tauri/${fixtureOf(app)}`]: "abc",
  [RELEASED]: `0001_a ${EMPTY} ${app}\n${fixtureOf(app)} ${ABC} ${app}\n`,
});

/** Every file `version` and `tag` read, carrying `version`. */
const carrying = (version: string, released = "# none yet\n") => ({
  "package.json": JSON.stringify({ version }),
  "package-lock.json": JSON.stringify({ version, packages: { "": { version } } }),
  "src-tauri/Cargo.toml": `[package]\nname = "hoplodex"\nversion = "${version}"\n`,
  "src-tauri/Cargo.lock": `[[package]]\nname = "hoplodex"\nversion = "${version}"\n`,
  "src-tauri/tauri.conf.json": JSON.stringify({ version }),
  "flatpak/io.github.exodious.HoploDex.metainfo.xml": `<release version="${version}" />`,
  [RELEASED]: released,
  ...migrations("0001_a"),
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
    env: {} as Record<string, string>,
    release(...args: string[]) {
      const r = spawnSync(process.execPath, ["scripts/release.mjs", ...args], {
        cwd: dir,
        encoding: "utf8",
        env: { ...process.env, ...this.env },
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

  it("011 US4-4: checks each screenshot address's commit is merged and has the working tree's image", () => {
    const dir = tempDir();
    const sha = "a".repeat(40);
    const unmerged = "b".repeat(40);
    const at = (file: string, commit = sha) =>
      `<image>https://raw.githubusercontent.com/exodious/HoploDex/${commit}/flatpak/screenshots/${file}</image>`;
    writeAll(dir, {
      "flatpak/io.github.exodious.HoploDex.metainfo.xml": [
        at("same.png"),
        at("changed.png"),
        at("gone.png"),
        "<image>https://raw.githubusercontent.com/exodious/HoploDex/v0.1.0/flatpak/screenshots/same.png</image>",
        at("same.png", unmerged),
      ].join("\n"),
      "flatpak/screenshots/same.png": "one",
      "flatpak/screenshots/changed.png": "new",
      "flatpak/screenshots/gone.png": "here",
    });
    const there: Record<string, string> = { "same.png": "one", "changed.png": "old" };
    const show = (rev: string) => {
      const file = rev.replace(`${sha}:flatpak/screenshots/`, "");
      return file in there ? Buffer.from(there[file]) : null;
    };
    const problems = screenshotProblems(dir, { show, merged: (c: string) => c === sha });
    expect(problems).toHaveLength(4);
    expect(problems[0]).toContain("changed.png differs");
    expect(problems[1]).toContain(`${sha} has no flatpak/screenshots/gone.png`);
    expect(problems[2]).toContain("v0.1.0/flatpak/screenshots/same.png isn't");
    expect(problems[3]).toContain(`${unmerged} isn't in the released commit's history`);
  });

  it("writes a version's records before a release candidate: each unreleased step in MIGRATIONS' order, then its database unless listed", () => {
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
    const lines = recordLines(dir, "0.2.0-rc.1");
    expect(lines).toEqual([
      `0003_later ${EMPTY} 0.2.0`,
      `0002_new ${ABC} 0.2.0`,
      `0004_a_step_name_long_enough_to_wrap ${ABC} 0.2.0`,
      `tests/fixtures/releases/0.2.0.hoplodex ${ABC} 0.2.0`,
    ]);
    expect(recordLines(dir, "0.2.0")).toEqual(lines);
    fs.appendFileSync(path.join(dir, RELEASED), `${lines.join("\n")}\n`);
    // A later candidate and the final, on the same records: nothing to add.
    expect(recordLines(dir, "0.2.0-rc.2")).toEqual([]);
    expect(recordLines(dir, "0.2.0")).toEqual([]);
    // A step that fixes what a candidate's testing found gets its line before the next one.
    fs.appendFileSync(
      path.join(dir, "src-tauri/src/db/mod.rs"),
      `\n("0005_fix", include_str!("migrations/0005_fix.sql")),`,
    );
    writeAll(dir, { "src-tauri/src/db/migrations/0005_fix.sql": "" });
    expect(recordLines(dir, "0.2.0-rc.2")).toEqual([`0005_fix ${EMPTY} 0.2.0`]);
    expect(() => recordLines(dir, "0.3.0-rc.1")).toThrow(/no stored database/);
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
      ["node", "scripts/ai-security-review.mjs", "gate", "--version", "0.1.0"],
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

  it("011 US4-4: the version step refuses a screenshot that differs from its address's commit", () => {
    const repo = scratchRepo();
    const metainfo = "flatpak/io.github.exodious.HoploDex.metainfo.xml";
    const shot = (sha: string) =>
      `<release version="0.1.0" />\n<image>https://raw.githubusercontent.com/exodious/HoploDex/${sha}/flatpak/screenshots/a.png</image>`;
    const images = repo.commit({ ...carrying("0.1.0"), "flatpak/screenshots/a.png": "one" });
    const pointed = repo.commit({ [metainfo]: shot(images) }, images);
    expect(repo.release("version", "0.1.0", "--commit", pointed)).toMatchObject({ status: 0 });

    const retaken = repo.commit({ "flatpak/screenshots/a.png": "two" }, pointed);
    const r = repo.release("version", "0.1.0", "--commit", retaken);
    expect(r.status).toBe(1);
    expect(r.stderr).toContain(`flatpak/screenshots/a.png differs from ${images}'s`);
  });

  it("011 US4-4: the version step refuses a screenshot address at a commit outside the released history", () => {
    const repo = scratchRepo();
    const metainfo = "flatpak/io.github.exodious.HoploDex.metainfo.xml";
    const shot = (sha: string) =>
      `<release version="0.1.0" />\n<image>https://raw.githubusercontent.com/exodious/HoploDex/${sha}/flatpak/screenshots/a.png</image>`;
    const main = repo.commit({ ...carrying("0.1.0"), "flatpak/screenshots/a.png": "one" });
    // A branch's images commit, squash-merged as another commit, so main never holds it.
    const branch = repo.commit({ "flatpak/screenshots/a.png": "two" }, main);
    const squashed = repo.commit({ [metainfo]: shot(branch) }, main);
    const r = repo.release("version", "0.1.0", "--commit", squashed);
    expect(r.status).toBe(1);
    expect(r.stderr).toContain(`${branch} isn't in the released commit's history`);
  });

  it("applies the workflow's tag check to the commit, not the working tree", () => {
    const repo = scratchRepo();
    const older = repo.commit({ ...carrying("0.1.0"), ...recorded("0.1.0") });
    repo.commit(carrying("0.2.0"), older);

    expect(repo.release("tag", "0.1.0-rc.1", "--commit", older, "--dry-run").status).toBe(0);
    const wrong = repo.release("tag", "0.2.0-rc.1", "--commit", older, "--dry-run");
    expect(wrong.status).toBe(1);
    expect(wrong.stderr).toContain("doesn't name");
    expect(wrong.stdout).toBe("");
  });

  it("refuses any tag, a release candidate's too, without its stored database or released.txt line, before any step", () => {
    const repo = scratchRepo();
    const bare = repo.commit(carrying("0.1.0"));
    const { [RELEASED]: lines, ...stored } = recorded("0.1.0");
    const unlisted = repo.commit(stored, bare);
    const listed = repo.commit({ [RELEASED]: lines }, unlisted);
    for (const version of ["0.1.0-rc.1", "0.1.0"]) {
      const none = repo.release("tag", version, "--commit", bare, "--dry-run");
      expect(none.status, version).toBe(1);
      expect(none.stderr, version).toContain("no stored database");
      expect(none.stdout, version).toBe("");

      const noLine = repo.release("tag", version, "--commit", unlisted, "--dry-run");
      expect(noLine.status, version).toBe(1);
      expect(noLine.stderr, version).toContain("no line");
    }

    // The final tag goes on the accepted candidate's commit, with no change.
    for (const version of ["0.1.0-rc.1", "0.1.0"]) {
      const ready = repo.release("all", version, "--commit", listed, "--dry-run");
      expect(ready.status, version).toBe(0);
      expect(ready.stdout, version).not.toContain("== records");
      expect(ready.stdout, version).toContain(
        `$ git tag -a v${version} -m "HoploDex ${version}" ${listed}`,
      );
    }
    // A dry run only prints it.
    expect(repo.git(["tag", "-l"])).toBe("");

    // Once the records are there, `records` says so and changes nothing.
    const again = repo.release("records", "0.1.0-rc.2", "--commit", listed);
    expect(again.status).toBe(0);
    expect(again.stdout).toContain("already has 0.1.0's lines");
    expect(repo.git(["status", "--porcelain"])).toBe("");

    // A step added to fix what rc.1's testing found: no tag until it has its line.
    const fix = repo.commit(
      { ...migrations("0001_a", "0002_fix"), "src-tauri/src/db/migrations/0002_fix.sql": "abc" },
      listed,
    );
    for (const version of ["0.1.0-rc.2", "0.1.0"]) {
      const unlisted = repo.release("tag", version, "--commit", fix, "--dry-run");
      expect(unlisted.status, version).toBe(1);
      expect(unlisted.stderr, version).toContain("no line for the step 0002_fix");
    }
    expect(repo.release("records", "0.1.0-rc.2", "--commit", fix).status).toBe(0);
    expect(fs.readFileSync(path.join(repo.dir, RELEASED), "utf8")).toBe(
      `${lines}0002_fix ${ABC} 0.1.0\n`,
    );
    const recordedFix = repo.commit({}, fix);
    for (const version of ["0.1.0-rc.2", "0.1.0"]) {
      expect(repo.release("tag", version, "--commit", recordedFix, "--dry-run").status).toBe(0);
    }
  });
});

type Step = {
  name?: string;
  uses?: string;
  env?: Record<string, string>;
  with?: Record<string, string>;
};

// Its own script, run by bash in a scratch checkout. Windows' `bash` may be WSL's.
describe.skipIf(process.platform === "win32")(
  "release.yml's tag check",
  { timeout: 60_000 },
  () => {
    it("requires the stored database and released.txt lines for it and every step, for every tag, a release candidate's too", () => {
      const release = yaml.load(
        fs.readFileSync(path.join(ROOT, ".github/workflows/release.yml"), "utf8"),
      ) as { jobs: Record<string, { steps: (Step & { run?: string })[] }> };
      const script = Object.values(release.jobs)
        .flatMap((j) => j.steps)
        .find(
          (s) => s.name === "Check the tag against the version and the migration records",
        )!.run!;
      const check = (files: Record<string, string>, tag: string) => {
        const dir = tempDir();
        writeAll(dir, files);
        writeAll(dir, {
          "scripts/release.mjs": fs.readFileSync(path.join(ROOT, "scripts/release.mjs"), "utf8"),
        });
        const r = spawnSync("bash", ["-e", "-c", script], {
          cwd: dir,
          encoding: "utf8",
          env: { ...process.env, TAG: tag },
        });
        return { status: r.status, out: r.stdout + r.stderr };
      };
      for (const tag of ["v0.1.0-rc.1", "v0.1.0"]) {
        const { [RELEASED]: lines, ...stored } = recorded("0.1.0");
        expect(check(carrying("0.1.0"), tag)).toMatchObject({ status: 1 });
        expect(check(carrying("0.1.0"), tag).out).toContain("no stored database");
        expect(check({ ...carrying("0.1.0"), ...stored }, tag).out).toContain("has no line");
        expect(check({ ...carrying("0.1.0", lines), ...stored }, tag).status, tag).toBe(0);
        expect(check({ ...carrying("0.1.0", lines), ...stored }, "v0.2.0-rc.1").out).toContain(
          "doesn't name",
        );
        const withFix = {
          ...carrying("0.1.0", lines),
          ...stored,
          ...migrations("0001_a", "0002_fix"),
        };
        expect(check(withFix, tag).out).toContain("no line for the step 0002_fix");
        expect(
          check({ ...withFix, [RELEASED]: `${lines}0002_fix ${ABC} 0.1.0\n` }, tag).status,
        ).toBe(0);
        expect(check(carrying("0.1.0"), "0.1.0").out).toContain("isn't v<version>");
      }
    });
  },
);

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

/**
 * A stand-in for gh on PATH, answering from `answers` by what the call asks
 * for, and logging every call. `releases` is the release list (one JSON
 * object a line, as the real call's --jq writes it), `commit-<tag>` a tag's
 * commit, `release.json` and `runs.json` the promote job's API reads, and
 * `assets/` what `release download` fetches. `verify-status` is what
 * `release verify-asset` exits with.
 */
function fakeGh(answers: Record<string, string>) {
  const dir = tempDir();
  writeAll(path.join(dir, "answers"), answers);
  const gh = path.join(dir, "bin/gh");
  writeAll(dir, {
    "bin/gh": `#!/usr/bin/env bash
a="$FAKE_GH/answers"
echo "$*" >> "$FAKE_GH/log"
case "$*" in
  api*/releases\\?*) cat "$a/releases" ;;
  api*/commits/*) s="$*"; t=\${s##*/commits/}; cat "$a/commit-\${t%% *}" ;;
  api*/releases/tags/*) cat "$a/release.json" ;;
  api*/actions/workflows/*) cat "$a/runs.json" ;;
  "release download "*) s="$*"; d=\${s##*--dir }; mkdir -p "\${d%% *}"; cp "$a"/assets/* "\${d%% *}/" ;;
  "release verify-asset "*) exit "$(cat "$a/verify-status" 2>/dev/null || echo 0)" ;;
  "release view "*) exit 1 ;;
  "release create "*|"release upload "*) ;;
  *) echo "fake gh: $*" >&2; exit 2 ;;
esac
`,
  });
  fs.chmodSync(gh, 0o755);
  return {
    env: { FAKE_GH: dir, PATH: `${path.dirname(gh)}${path.delimiter}${process.env.PATH}` },
    calls: () =>
      fs.existsSync(path.join(dir, "log"))
        ? fs.readFileSync(path.join(dir, "log"), "utf8").trim().split("\n")
        : [],
  };
}

const sha = (s: string) => s.repeat(40).slice(0, 40);
const published = (tag: string, commit: string, more = {}) => ({
  tag,
  commit,
  draft: false,
  prerelease: true,
  immutable: true,
  ...more,
});

describe("promoting a release candidate", () => {
  it("promotes the newest published candidate on the tag's commit, and builds when the version has none", () => {
    const [here, there] = [sha("a"), sha("b")];
    expect(candidateFor("0.1.1", here, [])).toBeNull();
    expect(candidateFor("0.1.1-rc.2", here, [published("v0.1.1-rc.1", here)])).toBeNull();
    // Another version's candidates don't count.
    expect(candidateFor("0.1.1", here, [published("v0.1.0-rc.1", there)])).toBeNull();
    const both = [
      published("v0.1.1-rc.2", here),
      published("v0.1.1-rc.10", here),
      published("v0.1.1-rc.1", there),
    ];
    expect(candidateFor("0.1.1", here, both)).toBe("v0.1.1-rc.10");
    expect(candidateFor("0.1.1", here, both, "0.1.1-rc.2")).toBe("v0.1.1-rc.2");
    expect(() => candidateFor("0.1.1", here, both, "0.1.1-rc.1")).toThrow(`built from ${there}`);
    expect(() => candidateFor("0.1.1", here, both, "0.1.1-rc.3")).toThrow(/isn't a published/);
    // A tag on another commit than the accepted candidate's builds nothing.
    expect(() => candidateFor("0.1.1", here, [published("v0.1.1-rc.1", there)])).toThrow(
      /on other commits/,
    );
    for (const more of [{ immutable: false }, { prerelease: false }])
      expect(() => candidateFor("0.1.1", here, [published("v0.1.1-rc.1", here, more)])).toThrow(
        /isn't an immutable pre-release/,
      );
    // A draft (a candidate not yet accepted) is no candidate.
    expect(
      candidateFor("0.1.1", here, [published("v0.1.1-rc.1", here, { draft: true })]),
    ).toBeNull();
  });

  it("tags the final on the candidate's commit after reviewed, gate and the audit, naming the candidate, and makes no release", () => {
    const promote = plan("promote", {
      version: "0.1.1",
      commit,
      candidate: "v0.1.1-rc.2",
      container: false,
    });
    const runs = promote.map((p: { run?: string[] }) => p.run).filter(Boolean);
    expect(runs).toEqual([
      ["node", "scripts/ai-security-review.mjs", "reviewed", "--commit", commit],
      ["node", "scripts/ai-security-review.mjs", "gate", "--version", "0.1.1"],
      ["bash", "-c", "npm run audit"],
      ["git", "tag", "-a", "v0.1.1", "-m", "HoploDex 0.1.1, promoted from v0.1.1-rc.2", commit],
      ["git", "push", "origin", "refs/tags/v0.1.1"],
    ]);
    expect(runs.flat().join(" ")).not.toMatch(/gh release/);
    expect(() =>
      plan("promote", { version: "0.1.1-rc.2", commit, candidate: "v0.1.1-rc.2" }),
    ).toThrow();
    expect(() => plan("promote", { version: "0.1.1", commit })).toThrow();
    expect(STEPS).not.toContain("promote");
  });

  it("writes the notes: which commit, how the files were made, and a candidate's reinstall line", () => {
    const template = fs.readFileSync(path.join(ROOT, ".github/release-notes.md"), "utf8");
    const built = releaseNotes(template, { version: "0.1.1", commit });
    expect(built).toMatch(/^# HoploDex 0\.1\.1\n/);
    expect(built).toContain(`Built from commit ${commit} by this tag's release run.`);
    expect(built).not.toMatch(/\{\{|release candidate/);
    const promoted = releaseNotes(template, {
      version: "0.1.1",
      commit,
      promotedFrom: "v0.1.1-rc.2",
    });
    expect(promoted).toContain(`Promoted from v0.1.1-rc.2, built from commit ${commit}`);
    expect(promoted).toContain("HoploDex-0.1.1-x86_64.deb");
    const rc = releaseNotes(template, { version: "0.1.1-rc.2", commit });
    expect(rc).toMatch(/^\*\*This is a release candidate/);
    expect(rc).toContain("sudo apt install --reinstall ./HoploDex-0.1.1-x86_64.deb");
    expect(rc).toContain("# HoploDex 0.1.1-rc.2\n");
    expect(rc).toContain("/blob/v0.1.1-rc.2/README.md");
  });
});

/** A candidate's release, its runs and its downloaded files, all as they should be. */
function goodCandidate(rc = "v0.1.1-rc.1", at = commit) {
  const app = parseVersion(rc.slice(1)).app;
  const dir = tempDir();
  const packages = releaseFiles(app).filter((f) => f !== "SHA256SUMS");
  for (const f of packages) fs.writeFileSync(path.join(dir, f), `the bytes of ${f}`);
  const hash = (f: string) =>
    createHash("sha256")
      .update(fs.readFileSync(path.join(dir, f)))
      .digest("hex");
  fs.writeFileSync(
    path.join(dir, "SHA256SUMS"),
    packages.map((f) => `${hash(f)}  ${f}\n`).join(""),
  );
  return {
    rc,
    commit: at,
    dir,
    rehearsal: false,
    release: {
      tag_name: rc,
      draft: false,
      prerelease: true,
      immutable: true,
      assets: releaseFiles(app).map((name) => ({
        name,
        uploader: { login: "github-actions[bot]" },
      })),
    },
    runs: {
      workflow_runs: [{ head_sha: at, head_branch: rc, event: "push", conclusion: "success" }],
    },
  };
}

describe("a candidate's files", () => {
  it("pass only as a published, immutable pre-release's, uploaded by a release run that passed on the commit, matching SHA256SUMS", () => {
    expect(candidateProblems(goodCandidate())).toEqual([]);
    const fails = (
      change: (c: ReturnType<typeof goodCandidate>) => void,
      problem: string | RegExp,
    ) => {
      const c = goodCandidate();
      change(c);
      expect(candidateProblems(c).join("\n")).toMatch(problem);
    };
    fails((c) => (c.release.draft = true), "isn't a published, immutable release");
    fails((c) => (c.release.immutable = false), "isn't a published, immutable release");
    fails((c) => (c.release.prerelease = false), "isn't a pre-release");
    fails((c) => (c.release.tag_name = "v0.1.1-rc.2"), "not v0.1.1-rc.1");
    fails((c) => (c.release.assets[0].uploader.login = "someone"), "uploaded by someone");
    fails((c) => c.release.assets.pop(), "files are");
    fails((c) => (c.runs.workflow_runs = []), "no release run");
    fails((c) => (c.runs.workflow_runs[0].head_sha = sha("d")), "no release run");
    fails((c) => (c.runs.workflow_runs[0].head_branch = "main"), "no release run");
    fails(
      (c) => fs.appendFileSync(path.join(c.dir, "HoploDex-0.1.1-x86_64.deb"), "x"),
      ".deb doesn't match",
    );
    fails((c) => fs.writeFileSync(path.join(c.dir, "extra"), ""), "the download has");
    fails(
      (c) => fs.writeFileSync(path.join(c.dir, "SHA256SUMS"), `${"0".repeat(64)}  ../elsewhere\n`),
      "SHA256SUMS lists",
    );
    fails((c) => fs.rmSync(path.join(c.dir, "SHA256SUMS")), "has no SHA256SUMS");
  });

  it("are checked in a rehearsal against an earlier final, whatever its files are named", () => {
    const c = { ...goodCandidate("v0.1.0"), rehearsal: true };
    c.release.prerelease = false;
    expect(candidateProblems(c)).toEqual([]);
    expect(candidateProblems({ ...c, rehearsal: false }).join("\n")).toContain(
      "isn't a pre-release",
    );
    c.release.immutable = false;
    expect(candidateProblems(c).join("\n")).toContain("immutable");
  });
});

type Job = {
  if?: string;
  needs?: string | string[];
  permissions?: Record<string, string>;
  steps?: (Step & { run?: string; if?: string; id?: string })[];
};
const releaseJobs = () =>
  (
    yaml.load(fs.readFileSync(path.join(ROOT, ".github/workflows/release.yml"), "utf8")) as {
      jobs: Record<string, Job>;
    }
  ).jobs;

describe("release.yml's two paths", () => {
  it("builds, tests and drafts only on the build path, and promotes only on the promote path", () => {
    const jobs = releaseJobs();
    for (const name of ["ci", "linux-surface", "windows-surface", "macos-surface"]) {
      expect(jobs[name].needs, name).toBe("review");
      expect(jobs[name].if, name).toBe("needs.review.outputs.path == 'build'");
    }
    expect(jobs.draft.if).toBe(
      "github.event_name == 'push' && needs.review.outputs.path == 'build'",
    );
    expect(jobs.promote.if).toBe("needs.review.outputs.path == 'promote'");
    expect(jobs.promote.needs).toEqual(["review", "checks"]);
    // The audit runs on both paths; the release build's test only when it builds.
    expect(jobs.checks.if).toBeUndefined();
    const checks = jobs.checks.steps!;
    expect(checks.find((s) => s.name === "Dependency audit")!.if).toBeUndefined();
    expect(checks.find((s) => s.name === "import_largest_test")!.if).toBe(
      "needs.review.outputs.path == 'build'",
    );
    const writers = Object.entries(jobs)
      .filter(([, j]) => j.permissions?.contents === "write")
      .map(([n]) => n);
    expect(writers.sort()).toEqual(["draft", "promote"]);
    const create = jobs.promote.steps!.find((s) => s.run?.includes("gh release create"))!;
    expect(create.if).toBe("github.event_name == 'push'");
  });

  const promoteSteps = () =>
    releaseJobs().promote.steps!.filter((s): s is Step & { run: string; if?: string } => !!s.run);

  /**
   * Runs the promote job's steps as Actions does, each with bash -e, stopping
   * at the first that fails, in a scratch checkout with gh stood in for.
   */
  function runPromote(c: ReturnType<typeof goodCandidate>, { push = true, verify = "0" } = {}) {
    const work = tempDir();
    writeAll(work, {
      "scripts/release.mjs": fs.readFileSync(path.join(ROOT, "scripts/release.mjs"), "utf8"),
      ".github/release-notes.md": fs.readFileSync(
        path.join(ROOT, ".github/release-notes.md"),
        "utf8",
      ),
    });
    const answers: Record<string, string> = {
      "release.json": JSON.stringify(c.release),
      "runs.json": JSON.stringify(c.runs),
      "verify-status": verify,
    };
    for (const f of fs.readdirSync(c.dir))
      answers[`assets/${f}`] = fs.readFileSync(path.join(c.dir, f), "utf8");
    const gh = fakeGh(answers);
    const temp = tempDir();
    const env = {
      ...process.env,
      ...gh.env,
      GH_TOKEN: "token",
      GITHUB_REPOSITORY: "owner/repo",
      GITHUB_STEP_SUMMARY: path.join(temp, "summary"),
      RUNNER_TEMP: temp,
      RC: c.rc,
      COMMIT: c.commit,
      TAG: "v0.1.1",
      REHEARSAL: String(!push),
    };
    for (const step of promoteSteps()) {
      if (step.if === "github.event_name == 'push'" && !push) continue;
      const r = spawnSync("bash", ["-e", "-c", step.run], { cwd: work, encoding: "utf8", env });
      if (r.status !== 0) return { ok: false, calls: gh.calls(), out: r.stdout + r.stderr, temp };
    }
    return { ok: true, calls: gh.calls(), out: "", temp };
  }

  const drafted = (calls: string[]) => calls.filter((c) => /^release (create|upload)/.test(c));

  it.skipIf(process.platform === "win32")(
    "drafts the final from the candidate's own files and checksums, after every check",
    { timeout: 60_000 },
    () => {
      const r = runPromote(goodCandidate());
      expect(r.out).toBe("");
      expect(r.ok).toBe(true);
      const [create] = drafted(r.calls);
      expect(create).toMatch(/^release create v0\.1\.1 /);
      for (const f of releaseFiles("0.1.1")) expect(create).toContain(`/candidate/${f}`);
      expect(create).toContain("--draft");
      expect(create).not.toContain("--prerelease");
      expect(r.calls.filter((c) => c.startsWith("release verify-asset v0.1.1-rc.1 "))).toHaveLength(
        6,
      );
      expect(fs.readFileSync(path.join(r.temp, "notes.md"), "utf8")).toContain(
        `Promoted from v0.1.1-rc.1, built from commit ${commit}`,
      );
    },
  );

  it.skipIf(process.platform === "win32")(
    "drafts nothing in a rehearsal, or when any check fails",
    { timeout: 60_000 },
    () => {
      const rehearsal = runPromote(goodCandidate(), { push: false });
      expect(rehearsal.ok).toBe(true);
      expect(rehearsal.calls.filter((c) => c.startsWith("release verify-asset"))).toHaveLength(6);
      expect(drafted(rehearsal.calls)).toEqual([]);

      const failing: [string, (c: ReturnType<typeof goodCandidate>) => void, string?][] = [
        ["a file someone uploaded", (c) => (c.release.assets[1].uploader.login = "someone")],
        [
          "a changed file",
          (c) => fs.appendFileSync(path.join(c.dir, "HoploDex-0.1.1-x86_64.rpm"), "x"),
        ],
        ["no passing run", (c) => (c.runs.workflow_runs = [])],
        ["a mutable release", (c) => (c.release.immutable = false)],
        ["an attestation that doesn't verify", () => {}, "1"],
      ];
      for (const [what, change, verify] of failing) {
        const c = goodCandidate();
        change(c);
        const r = runPromote(c, { verify });
        expect(r.ok, what).toBe(false);
        expect(drafted(r.calls), what).toEqual([]);
      }
    },
  );
});

describe.skipIf(process.platform === "win32")(
  "release script's promote and path, run on a repository",
  { timeout: 60_000 },
  () => {
    function ready() {
      const repo = scratchRepo();
      const head = repo.commit({ ...carrying("0.1.1"), ...recorded("0.1.1") });
      return { repo, head };
    }
    const listing = (...rs: object[]) => rs.map((r) => JSON.stringify(r)).join("\n");

    it("promotes the published candidate on the commit, and refuses without one, tagging nothing", () => {
      const { repo, head } = ready();
      const rc = { tag: "v0.1.1-rc.1", draft: false, prerelease: true, immutable: true };
      const gh = fakeGh({ releases: listing(rc), "commit-v0.1.1-rc.1": head });
      repo.env = gh.env;
      const ok = repo.release("promote", "0.1.1", "--commit", head, "--dry-run");
      expect(ok.stderr).toBe("");
      expect(ok.stdout).toContain("promoting v0.1.1-rc.1");
      expect(ok.stdout).toContain(
        `$ git tag -a v0.1.1 -m "HoploDex 0.1.1, promoted from v0.1.1-rc.1" ${head}`,
      );
      expect(ok.stdout).toContain("ai-security-review.mjs gate");
      expect(ok.stdout).toContain("npm run audit");

      const rcVersion = repo.release("promote", "0.1.1-rc.1", "--commit", head, "--dry-run");
      expect(rcVersion.stderr).toContain("takes the final version");

      repo.env = fakeGh({ releases: "" }).env;
      const none = repo.release("promote", "0.1.1", "--commit", head);
      expect(none.status).toBe(1);
      expect(none.stderr).toContain("no published release candidate");
      expect(none.stdout).not.toContain("==");

      repo.env = fakeGh({
        releases: listing({ ...rc, immutable: false }),
        "commit-v0.1.1-rc.1": head,
      }).env;
      const mutable = repo.release("promote", "0.1.1", "--commit", head);
      expect(mutable.status).toBe(1);
      expect(mutable.stderr).toContain("isn't an immutable pre-release");
      expect(repo.git(["tag", "-l"])).toBe("");
    });

    it("chooses the workflow's path: promote, build, or neither", () => {
      const { repo, head } = ready();
      const rc = { tag: "v0.1.1-rc.1", draft: false, prerelease: true, immutable: true };
      const path_ = (answers: Record<string, string>, ...args: string[]) => {
        repo.env = fakeGh(answers).env;
        return repo.release("path", ...args);
      };
      const promote = path_(
        { releases: listing(rc), "commit-v0.1.1-rc.1": head },
        "0.1.1",
        "--commit",
        head,
      );
      expect(promote.stdout).toContain(`path=promote\ncandidate=v0.1.1-rc.1\ncommit=${head}\n`);
      const build = path_({ releases: "" }, "0.1.1", "--commit", head);
      expect(build.stdout).toContain("path=build\ncandidate=\n");
      const candidate = path_(
        { releases: listing(rc), "commit-v0.1.1-rc.1": head },
        "0.1.1-rc.2",
        "--commit",
        head,
      );
      expect(candidate.stdout).toContain("path=build\n");
      const elsewhere = path_(
        { releases: listing(rc), "commit-v0.1.1-rc.1": sha("e") },
        "0.1.1",
        "--commit",
        head,
      );
      expect(elsewhere.status).toBe(1);
      expect(elsewhere.stdout).toBe("");
      const rehearsal = path_({ "commit-v0.1.0": sha("f") }, "0.1.0", "--rehearsal");
      expect(rehearsal.stdout).toContain(`path=promote\ncandidate=v0.1.0\ncommit=${sha("f")}\n`);
    });
  },
);
