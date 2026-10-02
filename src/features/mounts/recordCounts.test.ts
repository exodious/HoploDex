import { describe, expect, it } from "vitest";
import { countKinds, describeCounts, hasAny, kindHeading, kindNoun } from "./recordCounts";

// Issue #56: a count or a noun for firearms and accessories says which.

const counts = (firearms: number, accessories: number) => ({ firearms, accessories });

describe("recordCounts", () => {
  it("counts a set by kind", () => {
    expect(countKinds([{ kind: "firearm" }, { kind: "accessory" }, { kind: "accessory" }])).toEqual(
      counts(1, 2),
    );
    expect(hasAny(counts(0, 0))).toBe(false);
    expect(hasAny(counts(0, 1))).toBe(true);
  });

  it("says what a count counts", () => {
    expect(describeCounts(counts(1, 0))).toBe("1 firearm");
    expect(describeCounts(counts(0, 3))).toBe("3 accessories");
    expect(describeCounts(counts(2, 1))).toBe("2 firearms and 1 accessory");
    expect(describeCounts(counts(0, 0))).toBe("no firearms or accessories");
  });

  it("names the kinds in a set, singular for one unless asked for the plural", () => {
    expect(kindNoun(counts(1, 0))).toBe("firearm");
    expect(kindNoun(counts(0, 1))).toBe("accessory");
    expect(kindNoun(counts(0, 1), true)).toBe("accessories");
    expect(kindNoun(counts(2, 0))).toBe("firearms");
    expect(kindNoun(counts(1, 1))).toBe("firearms and accessories");
  });

  it("heads a column by the kinds in it", () => {
    expect(kindHeading(counts(2, 0))).toBe("Firearm");
    expect(kindHeading(counts(0, 2))).toBe("Accessory");
    expect(kindHeading(counts(1, 1))).toBe("Firearm or accessory");
    expect(kindHeading(counts(0, 0))).toBe("Firearm or accessory");
  });
});
