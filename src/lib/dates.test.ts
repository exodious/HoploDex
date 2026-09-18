import { describe, expect, it } from "vitest";
import { daysUntil, formatDate, parseDateInput } from "./dates";

describe("parseDateInput", () => {
  it("accepts ISO and US month/day/year forms and normalizes to ISO", () => {
    expect(parseDateInput("2019-03-14")).toEqual({ ok: true, iso: "2019-03-14" });
    expect(parseDateInput("3/14/2019")).toEqual({ ok: true, iso: "2019-03-14" });
    expect(parseDateInput("03-14-2019")).toEqual({ ok: true, iso: "2019-03-14" });
    expect(parseDateInput(" ")).toEqual({ ok: true, iso: null });
  });

  it("rejects dates that do not exist", () => {
    expect(parseDateInput("2023-02-29").ok).toBe(false);
    expect(parseDateInput("13/01/2020").ok).toBe(false);
    expect(parseDateInput("yesterday").ok).toBe(false);
  });
});

describe("formatDate", () => {
  it("renders the calendar date without a time-zone shift", () => {
    expect(formatDate("2019-03-01", "en-US")).toBe("Mar 1, 2019");
    expect(formatDate(null)).toBe("—");
  });
});

describe("daysUntil", () => {
  it("counts whole calendar days", () => {
    expect(daysUntil("2026-01-31", "2026-01-01")).toBe(30);
    expect(daysUntil("2025-12-27", "2026-01-01")).toBe(-5);
  });
});
