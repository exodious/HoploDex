// The manual half of a release (#19; RELEASING.md): the steps the release
// workflow can't do, run on the owner's own machine, and last the tag that
// starts the workflow. It never makes a GitHub release: the workflow opens a
// draft, which stays a draft until the owner publishes it.
//
//   node scripts/release.mjs <step> <version> [--commit <sha>] [--after <sha>] [--dry-run]
//
//   version   check the version is the same everywhere it is written, with a
//             metainfo <release> entry for it, and that each metainfo screenshot
//             address's commit has the working tree's image
//   records   add the version's migration records to released.txt in the working
//             tree, for a pull request merged before a release candidate: each
//             step not yet listed, then the stored database, which must exist,
//             unless it is listed already; with nothing to add, it says so
//   checks    the tests to run on the owner's machine: release_upgrade_test,
//             released_migrations_test and performance_test
//   review    start the AI security review of --commit (default: origin/main's
//             head) in Claude Code, with --after <reviewed commit> after fixes
//   gate      the release gate (`ai-security-review.mjs gate --version <x.y.z>`)
//   tag       check the workflow will accept --commit (its tag check, read from
//             the commit, and `ai-security-review.mjs reviewed`), then create the
//             annotated tag v<version> on it and push it to origin, which starts
//             the release workflow
//   all       each of the above but records, in that order, the records being on
//             the commit already: `tag`'s check of the commit comes first of all
//   promote   instead of `tag`, for a final release from an accepted release
//             candidate: find the newest published v<version>-rc.<n> on --commit
//             (or --rc <version>-rc.<n>), which must be an immutable pre-release,
//             run `reviewed`, `gate` and `npm run audit`, then tag v<version> on
//             it, which has the release workflow promote that candidate's files
//   tag-check release.yml's tag check, of the working tree: the tag names the
//             app's version, and the commit has its stored database and a
//             released.txt line for it and for every step in MIGRATIONS
//
// release.yml also runs `path` (build, or promote a candidate), `promote-check`
// (the candidate's files against its release) and `notes` (the draft's notes).
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
const NOTES = ".github/release-notes.md";
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

const SCREENSHOT =
  /^https:\/\/raw\.githubusercontent\.com\/exodious\/HoploDex\/([0-9a-f]{40})\/flatpak\/screenshots\/([^/]+)$/;

/**
 * Each metainfo screenshot address is at a commit (011 research.md §13) in the
 * released commit's history, so GitHub keeps serving it, and the file there must
 * be the working tree's, or a software centre shows an old image. `merged(sha)`
 * says whether that commit is in the history, and `show(rev)` is `git show <rev>`'s
 * bytes, or null when it has none. CI's shallow checkout can't see that commit, so
 * this runs here and not in metainfo.test.ts.
 */
export function screenshotProblems(root, { show, merged }) {
  const addresses = [...read(root, METAINFO).matchAll(/<image[^>]*>([^<]*)<\/image>/g)];
  return addresses.flatMap(([, address]) => {
    const m = SCREENSHOT.exec(address.trim());
    if (!m) return [`${address.trim()} isn't a flatpak/screenshots/ file at a commit`];
    const [, sha, file] = m;
    if (!merged(sha))
      return [
        `${sha} isn't in the released commit's history: merge the images to main first, then point the addresses at that commit`,
      ];
    const there = show(`${sha}:flatpak/screenshots/${file}`);
    if (!there) return [`${sha} has no flatpak/screenshots/${file}`];
    const here = path.join(root, "flatpak/screenshots", file);
    if (!fs.existsSync(here) || !fs.readFileSync(here).equals(there))
      return [
        `flatpak/screenshots/${file} differs from ${sha}'s: point the address at the commit that changed it`,
      ];
    return [];
  });
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

/** The files a release of `app` publishes (#209), SHA256SUMS last. */
export const releaseFiles = (app) => [
  `HoploDex-${app}-x86_64-setup.exe`,
  `HoploDex-${app}-aarch64.dmg`,
  `HoploDex-${app}-x86_64.deb`,
  `HoploDex-${app}-x86_64.rpm`,
  `HoploDex-${app}-x86_64.flatpak`,
  "SHA256SUMS",
];

const RC = /^v(\d+\.\d+\.\d+)-rc\.(\d+)$/;
const SHA = /^[0-9a-f]{40}$/;

/**
 * The release candidate a final tag of `version` on `commit` promotes
 * (RELEASING.md, "Release candidates"), or null when the version has no
 * published candidate, so the tag builds its own. `releases` are the published
 * v<app>-rc.<n> releases, each with its tag's commit; the newest on `commit`
 * is taken, or `rc` (`0.1.1-rc.2`). Refuses one that isn't an immutable
 * pre-release, and a version whose published candidates are all on other
 * commits: that tag isn't on the accepted candidate.
 */
export function candidateFor(version, commit, releases, rc) {
  const { app, final } = parseVersion(version);
  if (!final) return null;
  const ours = releases.filter((r) => !r.draft && RC.exec(r.tag)?.[1] === app);
  const named = rc && ours.find((r) => r.tag === `v${rc}`);
  if (rc && !named) throw new Error(`v${rc} isn't a published release candidate of ${app}`);
  if (named && named.commit !== commit)
    throw new Error(`v${rc} was built from ${named.commit}, not ${commit}`);
  const here = named
    ? [named]
    : ours.filter((r) => r.commit === commit).sort((a, b) => RC.exec(b.tag)[2] - RC.exec(a.tag)[2]);
  if (!here.length) {
    if (!ours.length) return null;
    throw new Error(
      `${app}'s published release candidates are on other commits (${ours.map((r) => `${r.tag} on ${r.commit}`).join(", ")}): tag the accepted candidate's commit`,
    );
  }
  const [c] = here;
  if (!c.prerelease || !c.immutable)
    throw new Error(`${c.tag} isn't an immutable pre-release, so it can't be promoted`);
  return c.tag;
}

/**
 * What stops the promotion of release `rc` (`v0.1.1-rc.1`), whose files were
 * downloaded to `dir`: `release` is its GitHub release (the API's JSON) and
 * `runs` its successful release runs. It must be published and immutable, a
 * pre-release, with exactly a release's files, each uploaded by a release run,
 * which passed on `commit`, and SHA256SUMS must list every other file with its
 * hash. A rehearsal (`rehearsal`, an earlier final such as v0.1.0, whose files
 * have other names) checks all but the pre-release and the names.
 */
export function candidateProblems({ rc, commit, release, runs, dir, rehearsal }) {
  const { app } = parseVersion(rc.slice(1));
  const problems = [];
  if (release.tag_name !== rc) problems.push(`the release is ${release.tag_name}, not ${rc}`);
  if (release.draft || release.immutable !== true)
    problems.push(`${rc} isn't a published, immutable release`);
  if (!rehearsal && release.prerelease !== true) problems.push(`${rc} isn't a pre-release`);
  const names = (release.assets ?? []).map((a) => a.name).sort();
  const expected = releaseFiles(app).sort();
  if (!rehearsal && names.join("\n") !== expected.join("\n"))
    problems.push(`${rc}'s files are ${names.join(", ")}, not ${expected.join(", ")}`);
  for (const a of release.assets ?? [])
    if (a.uploader?.login !== "github-actions[bot]")
      problems.push(
        `${a.name} was uploaded by ${a.uploader?.login ?? "nobody"}, not a release run`,
      );
  const here = fs.readdirSync(dir).sort();
  if (here.join("\n") !== names.join("\n"))
    problems.push(`the download has ${here.join(", ")}, not ${rc}'s ${names.join(", ")}`);
  const passed = (runs.workflow_runs ?? []).some(
    (r) =>
      r.head_sha === commit &&
      r.head_branch === rc &&
      r.event === "push" &&
      r.conclusion === "success",
  );
  if (!passed) problems.push(`no release run of ${rc} passed on ${commit}`);
  if (!here.includes("SHA256SUMS")) return [...problems, `${rc} has no SHA256SUMS`];
  const listed = read(dir, "SHA256SUMS")
    .split("\n")
    .filter(Boolean)
    .map((l) => /^([0-9a-f]{64}) [ *](.+)$/.exec(l) ?? [l, null, null]);
  const packages = here.filter((f) => f !== "SHA256SUMS");
  if (
    listed
      .map((m) => m[2])
      .sort()
      .join("\n") !== packages.join("\n")
  )
    return [
      ...problems,
      `SHA256SUMS lists ${listed.map((m) => m[2] ?? m[0]).join(", ")}, not ${packages.join(", ")}`,
    ];
  for (const [, hash, f] of listed)
    if (sha256(path.join(dir, f)) !== hash) problems.push(`${f} doesn't match SHA256SUMS`);
  return problems;
}

/**
 * The draft release's notes: .github/release-notes.md with the version, tag
 * and where the files came from filled in, as data, and a release candidate's
 * note first.
 */
export function releaseNotes(template, { version, commit, promotedFrom }) {
  const { app, final, tag } = parseVersion(version);
  const from = promotedFrom
    ? `Promoted from ${promotedFrom}, built from commit ${commit}: the same files and checksums as that release candidate.`
    : `Built from commit ${commit} by this tag's release run.`;
  const candidate = final
    ? ""
    : `**This is a release candidate, for testing.** Try it on a copy of your collection.
Every candidate of ${app} carries the version ${app}, so installing it over an earlier candidate needs a reinstall:
\`sudo apt install --reinstall ./HoploDex-${app}-x86_64.deb\`, \`sudo dnf reinstall ./HoploDex-${app}-x86_64.rpm\`,
\`flatpak install --user --reinstall HoploDex-${app}-x86_64.flatpak\`, or the Windows installer's reinstall option.

`;
  return (
    candidate +
    template
      .replaceAll("{{version}}", version)
      .replaceAll("{{app_version}}", app)
      .replaceAll("{{tag}}", tag)
      .replaceAll("{{provenance}}", from)
  );
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
export function plan(step, { version, commit, after, candidate, container }) {
  const { app, final, tag } = parseVersion(version);
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
      return [{ run: ["node", "scripts/ai-security-review.mjs", "gate", "--version", app] }];
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
    case "promote":
      if (!final || !candidate) throw new Error(`promote takes a final version and its candidate`);
      return [
        { run: ["node", "scripts/ai-security-review.mjs", "reviewed", "--commit", commit] },
        { run: ["node", "scripts/ai-security-review.mjs", "gate", "--version", app] },
        // The one release check whose result changes with time, not the code.
        c(["npm", "run", "audit"]),
        {
          run: [
            "git",
            "tag",
            "-a",
            tag,
            "-m",
            `HoploDex ${version}, promoted from ${candidate}`,
            commit,
          ],
        },
        { run: ["git", "push", "origin", `refs/tags/${tag}`] },
        {
          print: [
            `Pushed ${tag}: the release workflow drafts the release from ${candidate}'s files; publish it once you approve it.`,
          ],
        },
      ];
    default:
      throw new Error(`no step ${step} (${[...STEPS, "promote", "all"].join(", ")})`);
  }
}

export function steps(step) {
  return step === "all" ? STEPS.filter((s) => s !== "records") : [step];
}

function git(args) {
  return execFileSync("git", args, { cwd: ROOT, encoding: "utf8" }).trim();
}

const gh = (args) => execFileSync("gh", args, { cwd: ROOT, encoding: "utf8" }).trim();
// gh fills in {owner}/{repo} from the checkout's remote.
const repo = () => process.env.GITHUB_REPOSITORY || "{owner}/{repo}";
const commitOf = (tag) => gh(["api", `repos/${repo()}/commits/${tag}`, "--jq", ".sha"]);

/** The published release candidates of `app`, each with its tag's commit. */
function publishedCandidates(app) {
  return gh([
    "api",
    "--paginate",
    `repos/${repo()}/releases?per_page=100`,
    "--jq",
    ".[] | select(.draft | not) | {tag: .tag_name, draft, prerelease, immutable}",
  ])
    .split("\n")
    .filter(Boolean)
    .map((l) => JSON.parse(l))
    .filter((r) => RC.exec(r.tag)?.[1] === app)
    .map((r) => ({ ...r, commit: commitOf(r.tag) }));
}

function requireSha(sha) {
  if (!SHA.test(sha ?? "")) throw new Error(`not a commit: ${sha}`);
  return sha;
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
      rc: { type: "string" },
      // release.yml's own steps.
      rehearsal: { type: "boolean" },
      release: { type: "string" },
      runs: { type: "string" },
      files: { type: "string" },
      "promoted-from": { type: "string" },
    },
  });
  const [step, version, ...rest] = positionals;
  if (!step || !version || rest.length) {
    throw new Error(
      "usage: release.mjs <step> <version> [--commit <sha>] [--after <sha>] [--rc <version>] [--dry-run]",
    );
  }
  const { app, tag, final } = parseVersion(version);
  if (step === "promote" && !final)
    throw new Error(`promote takes the final version, not ${version}`);
  if (step === "notes") {
    const template = read(ROOT, NOTES);
    const promotedFrom = values["promoted-from"];
    if (promotedFrom !== undefined && !RC.test(promotedFrom))
      throw new Error(`not a release candidate's tag: ${promotedFrom}`);
    process.stdout.write(
      releaseNotes(template, { version, commit: requireSha(values.commit), promotedFrom }),
    );
    return;
  }
  if (step === "path") {
    // release.yml's review job: build the packages, or promote a candidate.
    // Its lines go to GITHUB_OUTPUT.
    const out = (p, candidate = "", commit = "", summary) =>
      console.log(`path=${p}\ncandidate=${candidate}\ncommit=${commit}\nsummary=${summary}`);
    if (values.rehearsal) {
      out("promote", tag, requireSha(commitOf(tag)), `A rehearsal of promoting ${tag}: no draft.`);
      return;
    }
    const commit = requireSha(values.commit);
    const candidate = final && candidateFor(version, commit, publishedCandidates(app));
    if (candidate)
      out("promote", candidate, commit, `${tag} promotes ${candidate}: nothing is built.`);
    else
      out(
        "build",
        "",
        commit,
        final
          ? `${tag} builds its packages: ${app} has no published release candidate.`
          : `${tag}, a release candidate, builds its packages.`,
      );
    return;
  }
  if (step === "promote-check") {
    const json = (f) => JSON.parse(fs.readFileSync(f, "utf8"));
    const problems = candidateProblems({
      rc: tag,
      commit: requireSha(values.commit),
      release: json(values.release),
      runs: json(values.runs),
      dir: values.files,
      rehearsal: values.rehearsal,
    });
    if (problems.length) throw new Error(`${tag} can't be promoted:\n  ${problems.join("\n  ")}`);
    console.log(`${tag}'s files are its release's, checked against its SHA256SUMS`);
    return;
  }
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
  if (toRun.includes("tag") || toRun.includes("promote")) requireTaggable(version, commit);
  let candidate;
  if (step === "promote") {
    if (values.rc) parseVersion(values.rc);
    candidate = candidateFor(version, commit, publishedCandidates(app), values.rc);
    if (!candidate)
      throw new Error(
        `${app} has no published release candidate on ${commit}: publish the accepted one's pre-release first, or tag the final itself (release.mjs tag)`,
      );
    console.log(`promoting ${candidate}`);
  }
  for (const s of toRun) {
    console.log(`\n== ${s}`);
    for (const p of plan(s, { version, commit, after: values.after, candidate })) {
      if (p.print) {
        if (!dryRun) console.log(p.print.join("\n"));
      } else if (p.internal === "version") {
        const problems = versionProblems(ROOT, version);
        if (problems.length)
          throw new Error(`the version isn't ${version} everywhere:\n  ${problems.join("\n  ")}`);
        console.log(`${parseVersion(version).app} everywhere`);
        const shots = screenshotProblems(ROOT, {
          show: (rev) => {
            const r = spawnSync("git", ["show", rev], {
              cwd: ROOT,
              stdio: ["ignore", "pipe", "ignore"],
            });
            return r.status === 0 ? r.stdout : null;
          },
          merged: (sha) =>
            spawnSync("git", ["merge-base", "--is-ancestor", sha, commit], {
              cwd: ROOT,
              stdio: "ignore",
            }).status === 0,
        });
        if (shots.length)
          throw new Error(`the metainfo's screenshot addresses:\n  ${shots.join("\n  ")}`);
        console.log(
          "the screenshot addresses are at commits in its history with the working tree's images",
        );
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
