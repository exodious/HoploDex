// @vitest-environment node
// The npm deprecation audit (012 FR-001 to FR-013): fixture lockfiles, no files
// and no child process.
import { describe, expect, it } from "vitest";
import { checkDeprecated, satisfiesRange } from "./check-npm-deprecated.mjs";

const TODAY = "2026-11-01";

const lock = (packages: Record<string, object>) => ({
  packages: {
    "": {
      name: "hoplodex",
      dependencies: { react: "^18.0.0", shipdep: "^1.0.0" },
      devDependencies: { devdep: "^1.0.0", a: "^1.0.0" },
    },
    ...packages,
  },
});

const exception = (over: Record<string, unknown> = {}) => ({
  package: "glob",
  range: ">=10 <11",
  scope: "dev",
  reason: "no replacement yet",
  via: "@wdio/config",
  successor: "glob 13",
  review: "2027-01-31",
  ...over,
});

const file = (...exceptions: unknown[]) => ({ "//": "note", exceptions });

const GLOB = "node_modules/glob";
const devGlob = (location = GLOB) => ({
  [location]: { version: "10.5.0", dev: true, deprecated: "Glob 10 is old" },
});
const shippedGlob = { [GLOB]: { version: "10.5.0", deprecated: "Glob 10 is old" } };

describe("012 US1 - a deprecated package with no decision fails", () => {
  it("012 US1-1: a shipped transitive package without an exception fails", () => {
    const { findings, problems } = checkDeprecated(lock(shippedGlob), file(), TODAY);
    expect(findings).toHaveLength(1);
    expect(findings[0]).toMatchObject({ scope: "shipped", direct: false, outcome: "fails" });
    expect(problems).toHaveLength(1);
    expect(problems[0]).toContain("glob@10.5.0");
    expect(problems[0]).toContain(GLOB);
    expect(problems[0]).toContain("ships");
  });

  it("012 US1-2: a development-only package without an exception fails", () => {
    const { problems } = checkDeprecated(lock(devGlob()), file(), TODAY);
    expect(problems).toHaveLength(1);
    expect(problems[0]).toContain("development-only");
  });

  it("012 US1-3: a matching exception makes the finding excepted", () => {
    const { findings, problems } = checkDeprecated(lock(devGlob()), file(exception()), TODAY);
    expect(problems).toEqual([]);
    expect(findings[0]).toMatchObject({ outcome: "excepted", review: "2027-01-31" });
  });

  it("012 US1-4: an exception whose range doesn't cover the version doesn't apply", () => {
    const { problems } = checkDeprecated(
      lock(devGlob()),
      file(exception({ range: ">=11" })),
      TODAY,
    );
    expect(problems.some((p) => p.includes("glob@10.5.0"))).toBe(true);
    expect(problems.some((p) => p.includes("remove the exception"))).toBe(true);
  });

  it("012 US1-5: a shipped direct dependency fails even with a matching exception", () => {
    const packages = { "node_modules/shipdep": { version: "1.0.0", deprecated: "use other" } };
    const { problems } = checkDeprecated(
      lock(packages),
      file(exception({ package: "shipdep", range: "1", scope: "shipped" })),
      TODAY,
    );
    expect(problems.some((p) => p.includes("shipdep@1.0.0") && p.includes("Replace it"))).toBe(
      true,
    );
    expect(problems.some((p) => p.includes("can't be excepted"))).toBe(true);
  });

  it("012 US1-6: a nested and a top-level copy are judged separately on their own versions", () => {
    const packages = {
      ...devGlob(),
      "node_modules/a/node_modules/glob": { version: "9.0.0", dev: true, deprecated: "old" },
    };
    const { findings, problems } = checkDeprecated(lock(packages), file(exception()), TODAY);
    expect(findings.map((f) => [f.location, f.outcome])).toEqual([
      [GLOB, "excepted"],
      ["node_modules/a/node_modules/glob", "fails"],
    ]);
    expect(problems).toHaveLength(1);
    expect(problems[0]).toContain("node_modules/a/node_modules/glob");
  });

  it("012 US1-7: no deprecations and no exceptions gives nothing", () => {
    const { findings, problems } = checkDeprecated(
      lock({ "node_modules/react": { version: "18.3.1" } }),
      file(),
      TODAY,
    );
    expect({ findings, problems }).toEqual({ findings: [], problems: [] });
  });

  it("an empty deprecated string is not a finding", () => {
    const packages = { [GLOB]: { version: "10.5.0", deprecated: "" } };
    expect(checkDeprecated(lock(packages), file(), TODAY).findings).toEqual([]);
  });

  it("012 FR-004: a devOptional package counts as development-only", () => {
    const packages = { [GLOB]: { version: "10.5.0", devOptional: true, deprecated: "old" } };
    expect(checkDeprecated(lock(packages), file(), TODAY).findings[0].scope).toBe("dev");
  });

  it("012 FR-006: a dev direct dependency with a valid exception is excepted", () => {
    const packages = { "node_modules/devdep": { version: "1.0.0", dev: true, deprecated: "old" } };
    const { findings, problems } = checkDeprecated(
      lock(packages),
      file(exception({ package: "devdep", range: "1", via: "package.json" })),
      TODAY,
    );
    expect(findings[0]).toMatchObject({ direct: true, outcome: "excepted" });
    expect(problems).toEqual([]);
  });
});

describe("012 research.md §4 - satisfiesRange", () => {
  it.each([
    ["10.5.0", "10.5.0", true],
    ["10.5.1", "10.5.0", false],
    ["10.5.0", "=10.5.0", true],
    ["10.5.0", ">=10 <11", true],
    ["11.0.0", ">=10 <11", false],
    ["9.9.9", ">=10 <11", false],
    ["10.5.0", ">=10.0.0 <11.0.0", true],
    ["1.2.3", "<=1.2.3", true],
    ["1.2.4", "<=1.2.3", false],
    ["1.9.9", ">1", false],
    ["2.0.0", ">1", true],
    ["10.2.2", "^10.2.2", true],
    ["10.9.0", "^10.2.2", true],
    ["10.2.1", "^10.2.2", false],
    ["11.0.0", "^10.2.2", false],
    ["0.2.9", "^0.2.3", true],
    ["0.3.0", "^0.2.3", false],
    ["1.2.9", "~1.2.3", true],
    ["1.3.0", "~1.2.3", false],
    ["1.5.0", "*", true],
    ["1.5.0", "", true],
    ["1.5.0", "1.x || >=3", true],
    ["2.5.0", "1.x || >=3", false],
    ["3.0.0", "1.x || >=3", true],
    ["10.5.0-rc.1", "10.5.0", true],
    ["10.5.0-rc.1", ">=10.5.0 <10.6.0", true],
  ])("%s in %j is %s", (version, range, expected) => {
    expect(satisfiesRange(version, range)).toBe(expected);
  });

  it.each(["latest", ">=a", "1.2.3 - 2"])("throws on the unreadable range %j", (range) => {
    expect(() => satisfiesRange("1.0.0", range)).toThrow();
  });
});

describe("012 US2 - exceptions come back when their review date passes", () => {
  it("012 US2-1: an expired exception fails, naming it, and isn't also stale", () => {
    const { problems } = checkDeprecated(
      lock(devGlob()),
      file(exception({ review: "2026-10-31" })),
      TODAY,
    );
    expect(problems).toHaveLength(1);
    expect(problems[0]).toContain("exceptions[0]");
    expect(problems[0]).toContain("glob");
    expect(problems[0]).toContain("2026-10-31");
    expect(problems[0]).toContain("expired");
  });

  it("012 US2-2: an exception reviewed today still applies", () => {
    const { problems } = checkDeprecated(
      lock(devGlob()),
      file(exception({ review: TODAY })),
      TODAY,
    );
    expect(problems).toEqual([]);
  });

  it("012 US2-3: an exception that matches no finding fails as stale", () => {
    const { problems } = checkDeprecated(lock({}), file(exception()), TODAY);
    expect(problems).toHaveLength(1);
    expect(problems[0]).toContain("exceptions[0]");
    expect(problems[0]).toContain("remove the exception");
  });

  it.each(["package", "range", "scope", "reason", "via", "successor", "review"])(
    "012 US2-4: a missing %s fails, naming the index and field",
    (field) => {
      const e: Record<string, unknown> = exception();
      delete e[field];
      const { problems } = checkDeprecated(lock(devGlob()), file(e), TODAY);
      expect(problems.some((p) => p.includes("exceptions[0]") && p.includes(`${field}:`))).toBe(
        true,
      );
    },
  );

  it.each([
    ["scope", { scope: "prod" }],
    ["review", { review: "2026-02-30" }],
    ["review", { review: "31/12/2026" }],
    ["range", { range: "latest" }],
    ["reason", { reason: "" }],
    ["via", { via: " " }],
    ["reviewed", { reviewed: "2027-01-01" }],
  ])("012 US2-4: a bad %s fails", (field, over) => {
    const { problems } = checkDeprecated(lock(devGlob()), file(exception(over)), TODAY);
    expect(problems.some((p) => p.includes("exceptions[0]") && p.includes(`${field}:`))).toBe(true);
    // An invalid exception never matches, so the finding fails too.
    expect(problems.some((p) => p.includes("glob@10.5.0"))).toBe(true);
  });

  it("012 US2-4: a null successor is accepted", () => {
    const { problems } = checkDeprecated(
      lock(devGlob()),
      file(exception({ successor: null })),
      TODAY,
    );
    expect(problems).toEqual([]);
  });

  it("012 US2-4: a bad file shape fails", () => {
    expect(checkDeprecated(lock({}), { exceptions: [], extra: 1 }, TODAY).problems[0]).toContain(
      "extra: unknown key",
    );
    expect(checkDeprecated(lock({}), { exceptions: {} }, TODAY).problems[0]).toContain(
      "exceptions: must be an array",
    );
  });

  it("012 US2-5: a dev exception doesn't cover the same package when it ships", () => {
    const { problems } = checkDeprecated(lock(shippedGlob), file(exception()), TODAY);
    expect(problems.some((p) => p.includes("glob@10.5.0") && p.includes("ships"))).toBe(true);
    expect(problems.some((p) => p.includes("remove the exception"))).toBe(true);
  });
});
