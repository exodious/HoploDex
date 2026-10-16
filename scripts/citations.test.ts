// The spec citation check (#121, scripts/citations.mjs): its rules on small
// texts against the real specs, then the whole tree.
import { beforeAll, describe, expect, it } from "vitest";
import { check, checkE2eDescribes, checkText, loadSpecs } from "./citations.mjs";

let specs: ReturnType<typeof loadSpecs>;
beforeAll(() => {
  specs = loadSpecs();
});

function problems(text: string): string[] {
  return checkText(text, specs).map((p: { token: string }) => p.token);
}

describe("spec citations", () => {
  it("accepts a citation that names its spec", () => {
    expect(problems("// 003 FR-040, 006 SC-002, 001 US2, 007 US3-2, 008 T050")).toEqual([]);
    expect(
      problems("// 003 US3-4a; 007 research.md §5; 006 contracts/ui-accessories.md §3"),
    ).toEqual([]);
  });

  it("refuses a bare FR, SC, story, scenario or task ID", () => {
    expect(problems("// FR-040 and SC-002")).toEqual(["FR-040", "SC-002"]);
    expect(problems("// (US2-3; T050)")).toEqual(["US2-3", "T050"]);
  });

  it("lets one spec number cover a list, wrapped or not", () => {
    expect(problems("// (006 FR-001, FR-002 and SC-003)")).toEqual([]);
    expect(problems("// 003 research.md §1, §11 and 008 T005-T008")).toEqual([]);
    expect(problems("/// (006 FR-001,\n/// FR-002)")).toEqual([]);
    expect(problems("// (006 FR-006; FR-004)")).toEqual(["FR-004"]);
  });

  it("refuses a bare document or section, but not a path into a spec folder", () => {
    expect(problems("// research.md §5")).toEqual(["research.md", "§5"]);
    expect(problems("// (§12)")).toEqual(["§12"]);
    expect(problems("// specs/007-document-preview/research.md §4 and §23")).toEqual([]);
    expect(problems("# specs/007-document-preview/\n# research.md §23")).toEqual([]);
    expect(problems("// W3C WebDriver §13.2.2")).toEqual([]);
  });

  it("treats a contract's name as a document, with or without its folder", () => {
    expect(problems('// spreadsheet-format.md "Origin values"')).toEqual(["spreadsheet-format.md"]);
    expect(problems("/// (contracts/\n/// tauri-commands.md)")).toEqual([
      "contracts/tauri-commands.md",
    ]);
    expect(problems("// 002 spreadsheet-format.md, 003 contracts/ui-databases.md §0")).toEqual([]);
    expect(problems("// 002 FR-010/research.md §8")).toEqual([]);
  });

  it('refuses "User Story" and "Scenario" wording, qualified or not', () => {
    expect(problems("// User Story 2, Scenario 3")).toEqual(["User Story 2", "Scenario 3"]);
    expect(problems("// 001 US1 Scenario 9")).toEqual(["Scenario 9"]);
  });

  it("refuses a spec's section named without its spec", () => {
    expect(problems("// (spec Edge Cases)")).toEqual(["spec Edge Cases"]);
    expect(problems("// (004 spec.md Edge Cases)")).toEqual([]);
  });

  it("refuses what the named spec doesn't have", () => {
    const text = "// 001 FR-999, 002 US9, 003 US1-99, 004 T999, 005 research.md §99, 006 nope.md";
    const found = checkText(text, specs).map((p: { message: string }) => p.message);
    expect(found).toEqual([
      expect.stringContaining("no such requirement"),
      expect.stringContaining("no User Story 9"),
      expect.stringContaining("no scenario 99"),
      expect.stringContaining("no such task"),
      expect.stringContaining("no section §99"),
    ]);
  });

  it("checks that an E2E describe() quotes its story's heading", () => {
    const ok =
      'describe("006 US2 - Mount an Item on a Firearm (was us12-accessories-mounting)", () => {';
    expect(checkE2eDescribes(`${ok}\ndescribe("Issue #44 - Enter", () => {`, specs)).toEqual([]);
    const messages = (text: string) =>
      checkE2eDescribes(text, specs).map((p: { message: string }) => p.message);
    expect(messages('describe("006 US2 - Mounting", () => {')).toContainEqual(
      expect.stringContaining(`heading is "Mount an Item on a Firearm"`),
    );
    expect(messages('describe("User Story 2 - Mounting", () => {')).toContainEqual(
      expect.stringContaining("isn't"),
    );
    expect(messages('describe("Issue #44 - Enter", () => {')).toEqual([
      "no describe() names a story",
    ]);
  });

  it("finds nothing wrong in the code, tests and tooling", () => {
    expect(check()).toEqual([]);
  });
});
