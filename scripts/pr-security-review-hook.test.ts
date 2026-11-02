// @vitest-environment node
// The Claude hook that holds `gh pr create` until the branch's HEAD has an AI
// security review and the PR body carries its lines (#21,
// .claude/hooks/pr-security-review.sh), run with bash on a scratch repository.
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { ROOT } from "./attack-surface.mjs";

const HOOK = path.join(ROOT, ".claude/hooks/pr-security-review.sh");
let repo: string;
let head: string;
let recordFile: string;

const git = (...args: string[]) =>
  execFileSync("git", ["-C", repo, ...args], { encoding: "utf8" }).trim();

const lines = () => [
  `- Reviewed commit: ${head}`,
  "- Findings uploaded to code scanning: 0 (critical 0, high 0, medium 0, low 0)",
  "- Attack surface: unchanged",
];

function record(over: Record<string, unknown> = {}) {
  fs.mkdirSync(path.dirname(recordFile), { recursive: true });
  fs.writeFileSync(
    recordFile,
    JSON.stringify({ mode: "pr", commit: head, body: lines(), ...over }),
  );
}

/** The hook's decision for `command`: "allow" when it says nothing. */
function run(command: string): { decision: string; reason: string } {
  const out = execFileSync("bash", [HOOK], {
    input: JSON.stringify({ tool_input: { command }, cwd: repo }),
    encoding: "utf8",
  });
  if (out.trim() === "") return { decision: "allow", reason: "" };
  const o = JSON.parse(out).hookSpecificOutput;
  return { decision: o.permissionDecision, reason: o.permissionDecisionReason };
}

const body = (section: string[]) =>
  `## Summary\n\nWhat changed.\n\n## Security review\n\n${section.join("\n")}\n\n## Tests\n\nnpm test`;
const create = (text: string) => `gh pr create --title t --body "$(cat <<'EOF'\n${text}\nEOF\n)"`;

beforeAll(() => {
  repo = fs.mkdtempSync(path.join(os.tmpdir(), "hook-"));
  execFileSync("git", ["init", "-q", "-b", "main", repo]);
  git(
    "-c",
    "user.name=t",
    "-c",
    "user.email=t@example.com",
    "commit",
    "-q",
    "--allow-empty",
    "-m",
    "one",
  );
  git("checkout", "-q", "-b", "fm/some-work");
  head = git("rev-parse", "HEAD");
  recordFile = path.join(repo, ".git/ai-security-review/fm__some-work.json");
});

afterAll(() => fs.rmSync(repo, { recursive: true, force: true }));

// Every test spawns bash (and git, jq) several times: about a second each on Windows.
describe("pr-security-review hook", { timeout: 30_000 }, () => {
  it("ignores other commands, and the words outside command position", () => {
    expect(run("git status").decision).toBe("allow");
    expect(run('git commit -m "then gh pr create"').decision).toBe("allow");
    expect(run("gh pr create --help").decision).toBe("allow");
  });

  it("refuses a PR with no review recorded, naming the skill", () => {
    fs.rmSync(recordFile, { force: true });
    const r = run(create(body(lines())));
    expect(r.decision).toBe("deny");
    expect(r.reason).toContain("/ai-security-review pr");
  });

  it("refuses a record that isn't a pull request review", () => {
    record({ mode: "release" });
    expect(run(create(body(lines()))).reason).toMatch(/isn't a pull request review record/);
  });

  it("refuses a body without the section, or without one of its lines", () => {
    record();
    expect(run(create("## Summary\n\nx")).reason).toMatch(/no "## Security review" section/);
    const r = run(create(body(lines().slice(0, 2))));
    expect(r.decision).toBe("deny");
    expect(r.reason).toContain("- Attack surface: unchanged");
    // The lines must be in the section itself, not elsewhere in the body.
    expect(run(create(`${lines().join("\n")}\n\n## Security review\n\nnone`)).decision).toBe(
      "deny",
    );
  });

  it("allows the lines from a heredoc, --body-file, or gh-axi, with or without list markers", () => {
    record();
    expect(run(create(body(lines())))).toEqual({ decision: "allow", reason: "" });
    expect(run(create(body(lines().map((l) => `  ${l.slice(2)}  `)))).decision).toBe("allow");
    const file = path.join(repo, "body.md");
    fs.writeFileSync(file, body(lines()).replaceAll("\n", "\r\n"));
    expect(run(`gh-axi pr create --title t --body-file '${file}'`).decision).toBe("allow");
  });

  it("asks when HEAD moved since the review", () => {
    record();
    git(
      "-c",
      "user.name=t",
      "-c",
      "user.email=t@example.com",
      "commit",
      "-q",
      "--allow-empty",
      "-m",
      "two",
    );
    try {
      const r = run(create(body(lines())));
      expect(r.decision).toBe("ask");
      expect(r.reason).toContain(head.slice(0, 7));
    } finally {
      git("reset", "-q", "--hard", head);
    }
  });
});
