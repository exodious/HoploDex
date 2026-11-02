// @vitest-environment node
// The PR description check (.github/workflows/pr-body.yml): its step's script,
// run with bash the way Actions runs it, against a fake `gh` that lists the
// pull request's files. The security review lines are #21's.
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { ROOT } from "./attack-surface.mjs";

const HEAD = "a".repeat(40);
let script: string;
let bin: string;

beforeAll(() => {
  const yaml = fs
    .readFileSync(path.join(ROOT, ".github/workflows/pr-body.yml"), "utf8")
    .split("\n");
  const start = yaml.findIndex((l) => /^\s+run: \|$/.test(l));
  const indent = /^\s*/.exec(yaml[start + 1])![0].length;
  const body: string[] = [];
  for (const l of yaml.slice(start + 1)) {
    if (l.trim() !== "" && /^\s*/.exec(l)![0].length < indent) break;
    body.push(l.slice(indent));
  }
  script = body.join("\n");
  bin = fs.mkdtempSync(path.join(os.tmpdir(), "pr-body-"));
  fs.writeFileSync(path.join(bin, "gh"), '#!/bin/sh\nprintf "%s\\n" "$FAKE_CHANGED"\n', {
    mode: 0o755,
  });
});

afterAll(() => fs.rmSync(bin, { recursive: true, force: true }));

function check(
  text: string,
  { changed = ["scripts/x.mjs"], author = "exodious", head = HEAD } = {},
) {
  const r = spawnSync("bash", ["--noprofile", "--norc", "-eo", "pipefail", "-c", script], {
    encoding: "utf8",
    env: {
      PATH: `${bin}:${process.env.PATH}`,
      FAKE_CHANGED: changed.join("\n"),
      REPO: "exodious/HoploDex",
      NUMBER: "1",
      AUTHOR: author,
      HEAD_SHA: head,
      BODY: text,
    },
  });
  const notes = (r.stdout.match(/^::(error|warning|notice)::.*$/gm) ?? []).map((l) =>
    l.replace(/::([a-z]+)::.*/, "$1"),
  );
  return { status: r.status, notes };
}

const lines = {
  commit: `- Reviewed commit: ${HEAD}`,
  counts: "- Findings uploaded to code scanning: 0 (critical 0, high 0, medium 0, low 0)",
  surface: "- Attack surface: unchanged",
};
const description = (over: Partial<typeof lines> = {}, extra = "") =>
  `## Summary\r\n\r\nx\r\n\r\n## Security review\r\n\r\n${Object.values({ ...lines, ...over })
    .filter(Boolean)
    .join("\r\n")}\r\n${extra}`;

describe("PR description check", () => {
  it("passes a description with the review's lines", () => {
    expect(check(description())).toEqual({ status: 0, notes: [] });
  });

  it("exempts Dependabot", () => {
    expect(check("", { author: "dependabot[bot]" })).toEqual({ status: 0, notes: ["notice"] });
  });

  it("fails without the section, or without any one of its lines", () => {
    expect(check("## Summary\n\nx").status).toBe(1);
    for (const key of ["commit", "counts", "surface"] as const) {
      expect(check(description({ [key]: "" })).status, key).toBe(1);
    }
    // In another section, the lines don't count.
    expect(check(`${Object.values(lines).join("\n")}\n\n## Security review\n\nnone`).status).toBe(
      1,
    );
  });

  it("fails a changed surface unless docs/attack-surface.md changes too", () => {
    const changed = description({ surface: "- Attack surface: changed" });
    expect(check(changed).status).toBe(1);
    expect(
      check(changed, { changed: ["src-tauri/src/x.rs", "docs/attack-surface.md"] }).status,
    ).toBe(0);
  });

  it("warns, but passes, when the review isn't of the head", () => {
    expect(check(description(), { head: "b".repeat(40) })).toEqual({
      status: 0,
      notes: ["warning"],
    });
  });

  it("still needs screenshots for a change under src/", () => {
    expect(check(description(), { changed: ["src/App.tsx"] }).status).toBe(1);
    expect(
      check(description({}, "\r\n## Screenshots\r\n\r\nbefore/after"), { changed: ["src/App.tsx"] })
        .status,
    ).toBe(0);
  });
});
