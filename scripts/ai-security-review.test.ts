// @vitest-environment node
// The AI security review's script (#21, scripts/ai-security-review.mjs): rule
// ids and fingerprints, the SARIF, and the GitHub calls against a fake `gh`.
import { describe, expect, it } from "vitest";
import fs from "node:fs";
import path from "node:path";
import { gunzipSync } from "node:zlib";
import { LIST, parseList, ROOT } from "./attack-surface.mjs";
import {
  accountFor,
  assignRuleIds,
  baseRuleId,
  BODY,
  bodyLines,
  buildSarif,
  CATEGORY,
  carryDismissal,
  findingProblems,
  fingerprint,
  gate,
  openAlerts,
  reviewed,
  surfaceIds,
  touchedSurfaces,
  TOOL,
  upload,
} from "./ai-security-review.mjs";

const finding = (over: Record<string, unknown> = {}) => ({
  surface: "import-export",
  cwe: 1236,
  path: "src-tauri/src/services/spreadsheet.rs",
  symbol: "protect_csv_cell",
  region: { startLine: 626, endLine: 626 },
  severity: "high",
  title: "Formula injection in CSV export",
  message: "TEST ONLY",
  introduced: true,
  ...over,
});

const surfaces = new Set(["import-export", "ipc-commands"]);

type Route = [RegExp, (m: RegExpExecArray, args: string[], input?: string) => unknown];

/** A fake `gh api`: the first route whose pattern matches the endpoint answers. */
function fakeGh(routes: Route[]) {
  const calls: { endpoint: string; args: string[]; input?: string }[] = [];
  const gh = (args: string[], input?: string) => {
    const endpoint = args.find((a) => a.startsWith("repos/"))!;
    calls.push({ endpoint, args, input });
    for (const [re, answer] of routes) {
      const m = re.exec(endpoint);
      if (m) {
        const out = answer(m, args, input);
        // --paginate --slurp answers an array of pages.
        return args.includes("--slurp") ? [out] : out;
      }
    }
    throw new Error(`unexpected gh api ${endpoint}`);
  };
  return { gh, calls };
}

const alert = (number: number, rule: string, state = "open", ref = "refs/heads/main") => ({
  number,
  state,
  dismissed_reason: state === "dismissed" ? "won't fix" : null,
  rule: { id: rule, security_severity_level: "high" },
  most_recent_instance: { ref, location: { path: "a.rs", start_line: 3 }, message: { text: "m" } },
});

describe("rule ids and fingerprints", () => {
  it("are the same whatever the wording or lines", () => {
    const a = finding();
    const b = finding({
      region: { startLine: 700, endLine: 702 },
      message: "other words",
      title: "Other",
    });
    expect(baseRuleId(a)).toBe(baseRuleId(b));
    expect(fingerprint(baseRuleId(a))).toBe(fingerprint(baseRuleId(b)));
  });

  it("follow the scheme, with :: written as .", () => {
    expect(baseRuleId(finding({ symbol: "Session::open" }))).toBe(
      "import-export/cwe-1236/src-tauri/src/services/spreadsheet.rs#Session.open",
    );
    // The worked example in #21's code-scanning design.
    expect(
      fingerprint(
        "spreadsheet-export/cwe-1236/src-tauri/src/services/spreadsheet.rs#protect_csv_cell",
      ),
    ).toBe("8056ade0389d1178:1");
  });

  it("keep a carried id, and add ~k only for a distinct finding that collides", () => {
    const base = baseRuleId(finding());
    const out = assignRuleIds(
      [finding({ rule_id: base }), finding(), finding(), finding({ symbol: "other" })],
      [`${base}~2`],
    );
    expect(out.map((f: { rule_id: string }) => f.rule_id)).toEqual([
      base,
      `${base}~3`,
      `${base}~4`,
      baseRuleId(finding({ symbol: "other" })),
    ]);
    expect(() => assignRuleIds([finding({ rule_id: base }), finding({ rule_id: base })])).toThrow(
      /share/,
    );
  });
});

describe("finding checks", () => {
  it("accepts a whole finding", () => {
    expect(findingProblems(finding(), { mode: "pr", surfaces })).toEqual([]);
  });

  it("refuses a pre-existing weakness in pr mode only", () => {
    expect(findingProblems(finding({ introduced: false }), { mode: "pr", surfaces })).toEqual([
      expect.stringMatching(/never goes on a pull request/),
    ]);
    expect(findingProblems(finding({ introduced: false }), { mode: "release", surfaces })).toEqual(
      [],
    );
  });

  it("refuses an unknown surface, a bad path, symbol, region or band", () => {
    const problems = findingProblems(
      finding({
        surface: "nope",
        path: "../x",
        symbol: "two words",
        region: { startLine: 5, endLine: 4 },
        severity: "severe",
        cwe: 0,
      }),
      { mode: "release", surfaces },
    );
    expect(problems).toHaveLength(6);
  });

  it("refuses a carried id whose path or symbol changed (that's a move)", () => {
    const old = baseRuleId(finding());
    expect(
      findingProblems(finding({ rule_id: old, path: "src-tauri/src/other.rs" }), {
        mode: "release",
        surfaces,
      }),
    ).toEqual([expect.stringMatching(/rule_id must start with/)]);
  });

  it("takes the surface ids from the list, less not-surface", () => {
    const ids = surfaceIds(fs.readFileSync(path.join(ROOT, LIST), "utf8"));
    expect(ids.has("ipc-commands")).toBe(true);
    expect(ids.has("not-surface")).toBe(false);
  });
});

describe("touched surfaces", () => {
  it("maps changed files to the entries that claim them", () => {
    const entries = parseList(
      "## a-one: A\n\n- **Code:** `src-tauri/src/a.rs`; `src-tauri/src/a/**`\n\n## b-two: B\n\n- **Code:** `scripts/*.mjs` (only these)\n",
    );
    expect(
      touchedSurfaces(entries, ["src-tauri/src/a/deep/x.rs", "README.md", "scripts/x.mjs"]),
    ).toEqual({
      ids: ["a-one", "b-two"],
      unclaimed: ["README.md"],
    });
    expect(touchedSurfaces(entries, ["scripts/sub/x.mjs"]).ids).toEqual([]);
  });
});

describe("SARIF", () => {
  const findings = assignRuleIds([
    finding(),
    finding({ symbol: "a", severity: "critical" }),
    finding({ symbol: "b", severity: "medium" }),
    finding({ symbol: "c", severity: "low" }),
  ]);
  const sarif = buildSarif(findings, { runId: "pr-abc" });
  const run = sarif.runs[0];

  it("has what code scanning requires, one rule per finding", () => {
    expect(sarif.version).toBe("2.1.0");
    expect(run.tool.driver.name).toBe(TOOL);
    expect(run.automationDetails.id).toBe(`${CATEGORY}/pr-abc`);
    expect(run.tool.driver.rules).toHaveLength(4);
    run.results.forEach((r, i) => {
      expect(r.ruleId).toBe(findings[i].rule_id);
      expect(run.tool.driver.rules[r.ruleIndex].id).toBe(r.ruleId);
      expect(r.message.text).toBeTruthy();
      expect(r.locations[0].physicalLocation.artifactLocation.uri).toBe(findings[i].path);
      expect(r.locations[0].physicalLocation.region.startLine).toBeGreaterThan(0);
      expect(r.partialFingerprints.primaryLocationLineHash).toBe(fingerprint(r.ruleId));
    });
  });

  it("maps each band to its fixed score, level and tags", () => {
    const props = run.tool.driver.rules.map((r) => [
      r.properties["security-severity"],
      r.defaultConfiguration.level,
      r.properties["problem.severity"],
    ]);
    expect(props).toEqual([
      ["8.0", "error", "error"],
      ["9.5", "error", "error"],
      ["5.5", "warning", "warning"],
      ["2.0", "note", "recommendation"],
    ]);
    expect(run.tool.driver.rules[0].properties.tags).toEqual([
      "security",
      "external/cwe/cwe-1236",
      "surface/import-export",
    ]);
  });

  it("gives the PR body lines the checks expect, and no finding's text", () => {
    const lines = bodyLines("a".repeat(40), findings, "unchanged").join("\n");
    expect(lines).toMatch(BODY.commit);
    expect(lines).toMatch(BODY.counts);
    expect(lines).toMatch(BODY.surface);
    expect(lines).toContain("4 (critical 1, high 1, medium 1, low 1)");
    expect(lines).not.toContain("protect_csv_cell");
  });
});

describe("upload", () => {
  const sha = "1".repeat(40);
  const sarif = buildSarif(assignRuleIds([finding()]), { runId: "pr-1" });
  const rule = sarif.runs[0].results[0].ruleId;
  // `open` answers the first open-alerts read (before the upload), then `after`.
  const routes = (status: string[], open: unknown[], count = 1, after = open): Route[] => {
    let reads = 0;
    return [
      [/code-scanning\/sarifs$/, () => ({ id: "S1" })],
      [
        /code-scanning\/sarifs\/S1$/,
        () => ({ processing_status: status.shift(), errors: ["bad"] }),
      ],
      [
        /analyses\?sarif_id=S1$/,
        () => [{ id: 9, results_count: count, tool: { name: TOOL }, category: CATEGORY }],
      ],
      [/alerts\?.*state=open/, () => (reads++ === 0 ? open : after)],
      [/alerts\?.*state=dismissed/, () => []],
    ];
  };

  it("sends gzipped SARIF to the PR ref, waits, and maps rules to alerts", () => {
    const { gh, calls } = fakeGh(routes(["pending", "complete"], [alert(7, rule)]));
    const out = upload(gh, {
      mode: "pr",
      sarif,
      ref: "refs/pull/5/head",
      commit: sha,
      sleep: () => {},
    });
    expect(out.problems).toEqual([]);
    expect(out.alerts).toEqual([{ rule_id: rule, number: 7, state: "open" }]);
    const body = JSON.parse(calls.find((c) => c.input)!.input!);
    expect(body).toMatchObject({
      commit_sha: sha,
      ref: "refs/pull/5/head",
      tool_name: TOOL,
      validate: true,
    });
    expect(JSON.parse(gunzipSync(Buffer.from(body.sarif, "base64")).toString())).toEqual(sarif);
    expect(calls.filter((c) => c.endpoint.endsWith("sarifs/S1"))).toHaveLength(2);
  });

  it("reports an alert left open, a missing alert and a wrong count", () => {
    const { gh } = fakeGh(routes(["complete"], [], 2, [alert(3, "other/cwe-1/x#y")]));
    const out = upload(gh, {
      mode: "release",
      sarif,
      ref: "refs/heads/main",
      commit: sha,
      sleep: () => {},
    });
    expect(out.problems).toEqual([
      "the analysis has 2 results, not 1",
      `no open or dismissed alert for ${rule}`,
      "#3 other/cwe-1/x#y is still open but wasn't uploaded",
    ]);
  });

  it("reports an open alert the upload closed unless it is a verified fix", () => {
    // As the live tests showed: a left-out alert is already fixed once processing completes.
    const before = [alert(7, rule), alert(3, "other/cwe-1/x#y")];
    const after = [alert(7, rule)];
    const run = (fixed: string[]) =>
      upload(fakeGh(routes(["complete"], before, 1, after)).gh, {
        mode: "release",
        sarif,
        ref: "refs/heads/main",
        commit: sha,
        fixed,
        sleep: () => {},
      }).problems;
    expect(run([])).toEqual([
      "#3 other/cwe-1/x#y was open and this upload closed it, but it isn't a verified fix",
    ]);
    expect(run(["other/cwe-1/x#y"])).toEqual([]);
  });

  it("fails on a failed upload, and never uploads to a tag", () => {
    const { gh } = fakeGh(routes(["failed"], []));
    expect(() =>
      upload(gh, { mode: "release", sarif, ref: "refs/heads/main", commit: sha, sleep: () => {} }),
    ).toThrow(/failed.*bad/);
    // A release's whole finding set never goes to a pull request's public annotations.
    expect(() =>
      upload(gh, { mode: "release", sarif, ref: "refs/pull/5/head", commit: sha }),
    ).toThrow(/release upload can't go to refs\/pull/);
    expect(() => upload(gh, { mode: "pr", sarif, ref: "refs/heads/main", commit: sha })).toThrow(
      /pr upload can't go to refs\/heads/,
    );
    expect(() =>
      upload(gh, { mode: "release", sarif, ref: "refs/tags/v0.1.0", commit: sha }),
    ).toThrow(/can't go to refs\/tags/);
  });
});

describe("carry-forward", () => {
  it("lists open and dismissed alerts, and seeds open ones from PRs merged since the last analysis", () => {
    const { gh } = fakeGh([
      [/alerts\?.*ref=refs%2Fheads%2Fmain&state=open/, () => [alert(1, "a/cwe-1/x#a")]],
      [
        /alerts\?.*ref=refs%2Fheads%2Fmain&state=dismissed/,
        () => [alert(2, "a/cwe-1/x#b", "dismissed")],
      ],
      [
        /alerts\?.*ref=refs%2Fpull%2F10%2Fhead&state=open/,
        () => [alert(3, "a/cwe-1/x#c", "open", "refs/pull/10/head")],
      ],
      [
        /alerts\?.*ref=refs%2Fpull%2F12%2Fhead&state=open/,
        () => [alert(1, "a/cwe-1/x#a", "open", "refs/pull/12/head")],
      ],
      [
        /analyses\?/,
        () => [
          { ref: "refs/heads/main", created_at: "2026-10-01T00:00:00Z" },
          { ref: "refs/pull/9/head" },
          { ref: "refs/pull/10/head" },
          { ref: "refs/pull/11/head" },
          { ref: "refs/pull/12/head" },
        ],
      ],
      [/pulls\/9$/, () => ({ merged_at: "2026-09-01T00:00:00Z" })],
      [/pulls\/10$/, () => ({ merged_at: "2026-10-05T00:00:00Z" })],
      [/pulls\/11$/, () => ({ merged_at: null })],
      [/pulls\/12$/, () => ({ merged_at: "2026-10-06T00:00:00Z" })],
    ]);
    const out = openAlerts(gh, "refs/heads/main", { seedPrs: true });
    expect(out.map((a) => [a.number, a.state, a.seeded_from])).toEqual([
      [1, "open", undefined],
      [2, "dismissed", undefined],
      [3, "open", 10],
    ]);
  });

  const moved = (old: object = {}, next: object = {}) =>
    fakeGh([
      [
        /alerts\/4$/,
        () => ({
          tool: { name: TOOL },
          rule: { id: "paths/cwe-22/a.rs#f" },
          state: "dismissed",
          dismissed_reason: "false positive",
          dismissed_comment: "why",
          ...old,
        }),
      ],
      [
        /alerts\/8$/,
        () => ({
          tool: { name: TOOL },
          rule: { id: "paths/cwe-22/b.rs#f" },
          state: "open",
          ...next,
        }),
      ],
    ]);

  it("re-applies a dismissal to a moved finding's new alert", () => {
    const { gh, calls } = moved();
    carryDismissal(gh, 8, 4);
    expect(calls[2].args).toEqual(
      expect.arrayContaining([
        "PATCH",
        "dismissed_reason=false positive",
        "dismissed_comment=carried from #4: moved. why",
      ]),
    );
  });

  it("dismisses only this tool's open alert, from a dismissed one on the same surface", () => {
    const refused = (old: object, next: object) => {
      const { gh, calls } = moved(old, next);
      expect(() => carryDismissal(gh, 8, 4)).toThrow();
      expect(calls.some((c) => c.args.includes("PATCH"))).toBe(false);
    };
    refused({}, { tool: { name: "CodeQL" } });
    refused({ tool: { name: "CodeQL" } }, {});
    refused({ state: "open" }, {});
    refused({}, { state: "dismissed" });
    refused({}, { rule: { id: "logging/cwe-532/b.rs#f" } });
  });
});

describe("accounting for carried alerts", () => {
  const carried = [
    alert(1, "a/cwe-1/x#a"),
    { ...alert(2, "a/cwe-1/x#b"), seeded_from: 10 },
    alert(3, "a/cwe-1/x#c", "dismissed"),
  ].map((a) => ({ number: a.number, rule_id: a.rule.id, state: a.state }));
  const upload = (...ids: string[]) => ids.map((rule_id) => ({ rule_id }));

  it("needs every open one, seeded ones too, uploaded or named fixed", () => {
    expect(accountFor(carried, upload("a/cwe-1/x#a"), []).problems).toEqual([
      "carried #2 a/cwe-1/x#b is neither uploaded nor in --fixed",
    ]);
    expect(accountFor(carried, upload("a/cwe-1/x#a"), [2])).toEqual({
      fixed: [{ number: 2, rule_id: "a/cwe-1/x#b" }],
      problems: [],
    });
  });

  it("refuses a fix that isn't a carried open alert, or is also uploaded", () => {
    const all = upload("a/cwe-1/x#a", "a/cwe-1/x#b");
    expect(accountFor(carried, all, [3, 9, 1]).problems).toEqual([
      "--fixed 3: not an open alert in --carry",
      "--fixed 9: not an open alert in --carry",
      "#1 is both fixed and uploaded",
    ]);
  });
});

describe("release gate", () => {
  const gateGh = (n: {
    critical: number;
    high: number;
    dependabot: number;
    secrets: number;
    advisories: number;
  }) =>
    fakeGh([
      [/code-scanning\/alerts\?state=open&severity=critical/, () => Array(n.critical).fill({})],
      [/code-scanning\/alerts\?state=open&severity=high/, () => Array(n.high).fill({})],
      [/dependabot\/alerts/, () => Array(n.dependabot).fill({})],
      [/secret-scanning\/alerts/, () => Array(n.secrets).fill({})],
      [/security-advisories\?state=draft/, () => Array(n.advisories).fill({})],
      [/security-advisories\?state=triage/, () => []],
    ]);

  it("passes with nothing open, across every tool", () => {
    const { gh, calls } = gateGh({
      critical: 0,
      high: 0,
      dependabot: 0,
      secrets: 0,
      advisories: 0,
    });
    expect(gate(gh).pass).toBe(true);
    expect(calls.some((c) => c.endpoint.includes("tool_name"))).toBe(false);
  });

  it("fails on any one source", () => {
    for (const key of ["critical", "high", "dependabot", "secrets", "advisories"]) {
      const n = { critical: 0, high: 0, dependabot: 0, secrets: 0, advisories: 0, [key]: 1 };
      expect(gate(gateGh(n).gh).pass).toBe(false);
    }
  });
});

describe("the release workflow's review check", () => {
  const sha = "a".repeat(40);
  const other = "b".repeat(40);
  const analysis = (over: Record<string, unknown> = {}) => ({
    id: 9,
    commit_sha: sha,
    category: CATEGORY,
    ref: "refs/heads/main",
    created_at: "2026-11-08T00:00:00Z",
    ...over,
  });
  const reviewGh = (analyses: unknown[], highs = 0, dependabot = 0, compare = "identical") =>
    fakeGh([
      [/code-scanning\/analyses/, () => analyses],
      [/compare\//, () => ({ status: compare })],
      [/severity=critical&ref=refs%2Fheads%2Fmain/, () => []],
      [/severity=high&ref=refs%2Fheads%2Fmain/, () => Array(highs).fill({})],
      [/dependabot\/alerts\?state=open&severity=critical,high/, () => Array(dependabot).fill({})],
    ]);

  it("passes on the tagged commit's release review with nothing critical or high open", () => {
    const { gh, calls } = reviewGh([
      analysis({ commit_sha: other, created_at: "2026-11-07T00:00:00Z" }),
      analysis(),
    ]);
    const r = reviewed(gh, sha);
    expect(r.pass).toBe(true);
    expect(r.analysis).toMatchObject({ id: 9, ref: "refs/heads/main" });
    expect(calls[0].endpoint).toContain(`tool_name=${encodeURIComponent(TOOL)}`);
  });

  it("fails without a review of that commit on main, such as a pull request's or another branch's", () => {
    expect(reviewed(reviewGh([analysis({ commit_sha: other })]).gh, sha).pass).toBe(false);
    expect(reviewed(reviewGh([analysis({ ref: "refs/pull/7/head" })]).gh, sha).pass).toBe(false);
    expect(reviewed(reviewGh([analysis({ ref: "refs/heads/release/x" })]).gh, sha).pass).toBe(
      false,
    );
    expect(reviewed(reviewGh([analysis({ category: "other" })]).gh, sha).pass).toBe(false);
  });

  it("fails when a later review on the branch moved its alerts on from that commit", () => {
    const later = analysis({ id: 10, commit_sha: other, created_at: "2026-11-09T00:00:00Z" });
    const r = reviewed(reviewGh([later, analysis()]).gh, sha);
    expect(r).toMatchObject({ pass: false, newer: other });
    // A later review on another branch doesn't count.
    const elsewhere = { ...later, ref: "refs/heads/release/x" };
    expect(reviewed(reviewGh([elsewhere, analysis()]).gh, sha).pass).toBe(true);
  });

  it("fails once the commit is no longer its branch's head, so the branch's alerts aren't its own", () => {
    const r = reviewed(reviewGh([analysis()], 0, 0, "ahead").gh, sha);
    expect(r).toMatchObject({ pass: false, behind: "refs/heads/main" });
  });

  it("fails on an open high alert, or a Dependabot one, and refuses a short sha", () => {
    expect(reviewed(reviewGh([analysis()], 1).gh, sha).pass).toBe(false);
    expect(reviewed(reviewGh([analysis()], 0, 1).gh, sha).pass).toBe(false);
    expect(() => reviewed(reviewGh([]).gh, "abc")).toThrow(/full commit/);
  });
});
