// @vitest-environment node
// The lint config turns on the type-aware deprecated-API rule for the
// project's own TypeScript (012 FR-018, 012 research.md §7).
import { describe, expect, it } from "vitest";
import path from "node:path";
import { ESLint } from "eslint";

const repoRoot = path.resolve(import.meta.dirname, "..");

describe("012 US4 - Lint Stops New Uses of Deprecated APIs", () => {
  it.each([
    ["012 US4-1", "src/App.tsx"],
    ["012 US4-2", "src/components/lock.ts"],
  ])("%s: no-deprecated is an error with a project service for %s", async (_id, file) => {
    const config = await new ESLint({ cwd: repoRoot }).calculateConfigForFile(
      path.join(repoRoot, file),
    );
    expect(config.rules?.["@typescript-eslint/no-deprecated"]?.[0]).toBe(2);
    expect(config.languageOptions?.parserOptions?.projectService).toBeTruthy();
  });

  // A sample under scripts/ is linted through allowDefaultProject's `scripts/*.ts`.
  // Each builds a TypeScript program, several seconds under the full suite.
  const lintSample = async (code: string) => {
    const [result] = await new ESLint({ cwd: repoRoot }).lintText(code, {
      filePath: path.join(repoRoot, "scripts/__lint-sample__.ts"),
    });
    return result.messages.filter((m) => m.ruleId === "@typescript-eslint/no-deprecated");
  };
  const sample = `/** @deprecated use newer */
export function older(): number {
  return 1;
}
%s
export const n = older();
`;

  it("012 US4-1: a deprecated use is an error", { timeout: 60_000 }, async () => {
    const hits = await lintSample(sample.replace("%s", ""));
    expect(hits).toHaveLength(1);
    expect(hits[0].severity).toBe(2);
  });

  it(
    "012 US4-2: the same use with a justified disable comment passes",
    { timeout: 60_000 },
    async () => {
      const disable =
        "// eslint-disable-next-line @typescript-eslint/no-deprecated -- no replacement in WebKit";
      expect(await lintSample(sample.replace("%s", disable))).toEqual([]);
    },
  );
});
