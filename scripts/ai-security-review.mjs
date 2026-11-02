// The deterministic half of the AI security review (#21; skill
// .claude/skills/ai-security-review/): it turns the reviewers' findings into
// SARIF, uploads it to code scanning and checks what came of it, so the model
// never writes SARIF, hashes or API calls by hand. Findings live in code
// scanning, never in a committed file (constitution, "Development Workflow &
// Quality Gates").
//
//   node scripts/ai-security-review.mjs touched --base <sha> [--commit <sha>]
//       the surfaces whose **Code** claims a file the diff changes, and the
//       changed files no entry claims
//   node scripts/ai-security-review.mjs open --ref <ref> [--seed-prs]
//       the tool's open and dismissed alerts on <ref>, as JSON; --seed-prs adds
//       those still open on pull requests merged since <ref>'s last analysis
//   node scripts/ai-security-review.mjs build <findings.json> --mode pr|release
//       --commit <sha> [--known <open.json>[,…]] [--fixed <alert>,…]
//       [--base <sha> --surface unchanged|changed]
//       validates the findings, gives each a rule id and fingerprint, writes the
//       SARIF and a record beside it (pr mode prints the PR body's section)
//   node scripts/ai-security-review.mjs upload --pr <n> | --release [--ref <ref>]
//       uploads the recorded SARIF, waits for it, and checks the alerts
//   node scripts/ai-security-review.mjs gate [--ref <ref>]
//       the release gate: open critical and high alerts, and draft advisories
//   node scripts/ai-security-review.mjs carry-dismissal <new alert> <old alert>
//       dismisses <new> with <old>'s reason (a moved finding; ask first)
//
// What the live tests in exodious/hoplodex-sarif-scratch showed (#21's pull
// request): a custom primaryLocationLineHash is honoured; a result left out
// closes its alert as fixed and reopens the same alert when it returns; a
// changed security-severity changes the alert's band; a dismissal survives
// re-upload, absence and return; and a pull request upload under a category
// `main` doesn't have shows a public "configuration not found" warning, so
// both modes share one category.

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";
import { globToRegExp, LIST, parseList } from "./attack-surface.mjs";

export const REPO = "exodious/HoploDex";
/** Never change these: a new name or category orphans every existing alert. */
export const TOOL = "HoploDex AI security review";
export const CATEGORY = "hoplodex-ai-review";

/** Fixed scores, so a band can't flip on a reviewer's 6.9 against 7.4. */
export const BANDS = {
  critical: { score: "9.5", level: "error", problem: "error" },
  high: { score: "8.0", level: "error", problem: "error" },
  medium: { score: "5.5", level: "warning", problem: "warning" },
  low: { score: "2.0", level: "note", problem: "recommendation" },
};

/** The PR body's lines, which the hook and .github/workflows/pr-body.yml check. */
export const BODY = {
  commit: /^[ \t]*(?:[-*][ \t]+)?Reviewed commit: ([0-9a-f]{40})[ \t]*$/m,
  counts:
    /^[ \t]*(?:[-*][ \t]+)?Findings uploaded to code scanning: \d+ \(critical \d+, high \d+, medium \d+, low \d+\)[ \t]*$/m,
  surface: /^[ \t]*(?:[-*][ \t]+)?Attack surface: (unchanged|changed)[ \t]*$/m,
};

/** `<surface>/cwe-<n>/<path>#<symbol>`, with `::` written as `.`. */
export function baseRuleId(f) {
  return `${f.surface}/cwe-${f.cwe}/${f.path}#${f.symbol.replaceAll("::", ".")}`;
}

/** Code scanning keys an alert on this; it follows from the rule id alone. */
export function fingerprint(ruleId) {
  return `${createHash("sha256").update(ruleId).digest("hex").slice(0, 16)}:1`;
}

/** Every problem with one finding, as text. */
export function findingProblems(f, { mode, surfaces }) {
  const problems = [];
  const text = (v) => typeof v === "string" && v.trim() !== "";
  if (!surfaces.has(f.surface)) problems.push(`surface \`${f.surface}\` isn't in ${LIST}`);
  if (!Number.isInteger(f.cwe) || f.cwe <= 0) problems.push("cwe must be a positive integer");
  if (
    !text(f.path) ||
    f.path.startsWith("/") ||
    f.path.split("/").includes("..") ||
    /[\s#]/.test(f.path)
  ) {
    problems.push("path must be repository-relative, with no `..`, space or `#`");
  }
  if (!text(f.symbol) || /\s/.test(f.symbol))
    problems.push("symbol must be one word (`file` for a whole file)");
  const r = f.region ?? {};
  if (
    !Number.isInteger(r.startLine) ||
    r.startLine < 1 ||
    !Number.isInteger(r.endLine) ||
    r.endLine < r.startLine
  ) {
    problems.push("region needs startLine >= 1 and endLine >= startLine");
  }
  if (!(f.severity in BANDS))
    problems.push(`severity must be one of ${Object.keys(BANDS).join(", ")}`);
  if (!text(f.title) || f.title.length > 255) problems.push("title must be 1 to 255 characters");
  if (!text(f.message)) problems.push("message is required");
  if (f.rule_id !== undefined && f.rule_id !== null) {
    // A carried alert keeps its id; its path and symbol are part of it.
    const prefix = `${f.surface}/cwe-${f.cwe}/${f.path}#`;
    if (typeof f.rule_id !== "string" || !f.rule_id.startsWith(prefix)) {
      problems.push(
        `rule_id must start with \`${prefix}\` (a move or reclassification is a new finding)`,
      );
    }
  }
  // 1.5.0: a per-PR upload carries only what the pull request adds or changes.
  if (mode === "pr" && f.introduced !== true) {
    problems.push(
      "a pre-existing weakness never goes on a pull request: route it privately instead",
    );
  }
  return problems;
}

/**
 * The findings with their rule ids: a carried one keeps its own; a new one
 * gets its base id, or `~2`, `~3`… when that's taken by another finding or a
 * known alert (open or dismissed) the verifier said it isn't.
 */
export function assignRuleIds(findings, knownIds = []) {
  const taken = new Set(knownIds);
  for (const f of findings) if (f.rule_id) taken.add(f.rule_id);
  const seen = new Set();
  return findings.map((f) => {
    let id = f.rule_id;
    if (!id) {
      const base = baseRuleId(f);
      id = base;
      for (let k = 2; taken.has(id); k++) id = `${base}~${k}`;
      taken.add(id);
    }
    if (seen.has(id)) throw new Error(`two findings share the rule id ${id}`);
    seen.add(id);
    return { ...f, rule_id: id };
  });
}

/** One run, one rule per finding (severity is per rule). */
export function buildSarif(findings, { runId }) {
  return {
    $schema: "https://json.schemastore.org/sarif-2.1.0.json",
    version: "2.1.0",
    runs: [
      {
        tool: {
          driver: {
            name: TOOL,
            semanticVersion: "1.0.0",
            rules: findings.map((f) => ({
              id: f.rule_id,
              name: f.title,
              shortDescription: { text: f.title },
              fullDescription: { text: `Surface: ${f.surface}. CWE-${f.cwe}.` },
              defaultConfiguration: { level: BANDS[f.severity].level },
              help: { text: f.help_markdown ?? f.message, markdown: f.help_markdown ?? f.message },
              properties: {
                tags: ["security", `external/cwe/cwe-${f.cwe}`, `surface/${f.surface}`],
                precision: "high",
                "problem.severity": BANDS[f.severity].problem,
                "security-severity": BANDS[f.severity].score,
              },
            })),
          },
        },
        automationDetails: { id: `${CATEGORY}/${runId}` },
        results: findings.map((f, i) => ({
          ruleId: f.rule_id,
          ruleIndex: i,
          level: BANDS[f.severity].level,
          message: { text: f.message },
          locations: [
            {
              physicalLocation: {
                artifactLocation: { uri: f.path },
                region: { startLine: f.region.startLine, endLine: f.region.endLine },
              },
            },
          ],
          partialFingerprints: { primaryLocationLineHash: fingerprint(f.rule_id) },
        })),
      },
    ],
  };
}

export function counts(findings) {
  const c = { critical: 0, high: 0, medium: 0, low: 0 };
  for (const f of findings) c[f.severity]++;
  return c;
}

/** The `## Security review` lines for a pull request: the commit and counts, never a finding. */
export function bodyLines(commit, findings, surface) {
  const c = counts(findings);
  return [
    `- Reviewed commit: ${commit}`,
    `- Findings uploaded to code scanning: ${findings.length} (critical ${c.critical}, high ${c.high}, medium ${c.medium}, low ${c.low})`,
    `- Attack surface: ${surface}`,
  ];
}

// ---- GitHub ----

/** `gh api` with JSON out; `input` goes to stdin. */
export function realGh(args, input) {
  const out = execFileSync("gh", ["api", ...args], {
    input,
    encoding: "utf8",
    maxBuffer: 64 << 20,
    stdio: ["pipe", "pipe", "inherit"],
  });
  return out.trim() === "" ? null : JSON.parse(out);
}

/** `--paginate` prints one array per page; `--slurp` joins them. */
function paged(gh, endpoint) {
  return (gh(["--paginate", "--slurp", endpoint]) ?? []).flat();
}

const enc = encodeURIComponent;

function toAlert(a) {
  const loc = a.most_recent_instance?.location ?? {};
  return {
    number: a.number,
    rule_id: a.rule.id,
    state: a.state,
    severity: a.rule.security_severity_level,
    dismissed_reason: a.dismissed_reason ?? null,
    path: loc.path,
    start_line: loc.start_line,
    message: a.most_recent_instance?.message?.text ?? "",
    ref: a.most_recent_instance?.ref,
  };
}

export function alertsOn(gh, ref, state) {
  return paged(
    gh,
    `repos/${REPO}/code-scanning/alerts?tool_name=${enc(TOOL)}&ref=${enc(ref)}&state=${state}&per_page=100`,
  ).map(toAlert);
}

/**
 * The alerts to carry forward on `ref` (open, and dismissed as context), plus,
 * with `seedPrs`, any still open on a pull request merged after `ref`'s last
 * analysis: a finding merged unfixed isn't on `ref` until a review uploads it.
 */
export function openAlerts(gh, ref, { seedPrs = false } = {}) {
  const alerts = [...alertsOn(gh, ref, "open"), ...alertsOn(gh, ref, "dismissed")];
  if (!seedPrs) return alerts;
  const analyses = paged(
    gh,
    `repos/${REPO}/code-scanning/analyses?tool_name=${enc(TOOL)}&per_page=100`,
  );
  const last =
    analyses
      .filter((a) => a.ref === ref)
      .map((a) => a.created_at)
      .sort()
      .at(-1) ?? "";
  const have = new Set(alerts.map((a) => a.rule_id));
  const prs = new Set();
  for (const a of analyses) {
    const m = /^refs\/pull\/(\d+)\/head$/.exec(a.ref);
    if (m) prs.add(Number(m[1]));
  }
  for (const n of [...prs].sort((a, b) => a - b)) {
    const pr = gh([`repos/${REPO}/pulls/${n}`]);
    if (!pr.merged_at || pr.merged_at <= last) continue;
    for (const a of alertsOn(gh, `refs/pull/${n}/head`, "open")) {
      if (!have.has(a.rule_id)) alerts.push({ ...a, seeded_from: n });
      have.add(a.rule_id);
    }
  }
  return alerts;
}

/**
 * Upload, wait for processing, then check the analysis and every alert. An
 * upload closes, as fixed, every open alert it leaves out, so the open alerts
 * are read first: one the upload closed must be among `fixed` (the rule ids
 * of the verified fixes), or it's reported, to reopen by uploading it again.
 */
export function upload(gh, { sarif, ref, commit, fixed = [], sleep = sleepSeconds, tries = 40 }) {
  if (!/^refs\/heads\/[^\s]+$/.test(ref) && !/^refs\/pull\/\d+\/head$/.test(ref)) {
    // An alert on a tag can never close: no later analysis reaches its commit.
    throw new Error(`upload to a branch or refs/pull/<n>/head only, not ${ref}`);
  }
  if (!/^[0-9a-f]{40}$/.test(commit)) throw new Error(`not a full commit sha: ${commit}`);
  const results = sarif.runs[0].results;
  const before = alertsOn(gh, ref, "open");
  const posted = gh(
    ["--method", "POST", `repos/${REPO}/code-scanning/sarifs`, "--input", "-"],
    JSON.stringify({
      commit_sha: commit,
      ref,
      tool_name: TOOL,
      validate: true,
      sarif: gzipSync(JSON.stringify(sarif)).toString("base64"),
    }),
  );
  let status;
  for (let i = 0; ; i++) {
    sleep(Math.min(2 + i, 15));
    status = gh([`repos/${REPO}/code-scanning/sarifs/${posted.id}`]);
    if (status.processing_status !== "pending") break;
    if (i + 1 >= tries) throw new Error(`upload ${posted.id} still pending`);
  }
  if (status.processing_status !== "complete") {
    throw new Error(
      `upload ${posted.id} ${status.processing_status}: ${JSON.stringify(status.errors)}`,
    );
  }
  const problems = [];
  const analysis = (gh([`repos/${REPO}/code-scanning/analyses?sarif_id=${posted.id}`]) ?? [])[0];
  if (!analysis) problems.push("no analysis for the upload");
  else {
    if (analysis.results_count !== results.length) {
      problems.push(`the analysis has ${analysis.results_count} results, not ${results.length}`);
    }
    if (analysis.tool?.name !== TOOL)
      problems.push(`the analysis's tool is ${analysis.tool?.name}`);
    if (analysis.category !== CATEGORY)
      problems.push(`the analysis's category is ${analysis.category}`);
  }
  // Every uploaded rule has a live alert, only verified fixes closed, and
  // nothing open was left behind.
  const live = [...alertsOn(gh, ref, "open"), ...alertsOn(gh, ref, "dismissed")];
  const byRule = new Map(live.map((a) => [a.rule_id, a]));
  const uploaded = new Set(results.map((r) => r.ruleId));
  const verified = new Set(fixed);
  for (const id of uploaded)
    if (!byRule.has(id)) problems.push(`no open or dismissed alert for ${id}`);
  for (const a of before) {
    if (!uploaded.has(a.rule_id) && !verified.has(a.rule_id)) {
      problems.push(
        `#${a.number} ${a.rule_id} was open and this upload closed it, but it isn't a verified fix`,
      );
    }
  }
  for (const a of live) {
    if (a.state === "open" && !uploaded.has(a.rule_id))
      problems.push(`#${a.number} ${a.rule_id} is still open but wasn't uploaded`);
  }
  return {
    sarif_id: posted.id,
    analysis_id: analysis?.id ?? null,
    alerts: [...uploaded].map((id) => ({
      rule_id: id,
      number: byRule.get(id)?.number ?? null,
      state: byRule.get(id)?.state ?? null,
    })),
    problems,
  };
}

/**
 * The release gate (constitution 1.5.0): no open critical or high code
 * scanning (every tool), Dependabot or secret scanning alert. Draft and triage
 * advisories count too: after a release, a finding routed to one is otherwise
 * invisible here.
 */
export function gate(gh, { ref = null } = {}) {
  const r = ref ? `&ref=${enc(ref)}` : "";
  const scanning = ["critical", "high"].map(
    (s) =>
      paged(gh, `repos/${REPO}/code-scanning/alerts?state=open&severity=${s}${r}&per_page=100`)
        .length,
  );
  const dependabot = paged(
    gh,
    `repos/${REPO}/dependabot/alerts?state=open&severity=critical,high&per_page=100`,
  ).length;
  // A secret scanning alert has no severity: every open one is a leaked secret.
  // shortcut: without `secret_type` the API returns default (provider) patterns
  // only, so generic-pattern alerts aren't counted; name them here if the
  // repository turns generic detection on.
  const secrets = paged(gh, `repos/${REPO}/secret-scanning/alerts?state=open&per_page=100`).length;
  const advisories = ["draft", "triage"].reduce(
    (n, s) => n + paged(gh, `repos/${REPO}/security-advisories?state=${s}&per_page=100`).length,
    0,
  );
  const result = {
    code_scanning: { critical: scanning[0], high: scanning[1] },
    dependabot,
    secret_scanning: secrets,
    advisories,
  };
  return { ...result, pass: scanning[0] + scanning[1] + dependabot + secrets + advisories === 0 };
}

/**
 * Dismiss `next` as `old` was: only this tool's alerts, `next` open and `old`
 * dismissed, on the same surface, so it can't silence another tool's alert.
 */
export function carryDismissal(gh, next, old) {
  const was = gh([`repos/${REPO}/code-scanning/alerts/${old}`]);
  const now = gh([`repos/${REPO}/code-scanning/alerts/${next}`]);
  if (was.tool?.name !== TOOL || now.tool?.name !== TOOL)
    throw new Error(`#${old} and #${next} must both be ${TOOL}'s`);
  if (was.state !== "dismissed") throw new Error(`#${old} isn't dismissed`);
  if (now.state !== "open") throw new Error(`#${next} isn't open`);
  const surface = (a) => a.rule.id.split("/")[0];
  if (surface(was) !== surface(now))
    throw new Error(`#${old} and #${next} are on different surfaces`);
  return gh([
    "--method",
    "PATCH",
    `repos/${REPO}/code-scanning/alerts/${next}`,
    "-f",
    "state=dismissed",
    "-f",
    `dismissed_reason=${was.dismissed_reason}`,
    "-f",
    `dismissed_comment=carried from #${old}: moved. ${was.dismissed_comment ?? ""}`.slice(0, 280),
  ]);
}

// ---- local ----

function git(args, opts = {}) {
  return execFileSync("git", args, { encoding: "utf8", ...opts }).trim();
}

/** Where a run's SARIF and record live: the git common dir, never the tree. */
export function recordPaths(mode) {
  const dir = path.join(
    git(["rev-parse", "--path-format=absolute", "--git-common-dir"]),
    "ai-security-review",
  );
  const name =
    mode === "release"
      ? "release"
      : git(["rev-parse", "--abbrev-ref", "HEAD"]).replaceAll("/", "__");
  if (name === "HEAD") throw new Error("pr mode needs a branch, not a detached HEAD");
  return { dir, record: path.join(dir, `${name}.json`), sarif: path.join(dir, `${name}.sarif`) };
}

/** The surfaces a finding can name: every entry but `not-surface`. */
export function surfaceIds(listText) {
  return new Set(
    parseList(listText)
      .map((e) => e.id)
      .filter((id) => id !== "not-surface"),
  );
}

/** The ids of the entries whose **Code** claims one of `files`, and the files none claims. */
export function touchedSurfaces(entries, files) {
  const claims = entries.map((e) => ({ id: e.id, res: (e.code?.paths ?? []).map(globToRegExp) }));
  const hit = (c, f) => c.res.some((r) => r.test(f));
  return {
    ids: claims.filter((c) => files.some((f) => hit(c, f))).map((c) => c.id),
    unclaimed: files.filter((f) => !claims.some((c) => hit(c, f))),
  };
}

/** The surface ids at `commit`, read from the commit, not the working tree. */
export function surfacesAt(commit) {
  return surfaceIds(git(["show", `${commit}:${LIST}`], { maxBuffer: 16 << 20 }));
}

function sleepSeconds(s) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, s * 1000);
}

function options(argv) {
  const opts = { _: [] };
  for (let i = 0; i < argv.length; i++) {
    const m = /^--([a-z-]+)$/.exec(argv[i]);
    if (!m) opts._.push(argv[i]);
    else if (["seed-prs", "release"].includes(m[1])) opts[m[1]] = true;
    else opts[m[1]] = argv[++i];
  }
  return opts;
}

function build(opts) {
  const mode = opts.mode;
  if (mode !== "pr" && mode !== "release") throw new Error("--mode pr|release");
  const commit = git(["rev-parse", "--verify", `${opts.commit}^{commit}`]);
  const raw = JSON.parse(fs.readFileSync(opts._[1], "utf8"));
  if (!Array.isArray(raw)) throw new Error("findings must be a JSON array");
  const surfaces = surfacesAt(commit);
  const problems = raw.flatMap((f, i) =>
    findingProblems(f, { mode, surfaces }).map((p) => `finding ${i}: ${p}`),
  );
  if (mode === "pr") {
    if (!["unchanged", "changed"].includes(opts.surface))
      problems.push("--surface unchanged|changed");
    if (!opts.base) problems.push("--base <merge base>");
    // The same rule as pr-body.yml: a changed verdict needs the list's update.
    else if (opts.surface === "changed") {
      try {
        git(["diff", "--quiet", opts.base, commit, "--", LIST]);
        problems.push(
          `--surface changed, but ${LIST} is unchanged since ${opts.base}: commit its update first`,
        );
      } catch {
        // differs: as it should
      }
    }
  }
  if (problems.length) throw new Error(problems.join("\n"));
  const knownAlerts = (opts.known ? opts.known.split(",") : []).flatMap((file) =>
    JSON.parse(fs.readFileSync(file, "utf8")),
  );
  const known = knownAlerts.map((a) => a.rule_id);
  // The carried alerts the verifier confirmed fixed: the only ones the upload may close.
  const fixed = (opts.fixed ? opts.fixed.split(",") : []).map((n) => {
    const a = knownAlerts.find((k) => k.number === Number(n) && k.state === "open");
    if (!a) throw new Error(`--fixed ${n}: not an open alert in --known`);
    return { number: a.number, rule_id: a.rule_id };
  });
  const findings = assignRuleIds(raw, known);
  for (const f of fixed) {
    if (findings.some((x) => x.rule_id === f.rule_id))
      throw new Error(`#${f.number} is both fixed and uploaded`);
  }
  const sarif = buildSarif(findings, { runId: `${mode}-${commit.slice(0, 12)}` });
  const where = recordPaths(mode);
  fs.mkdirSync(where.dir, { recursive: true });
  fs.writeFileSync(where.sarif, JSON.stringify(sarif, null, 1));
  const record = {
    mode,
    commit,
    branch: mode === "pr" ? git(["rev-parse", "--abbrev-ref", "HEAD"]) : null,
    counts: counts(findings),
    rule_ids: findings.map((f) => f.rule_id),
    fixed,
    surface: opts.surface ?? null,
    body: mode === "pr" ? bodyLines(commit, findings, opts.surface) : null,
    uploaded: null,
  };
  fs.writeFileSync(where.record, `${JSON.stringify(record, null, 1)}\n`);
  console.log(`wrote ${where.sarif} (${findings.length} findings) and ${where.record}`);
  for (const f of findings) console.log(`  ${f.severity.padEnd(8)} ${f.rule_id}`);
  if (record.body)
    console.log(`\nThe PR body's section:\n\n## Security review\n\n${record.body.join("\n")}`);
}

function uploadCommand(opts) {
  const mode = opts.pr ? "pr" : opts.release ? "release" : null;
  if (!mode) throw new Error("upload --pr <n> | --release [--ref <ref>]");
  const where = recordPaths(mode);
  const record = JSON.parse(fs.readFileSync(where.record, "utf8"));
  if (record.mode !== mode)
    throw new Error(`${where.record} is a ${record.mode} record, not ${mode}`);
  let ref;
  if (mode === "pr") {
    const pr = realGh([`repos/${REPO}/pulls/${Number(opts.pr)}`]);
    if (pr.head.sha !== record.commit) {
      throw new Error(
        `PR #${opts.pr}'s head is ${pr.head.sha.slice(0, 7)}, but ${record.commit.slice(0, 7)} was reviewed: push it, or review again`,
      );
    }
    ref = `refs/pull/${pr.number}/head`;
  } else {
    ref = opts.ref ?? "refs/heads/main";
    const branch = ref.replace(/^refs\/heads\//, "");
    const cmp = realGh([`repos/${REPO}/compare/${record.commit}...${enc(branch)}`]);
    if (!["identical", "ahead"].includes(cmp.status))
      throw new Error(`${record.commit.slice(0, 7)} isn't on ${ref}`);
  }
  const sarif = JSON.parse(fs.readFileSync(where.sarif, "utf8"));
  const out = upload(realGh, {
    sarif,
    ref,
    commit: record.commit,
    fixed: (record.fixed ?? []).map((f) => f.rule_id),
  });
  record.uploaded = {
    ref,
    at: new Date().toISOString(),
    sarif_id: out.sarif_id,
    analysis_id: out.analysis_id,
  };
  fs.writeFileSync(where.record, `${JSON.stringify(record, null, 1)}\n`);
  console.log(JSON.stringify({ ref, ...out }, null, 1));
  if (out.problems.length) process.exitCode = 1;
}

function main(argv) {
  const opts = options(argv);
  switch (opts._[0]) {
    case "touched": {
      if (!opts.base) throw new Error("touched --base <sha> [--commit <sha>]");
      const commit = opts.commit ?? "HEAD";
      // --no-renames: a moved file counts under its old name and its new one.
      const files = git(["diff", "--name-only", "--no-renames", opts.base, commit])
        .split("\n")
        .filter(Boolean);
      const entries = parseList(git(["show", `${commit}:${LIST}`], { maxBuffer: 16 << 20 }));
      console.log(JSON.stringify(touchedSurfaces(entries, files), null, 1));
      break;
    }
    case "open":
      if (!opts.ref) throw new Error("open --ref <ref>");
      console.log(
        JSON.stringify(openAlerts(realGh, opts.ref, { seedPrs: !!opts["seed-prs"] }), null, 1),
      );
      break;
    case "build":
      build(opts);
      break;
    case "upload":
      uploadCommand(opts);
      break;
    case "gate": {
      const g = gate(realGh, { ref: opts.ref ?? null });
      console.log(JSON.stringify(g, null, 1));
      console.log(g.pass ? "PASS" : "FAIL");
      if (!g.pass) process.exitCode = 1;
      break;
    }
    case "carry-dismissal":
      carryDismissal(realGh, Number(opts._[1]), Number(opts._[2]));
      console.log(`#${opts._[1]} dismissed as #${opts._[2]} was`);
      break;
    default:
      throw new Error(
        "usage: touched | open | build | upload | gate | carry-dismissal (see the header)",
      );
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
