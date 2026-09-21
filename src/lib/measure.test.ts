import { describe, expect, it } from "vitest";
import {
  formatInches,
  formatWeight,
  inchesToInput,
  ouncesToInput,
  parseInches,
  parseOunces,
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

describe("parseOunces (FR-039)", () => {
  it("reads decimal ounces as tenths", () => {
    expect(parseOunces("40.5")).toEqual({ ok: true, value: 405 });
    expect(parseOunces("32")).toEqual({ ok: true, value: 320 });
    expect(parseOunces("")).toEqual({ ok: true, value: null });
  });

  it("refuses more than one decimal place instead of rounding", () => {
    const result = parseOunces("40.55");
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.error).toMatch(/1 decimal place/);
  });

  it("refuses zero and negative values", () => {
    expect(parseOunces("0").ok).toBe(false);
    expect(parseOunces("-2").ok).toBe(false);
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
    expect(ouncesToInput(405)).toBe("40.5");
    expect(ouncesToInput(null)).toBe("");
  });
});
