import { describe, expect, it } from "vitest";
import { dollarsToInput, formatDollars, parseDollars } from "./money";

describe("parseDollars (FR-037)", () => {
  it("treats a blank field as no value", () => {
    expect(parseDollars("  ")).toEqual({ ok: true, dollars: null });
    expect(parseDollars("")).toEqual({ ok: true, dollars: null });
  });

  it("reads digits as whole dollars", () => {
    expect(parseDollars("1250")).toEqual({ ok: true, dollars: 1250 });
    expect(parseDollars("0")).toEqual({ ok: true, dollars: 0 });
  });

  it("drops a dollar sign, thousands commas and spaces instead of truncating", () => {
    // Regression: parseFloat("1,200") is 1, which silently saved $1.
    expect(parseDollars("1,200")).toEqual({ ok: true, dollars: 1200 });
    expect(parseDollars("$ 12,345")).toEqual({ ok: true, dollars: 12345 });
    expect(parseDollars(" $1,250 ")).toEqual({ ok: true, dollars: 1250 });
  });

  it("rejects a fractional part rather than rounding or truncating it", () => {
    // Silent rounding could change an amount by a factor of 100.
    for (const text of ["1250.50", "0.29", ".5", "1250.00", "1250."]) {
      const parsed = parseDollars(text);
      expect(parsed.ok, text).toBe(false);
      if (!parsed.ok) expect(parsed.error).toMatch(/whole dollars/i);
    }
  });

  it("rejects text that is not an amount rather than guessing", () => {
    // Regression: parseFloat("12abc") is 12.
    for (const text of ["12abc", "-5", "1e3", "abc"]) {
      expect(parseDollars(text).ok, text).toBe(false);
    }
  });

  it("rejects an amount too large to hold exactly", () => {
    expect(parseDollars("99999999999999999999").ok).toBe(false);
  });
});

describe("formatDollars (FR-037)", () => {
  it("groups thousands and never shows cents", () => {
    expect(formatDollars(1000)).toBe("$1,000");
    expect(formatDollars(100)).toBe("$100");
    expect(formatDollars(1250)).toBe("$1,250");
    expect(formatDollars(1234567)).toBe("$1,234,567");
    expect(formatDollars(0)).toBe("$0");
  });

  it("shows an em dash for no value", () => {
    expect(formatDollars(null)).toBe("—");
  });
});

describe("dollarsToInput", () => {
  it("is plain digits with no grouping, and round-trips through parseDollars", () => {
    expect(dollarsToInput(1250)).toBe("1250");
    expect(dollarsToInput(0)).toBe("0");
    expect(dollarsToInput(null)).toBe("");
    expect(parseDollars(dollarsToInput(1250))).toEqual({ ok: true, dollars: 1250 });
  });
});
