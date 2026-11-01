// @vitest-environment node
// The lint config turns on the type-aware deprecated-API rule for the
// project's own TypeScript (012 FR-018, 012 research.md §7).
import { describe, expect, it } from "vitest";
import path from "node:path";
import { ESLint } from "eslint";

const repoRoot = path.resolve(import.meta.dirname, "..");

describe("012 US4 - Deprecated APIs in the Project's Own Code Fail Lint", () => {
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
});
