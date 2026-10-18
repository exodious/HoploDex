// The SPDX evaluator shared by the audit and the notices (010 research.md §5):
// `satisfies()` as the audit has always judged an expression, and `choose()`,
// which resolves a choice of licenses to the one the notice names.
import { describe, expect, it } from "vitest";
import { ALLOWED } from "./policy.mjs";
import { choose, satisfies } from "./spdx.mjs";

const allowed = (id: string) => ALLOWED.has(id);

describe("010 research.md §5 - satisfies() keeps the audit's behaviour", () => {
  it("accepts a single allowed license and refuses an unknown one", () => {
    expect(satisfies("MIT", allowed)).toBe(true);
    expect(satisfies("Proprietary", allowed)).toBe(false);
  });

  it("OR needs one allowed part, AND needs every part", () => {
    expect(satisfies("Proprietary OR MIT", allowed)).toBe(true);
    expect(satisfies("MIT AND Proprietary", allowed)).toBe(false);
    expect(satisfies("MIT AND Apache-2.0", allowed)).toBe(true);
  });

  it("reads WITH as part of the identifier", () => {
    expect(satisfies("Apache-2.0 WITH LLVM-exception", allowed)).toBe(true);
    expect(satisfies("Apache-2.0 WITH Other-exception", allowed)).toBe(false);
  });

  it("honours parentheses", () => {
    expect(satisfies("(MIT OR Proprietary) AND ISC", allowed)).toBe(true);
    expect(satisfies("MIT AND (Proprietary OR Unknown)", allowed)).toBe(false);
  });

  it("throws on an unbalanced, trailing or missing token", () => {
    expect(() => satisfies("(MIT OR ISC", allowed)).toThrow(/unbalanced/);
    expect(() => satisfies("MIT ISC", allowed)).toThrow(/unexpected "ISC"/);
    expect(() => satisfies("MIT OR", allowed)).toThrow(/unexpected end/);
  });
});

describe("010 research.md §5 - choose() resolves a choice of licenses", () => {
  it("takes the first allowed license in PREFERENCE order, whatever the order written", () => {
    expect(choose("MIT OR Apache-2.0", allowed)).toBe("MIT");
    expect(choose("Apache-2.0 OR MIT", allowed)).toBe("MIT");
    expect(choose("Zlib OR Apache-2.0 OR MIT", allowed)).toBe("MIT");
    expect(choose("ISC OR BSD-3-Clause", allowed)).toBe("BSD-3-Clause");
  });

  it("skips a choice that isn't allowed", () => {
    expect(choose("Proprietary OR Zlib", allowed)).toBe("Zlib");
  });

  it("keeps every part of a conjunction", () => {
    expect(choose("MIT AND Apache-2.0", allowed)).toBe("MIT AND Apache-2.0");
    expect(choose("(MIT OR Apache-2.0) AND ISC", allowed)).toBe("MIT AND ISC");
    expect(choose("(Apache-2.0 OR MIT) AND (Zlib OR ISC)", allowed)).toBe("MIT AND ISC");
  });

  it("keeps a WITH exception whole", () => {
    expect(choose("Apache-2.0 WITH LLVM-exception", allowed)).toBe(
      "Apache-2.0 WITH LLVM-exception",
    );
    expect(choose("Apache-2.0 WITH LLVM-exception OR MIT", allowed)).toBe("MIT");
    expect(choose("Apache-2.0 WITH LLVM-exception OR MPL-2.0", allowed)).toBe("MPL-2.0");
  });

  it("returns nothing when no choice is allowed", () => {
    expect(choose("Proprietary", allowed)).toBeUndefined();
    expect(choose("Proprietary OR Unknown", allowed)).toBeUndefined();
    expect(choose("MIT AND Proprietary", allowed)).toBeUndefined();
    expect(choose("Apache-2.0 WITH Other-exception", allowed)).toBeUndefined();
  });

  it("ranks an allowed license outside PREFERENCE after every one inside it", () => {
    expect(choose("Unlicense OR Zlib", allowed)).toBe("Zlib");
    expect(choose("Unlicense", allowed)).toBe("Unlicense");
  });
});
