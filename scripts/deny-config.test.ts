// @vitest-environment node
// Guards src-tauri/deny.toml's advisory policy (012 FR-014, 012 research.md §6):
// a yanked crate always fails, and `ignore` holds advisory IDs only, so no
// `crate@version` entry can except a yanked one.
import { describe, expect, it } from "vitest";
import fs from "node:fs";
import path from "node:path";

const DENY_TOML = path.resolve(import.meta.dirname, "../src-tauri/deny.toml");

/** The `[advisories]` table: from its header to the next `[` header, comments stripped. */
function advisoriesTable(text: string): string {
  const m = /^\[advisories\]\s*$([\s\S]*?)(?=^\[|(?![\s\S]))/m.exec(text);
  if (!m) throw new Error("no [advisories] table");
  return m[1]
    .split("\n")
    .map((l) => l.replace(/#.*$/, ""))
    .join("\n");
}

function yankedDenied(text: string): boolean {
  return /^\s*yanked\s*=\s*"deny"\s*$/m.test(advisoriesTable(text));
}

/** Entries of `ignore = [ … ]` that are not a plain advisory ID. */
function badIgnoreEntries(text: string): string[] {
  const m = /^\s*ignore\s*=\s*\[([\s\S]*?)^\s*\]/m.exec(advisoriesTable(text));
  if (!m) return [];
  const id = /^(RUSTSEC|GHSA|CVE)-/;
  const bad: string[] = [];
  for (const [, table] of m[1].matchAll(/\{([^}]*)\}/g)) {
    const idValue = /\bid\s*=\s*"([^"]*)"/.exec(table)?.[1];
    if (!idValue || !id.test(idValue) || /\bcrate\s*=/.test(table)) bad.push(`{${table}}`);
  }
  for (const [, s] of m[1].replace(/\{[^}]*\}/g, "").matchAll(/"([^"]*)"/g)) {
    if (!id.test(s)) bad.push(s);
  }
  return bad;
}

/** Lines of the file outside comments. */
function code(text: string): string {
  return text
    .split("\n")
    .map((l) => l.replace(/#.*$/, ""))
    .join("\n");
}

/** `exclude`/`exclude-dev` anywhere would hide a crate from the check. */
function hasExclude(text: string): boolean {
  return /^\s*(exclude|exclude-dev)\s*=/m.test(code(text));
}

/** `[graph]` keeps `all-features = true`, so feature-gated crates are checked too. */
function graphAllFeatures(text: string): boolean {
  const m = /^\[graph\]\s*$([\s\S]*?)(?=^\[|(?![\s\S]))/m.exec(text);
  return !!m && /^\s*all-features\s*=\s*true\s*$/m.test(code(m[1]));
}

const real = fs.readFileSync(DENY_TOML, "utf8");

const YANKED_MSG = "a yanked crate is updated, never excepted (012 FR-014)";

describe("012 US3 - A Yanked Rust Crate Always Fails the Audit", () => {
  it("012 US3-1: the advisories table sets yanked = deny", () => {
    expect(yankedDenied(real), YANKED_MSG).toBe(true);
  });

  it("012 US3-1: the guard fails when the yanked line is removed", () => {
    const without = real.replace(/^yanked\s*=.*$/m, "");
    expect(without).not.toBe(real);
    expect(yankedDenied(without)).toBe(false);
  });

  it("012 US3-2: ignore holds only advisory IDs", () => {
    expect(badIgnoreEntries(real), YANKED_MSG).toEqual([]);
  });

  it("012 US3-2: the guard rejects a crate spec, inline or bare", () => {
    const wrap = (entry: string) => `[advisories]\nignore = [\n    ${entry},\n]\n`;
    expect(badIgnoreEntries(wrap('{ crate = "foo@1.0.0", reason = "x" }'))).toHaveLength(1);
    expect(badIgnoreEntries(wrap('"foo@1.0.0"'))).toEqual(["foo@1.0.0"]);
    expect(badIgnoreEntries(wrap('{ id = "RUSTSEC-2024-0001", reason = "x" }'))).toEqual([]);
    expect(badIgnoreEntries(wrap('"RUSTSEC-2024-0001"'))).toEqual([]);
    expect(
      badIgnoreEntries(wrap('{ id = "RUSTSEC-2024-0001", crate = "foo@1.0.0", reason = "x" }')),
    ).toHaveLength(1);
  });

  it("012 US3-1: no exclude or exclude-dev hides a crate, and [graph] keeps all-features", () => {
    expect(hasExclude(real), YANKED_MSG).toBe(false);
    expect(graphAllFeatures(real), YANKED_MSG).toBe(true);
  });

  it("012 US3-1: the guard fails on exclude, exclude-dev or a dropped all-features", () => {
    expect(hasExclude(`${real}\n[bans]\nexclude = ["foo"]\n`)).toBe(true);
    expect(hasExclude(real.replace("[graph]", "[graph]\nexclude-dev = true"))).toBe(true);
    expect(hasExclude("# exclude = []\n")).toBe(false);
    expect(graphAllFeatures(real.replace(/^all-features\s*=.*$/m, ""))).toBe(false);
    expect(graphAllFeatures(real.replace("all-features = true", "all-features = false"))).toBe(
      false,
    );
  });
});
