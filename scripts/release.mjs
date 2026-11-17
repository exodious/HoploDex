// The manual half of a release (#19; RELEASING.md): the steps the release
// workflow can't do, run on the owner's own machine, and last the tag that
// starts the workflow. It never makes a GitHub release: the workflow opens a
// draft, which stays a draft until the owner publishes it.
//
//   node scripts/release.mjs <step> <version> [--commit <sha>] [--after <sha>] [--dry-run]
//
//   version   check the version is the same everywhere it is written, with a
//             metainfo <release> entry for it
//   records   add the version's migration records to released.txt in the working
//             tree, for a pull request merged before a release candidate: each
//             step not yet listed, then the stored database, which must exist,
//             unless it is listed already; with nothing to add, it says so
//   checks    the tests to run on the owner's machine: release_upgrade_test,
//             released_migrations_test and performance_test
//   review    start the AI security review of --commit (default: origin/main's
//             head) in Claude Code, with --after <reviewed commit> after fixes
//   gate      the release gate (`ai-security-review.mjs gate`)
//   tag       check the workflow will accept --commit (its tag check, read from
//             the commit, and `ai-security-review.mjs reviewed`), then create the
//             annotated tag v<version> on it and push it to origin, which starts
//             the release workflow
//   all       each of the above but records, in that order, the records being on
//             the commit already: `tag`'s check of the commit comes first of all
//   tag-check release.yml's tag check, of the working tree: the tag names the
//             app's version, and the commit has its stored database and a
//             released.txt line for it and for every step in MIGRATIONS
//
// <version> is the app's version with the tag's suffix, if any: `0.1.0` or
// `0.1.0-rc.1`. version, checks and all run on the working tree, so they refuse
// unless it is clean and HEAD is --commit. With --dry-run each command, the
// tag's included, is printed instead of run, and `all` stops at the first step
// that fails, so it tags only after every check. Commands that the development
// docs run in the dev container go through scripts/dev-container.sh when podman
// is here and this isn't the container already. What the release workflow runs
// (the Flatpak's checks, the dependency audit, import_largest_test) stays there:
// its manual dry run is the check before a tag, on every OS.

import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

export const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const RELEASED = "src-tauri/src/db/migrations/released.txt";
const MIGRATIONS = "src-tauri/src/db/mod.rs";
const METAINFO = "flatpak/io.github.exodious.HoploDex.metainfo.xml";
export const STEPS = ["version", "records", "checks", "review", "gate", "tag"];
/** The steps that run on the working tree, so it must be the commit they are for. */
const ON_THE_TREE = ["version", "checks"];

/** `0.1.0-rc.1` → { app: "0.1.0", final: false }. */
export function parseVersion(version) {
  const m = /^(\d+\.\d+\.\d+)(-[0-9A-Za-z.-]+)?$/.exec(version ?? "");
  if (!m) throw new Error(`not a version: ${version} (such as 0.1.0 or 0.1.0-rc.1)`);
  return { app: m[1], final: !m[2], tag: `v${version}` };
}

const read = (root, file) => fs.readFileSync(path.join(root, file), "utf8");

/** Where the app's version is written, and what each says. */
export function versionsIn(root) {
  const lock = JSON.parse(read(root, "package-lock.json"));
  const cargoLock = /\[\[package\]\]\nname = "hoplodex"\nversion = "([^"]+)"/.exec(
    read(root, "src-tauri/Cargo.lock"),
  );
  return {
    "package.json": JSON.parse(read(root, "package.json")).version,
    "package-lock.json": lock.version,
    'package-lock.json packages[""]': lock.packages?.[""]?.version,
    "src-tauri/Cargo.toml": /^version = "([^"]+)"/m.exec(read(root, "src-tauri/Cargo.toml"))?.[1],
    "src-tauri/Cargo.lock": cargoLock?.[1],
    "src-tauri/tauri.conf.json": JSON.parse(read(root, "src-tauri/tauri.conf.json")).version,
    // The newest entry comes first.
    [`${METAINFO} (newest <release>)`]: /<release version="([^"]+)"/.exec(
      read(root, METAINFO),
    )?.[1],
  };
}

export function versionProblems(root, version) {
  const { app } = parseVersion(version);
  return Object.entries(versionsIn(root))
    .filter(([, v]) => v !== app)
    .map(([file, v]) => `${file} says ${v ?? "nothing"}, not ${app}`);
}

/** The step names in `MIGRATIONS`' order, from src-tauri/src/db/mod.rs's text. */
export function migrationSteps(text) {
  return [...text.matchAll(/\(\s*"(\d{4}_[a-z0-9_]+)",\s*include_str!/g)].map((m) => m[1]);
}

const sha256 = (file) => createHash("sha256").update(fs.readFileSync(file)).digest("hex");

/** released.txt's lines, each split into its fields. */
const listedIn = (text) =>
  text
    .split("\n")
    .filter((l) => l.trim() && !l.startsWith("#"))
    .map((l) => l.split(/\s+/));

/**
 * The lines a version adds to released.txt before a release candidate: each
 * step not yet listed, in `MIGRATIONS`' order, then its stored database unless
 * that is listed. None when every one is (a later candidate with no new step,
 * the final).
 */
export function recordLines(root, version) {
  const { app } = parseVersion(version);
  const done = new Set(listedIn(read(root, RELEASED)).map((f) => f[0]));
  const lines = migrationSteps(read(root, MIGRATIONS))
    .filter((s) => !done.has(s))
    .map(
      (s) => `${s} ${sha256(path.join(root, "src-tauri/src/db/migrations", `${s}.sql`))} ${app}`,
    );
  const fixture = `tests/fixtures/releases/${app}.hoplodex`;
  if (done.has(fixture)) return lines;
  const fixturePath = path.join(root, "src-tauri", fixture);
  if (!fs.existsSync(fixturePath)) {
    throw new Error(`no stored database ${fixturePath}: make it first (RELEASING.md, step 2)`);
  }
  return [...lines, `${fixture} ${sha256(fixturePath)} ${app}`];
}

/**
 * What release.yml's tag check refuses about a tag of `version`, reading the
 * tagged commit's files through `read` (text) and `has` (whether it exists).
 */
export function tagProblems(version, { read: show, has }) {
  const { app, tag } = parseVersion(version);
  const carried = JSON.parse(show("src-tauri/tauri.conf.json")).version;
  if (carried !== app) return [`${tag} doesn't name the app's version, ${carried}`];
  const fixture = `tests/fixtures/releases/${app}.hoplodex`;
  const listed = listedIn(show(RELEASED));
  const names = new Set(listed.map((f) => f[0]));
  return [
    ...(has(`src-tauri/${fixture}`) ? [] : [`no stored database src-tauri/${fixture}`]),
    ...(listed.some((f) => f[0] === fixture && /^[0-9a-f]{64}$/.test(f[1]) && f[2] === app)
      ? []
      : [`released.txt has no line for ${fixture}`]),
    ...migrationSteps(show(MIGRATIONS))
      .filter((s) => !names.has(s))
      .map((s) => `released.txt has no line for the step ${s}`),
  ];
}

const inContainer = () =>
  process.env.HOSTNAME === "hoplodex-dev" || fs.existsSync("/run/.containerenv");
const hasPodman = () => spawnSync("podman", ["--version"], { stdio: "ignore" }).status === 0;

/**
 * A command line for bash, through the dev container when the docs run it
 * there and it applies (CLAUDE.md, "Run tests in the dev container").
 */
export function containerised(cmd, { container = !inContainer() && hasPodman() } = {}) {
  const line = ["bash", "-c", cmd.join(" ")];
  return container ? ["scripts/dev-container.sh", ...line] : line;
}

const MANIFEST = ["--manifest-path", "src-tauri/Cargo.toml"];
const UNWIND = ["env", "CARGO_PROFILE_RELEASE_PANIC=unwind"];

/** What each step runs: `run` commands, or a `print` the owner acts on. */
export function plan(step, { version, commit, after, container }) {
  const { final, tag } = parseVersion(version);
  const c = (cmd) => ({ run: containerised(cmd, { container }) });
  switch (step) {
    case "version":
    case "records":
      return [{ internal: step }];
    case "checks":
      return [
        c([
          "cargo",
          "nextest",
          "run",
          ...MANIFEST,
          "--features",
          "test-kdf",
          "-E",
          "'binary(release_upgrade_test) or test(/^released_migrations_test::/)'",
        ]),
        c([
          ...UNWIND,
          "cargo",
          "test",
          ...MANIFEST,
          "--release",
          "--features",
          "upgrade-test-steps",
          "--test",
          "performance_test",
          "--",
          "--ignored",
          "--nocapture",
          "--test-threads=1",
        ]),
      ];
    case "review":
      return [
        {
          run: [
            "claude",
            `/ai-security-review release ${commit}${after ? ` --after ${after}` : ""}`,
          ],
        },
      ];
    case "gate":
      return [{ run: ["node", "scripts/ai-security-review.mjs", "gate"] }];
    case "tag":
      return [
        { run: ["node", "scripts/ai-security-review.mjs", "reviewed", "--commit", commit] },
        { run: ["git", "tag", "-a", tag, "-m", `HoploDex ${version}`, commit] },
        { run: ["git", "push", "origin", `refs/tags/${tag}`] },
        {
          print: [
            `Pushed ${tag}: the release workflow drafts the ${final ? "release" : "pre-release"}; publish it once you approve it.`,
          ],
        },
      ];
    default:
      throw new Error(`no step ${step} (${[...STEPS, "all"].join(", ")})`);
  }
}

export function steps(step) {
  return step === "all" ? STEPS.filter((s) => s !== "records") : [step];
}

function git(args) {
  return execFileSync("git", args, { cwd: ROOT, encoding: "utf8" }).trim();
}

function requireCheckoutAt(commit) {
  const status = git(["status", "--porcelain"]);
  if (status) throw new Error(`the working tree isn't clean:\n${status}`);
  const head = git(["rev-parse", "HEAD"]);
  if (head !== commit) throw new Error(`HEAD is ${head}, not ${commit}: check that commit out`);
}

function requireNoProblems(problems, of) {
  if (problems.length) throw new Error(`${of} fails the tag check:\n  ${problems.join("\n  ")}`);
}

/** Refuses, from `commit` itself, what release.yml's tag check would. */
function requireTaggable(version, commit) {
  const has = (f) =>
    spawnSync("git", ["cat-file", "-e", `${commit}:${f}`], { cwd: ROOT, stdio: "ignore" })
      .status === 0;
  requireNoProblems(
    tagProblems(version, { read: (f) => git(["show", `${commit}:${f}`]), has }),
    commit,
  );
}

function main(argv) {
  const { values, positionals } = parseArgs({
    args: argv,
    allowPositionals: true,
    options: {
      commit: { type: "string" },
      after: { type: "string" },
      "dry-run": { type: "boolean" },
    },
  });
  const [step, version, ...rest] = positionals;
  if (!step || !version || rest.length) {
    throw new Error(
      "usage: release.mjs <step> <version> [--commit <sha>] [--after <sha>] [--dry-run]",
    );
  }
  parseVersion(version);
  if (step === "tag-check") {
    const has = (f) => fs.existsSync(path.join(ROOT, f));
    requireNoProblems(tagProblems(version, { read: (f) => read(ROOT, f), has }), "the checkout");
    console.log(`v${version} passes the tag check`);
    return;
  }
  const dryRun = values["dry-run"];
  const commit = git(["rev-parse", "--verify", `${values.commit ?? "origin/main"}^{commit}`]);
  const toRun = steps(step);
  if (toRun.some((s) => ON_THE_TREE.includes(s))) requireCheckoutAt(commit);
  if (toRun.includes("tag")) requireTaggable(version, commit);
  for (const s of toRun) {
    console.log(`\n== ${s}`);
    for (const p of plan(s, { version, commit, after: values.after })) {
      if (p.print) {
        if (!dryRun) console.log(p.print.join("\n"));
      } else if (p.internal === "version") {
        const problems = versionProblems(ROOT, version);
        if (problems.length)
          throw new Error(`the version isn't ${version} everywhere:\n  ${problems.join("\n  ")}`);
        console.log(`${parseVersion(version).app} everywhere`);
      } else if (p.internal === "records") {
        const lines = recordLines(ROOT, version);
        if (!lines.length) {
          console.log(`${RELEASED} already has ${parseVersion(version).app}'s lines`);
          continue;
        }
        console.log(`${dryRun ? "would add" : "adding"} to ${RELEASED}:\n  ${lines.join("\n  ")}`);
        if (!dryRun) fs.appendFileSync(path.join(ROOT, RELEASED), `${lines.join("\n")}\n`);
      } else {
        console.log(`$ ${p.run.map((a) => (/\s/.test(a) ? JSON.stringify(a) : a)).join(" ")}`);
        if (dryRun) continue;
        const r = spawnSync(p.run[0], p.run.slice(1), { cwd: ROOT, stdio: "inherit" });
        if (r.status !== 0) throw new Error(`${s} failed (exit ${r.status ?? r.signal})`);
      }
    }
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main(process.argv.slice(2));
  } catch (e) {
    console.error(e.message);
    process.exitCode = 1;
  }
}
