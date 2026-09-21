import { describe, expect, it } from "vitest";
import {
  formatInches,
  formatWeight,
  inchesToInput,
  parseInches,
  parseWeight,
  weightToInputs,
} from "./measure";

describe("parseInches (FR-039)", () => {
  it("reads decimal inches as hundredths", () => {
    expect(parseInches("16.25")).toEqual({ ok: true, value: 1625 });
    expect(parseInches("18")).toEqual({ ok: true, value: 1800 });
    expect(parseInches("4.5")).toEqual({ ok: true, value: 450 });
    expect(parseInches(".5")).toEqual({ ok: true, value: 50 });
    expect(parseInches(" 16.25 ")).toEqual({ ok: true, value: 1625 });
  });

  it("treats a blank entry as no value", () => {
    expect(parseInches("")).toEqual({ ok: true, value: null });
    expect(parseInches("   ")).toEqual({ ok: true, value: null });
  });

  it("accepts a zero fraction beyond the precision", () => {
    expect(parseInches("16.250")).toEqual({ ok: true, value: 1625 });
  });

  it("refuses more than two decimal places instead of rounding", () => {
    const result = parseInches("16.255");
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.error).toMatch(/2 decimal places/);
  });

  it("refuses zero, negative and non-numeric values", () => {
    for (const bad of ["0", "0.00", "-1", "abc", "1.2.3", "16.", "."]) {
      expect(parseInches(bad).ok, bad).toBe(false);
    }
  });
});

describe("parseWeight (FR-039)", () => {
  it("reads ounces alone as tenths", () => {
    expect(parseWeight("", "40.5")).toEqual({ ok: true, value: 405 });
    expect(parseWeight("", "32")).toEqual({ ok: true, value: 320 });
  });

  it("reads pounds alone, with decimals", () => {
    expect(parseWeight("6", "")).toEqual({ ok: true, value: 960 });
    expect(parseWeight("6.5", "")).toEqual({ ok: true, value: 1040 });
    expect(parseWeight("6.625", "")).toEqual({ ok: true, value: 1060 });
  });

  it("adds pounds and ounces", () => {
    expect(parseWeight("2", "8.5")).toEqual({ ok: true, value: 405 });
    expect(parseWeight("0", "8")).toEqual({ ok: true, value: 80 });
    expect(parseWeight("1", "40")).toEqual({ ok: true, value: 560 });
  });

  it("converts to the nearest tenth of an ounce", () => {
    expect(parseWeight("2.53", "")).toEqual({ ok: true, value: 405 }); // 40.48 oz
    expect(parseWeight("", "40.55")).toEqual({ ok: true, value: 406 });
    expect(parseWeight("", "40.54")).toEqual({ ok: true, value: 405 });
  });

  it("treats two blank boxes as no value", () => {
    expect(parseWeight("", "")).toEqual({ ok: true, value: null });
    expect(parseWeight(" ", " ")).toEqual({ ok: true, value: null });
  });

  it("refuses zero, negative and non-numeric entries, naming the box", () => {
    expect(parseWeight("0", "")).toEqual({
      ok: false,
      errors: { pounds: "Must be greater than 0." },
    });
    expect(parseWeight("0", "0")).toEqual({
      ok: false,
      errors: { ounces: "Must be greater than 0." },
    });
    expect(parseWeight("", "0.04")).toEqual({
      ok: false,
      errors: { ounces: "Must be greater than 0." },
    });
    const bad = parseWeight("-2", "abc");
    expect(bad.ok).toBe(false);
    if (!bad.ok) {
      expect(bad.errors.pounds).toMatch(/Enter a number/);
      expect(bad.errors.ounces).toMatch(/Enter a number/);
    }
  });

  it("refuses more than three decimal places in a box", () => {
    const result = parseWeight("6.6251", "");
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.errors.pounds).toMatch(/3 decimal places/);
  });
});

describe("formatting (FR-039)", () => {
  it("shows inches without trailing zeros", () => {
    expect(formatInches(1625)).toBe("16.25");
    expect(formatInches(1800)).toBe("18");
    expect(formatInches(1650)).toBe("16.5");
    expect(formatInches(5)).toBe("0.05");
  });

  it("shows weight as pounds and ounces", () => {
    expect(formatWeight(405)).toBe("2 lb 8.5 oz");
    expect(formatWeight(160)).toBe("1 lb");
    expect(formatWeight(80)).toBe("8 oz");
    expect(formatWeight(155)).toBe("15.5 oz");
    expect(formatWeight(1005)).toBe("6 lb 4.5 oz");
  });

  it("gives the editable text for a field, blank when unset", () => {
    expect(inchesToInput(1625)).toBe("16.25");
    expect(inchesToInput(null)).toBe("");
    expect(weightToInputs(405)).toEqual({ pounds: "2", ounces: "8.5" });
    expect(weightToInputs(80)).toEqual({ pounds: "", ounces: "8" });
    expect(weightToInputs(160)).toEqual({ pounds: "1", ounces: "" });
    expect(weightToInputs(null)).toEqual({ pounds: "", ounces: "" });
  });
});
