import { describe, expect, it } from "vitest";
import { centsToInput, formatCents, parseDollars } from "./money";

describe("parseDollars", () => {
  it("treats a blank field as no value", () => {
    expect(parseDollars("  ")).toEqual({ ok: true, cents: null });
  });

  it("reads thousands separators and a leading dollar sign instead of truncating", () => {
    // Regression: parseFloat("1,200.00") is 1, which silently saved $1.00.
    expect(parseDollars("1,200.00")).toEqual({ ok: true, cents: 120000 });
    expect(parseDollars("$ 12,345")).toEqual({ ok: true, cents: 1234500 });
  });

  it("converts without floating-point drift", () => {
    expect(parseDollars("0.29")).toEqual({ ok: true, cents: 29 });
    expect(parseDollars("1234.5")).toEqual({ ok: true, cents: 123450 });
    expect(parseDollars(".5")).toEqual({ ok: true, cents: 50 });
  });

  it("rejects text that is not an amount rather than guessing", () => {
    // Regression: parseFloat("12abc") is 12.
    expect(parseDollars("12abc").ok).toBe(false);
    expect(parseDollars("1.234").ok).toBe(false);
    expect(parseDollars("-5").ok).toBe(false);
    expect(parseDollars("1,2,3.00").ok).toBe(false);
  });
});

describe("formatCents", () => {
  it("groups thousands and shows an em dash for no value", () => {
    expect(formatCents(123456789)).toBe("$1,234,567.89");
    expect(formatCents(null)).toBe("—");
  });

  it("can drop the cents on whole-dollar display amounts", () => {
    expect(formatCents(4825000, { whole: true })).toBe("$48,250");
    expect(formatCents(4825050, { whole: true })).toBe("$48,250.50");
  });
});

describe("centsToInput", () => {
  it("round-trips through parseDollars", () => {
    expect(centsToInput(120050)).toBe("1,200.50");
    expect(parseDollars(centsToInput(120050))).toEqual({ ok: true, cents: 120050 });
    expect(centsToInput(null)).toBe("");
  });
});
