import { describe, expect, it } from "vitest";
import {
  daysUntil,
  dispositionOrderError,
  formatDate,
  futureDateError,
  parseDateInput,
} from "./dates";

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

describe("futureDateError (FR-003 / FR-004)", () => {
  it("allows today and earlier, and blank", () => {
    expect(futureDateError("2026-09-20", "Acquisition date", "2026-09-20")).toBeUndefined();
    expect(futureDateError("1968-10-22", "Acquisition date", "2026-09-20")).toBeUndefined();
    expect(futureDateError(null, "Acquisition date", "2026-09-20")).toBeUndefined();
  });

  it("blocks a date after today, naming the field", () => {
    expect(futureDateError("2026-09-21", "Disposition date", "2026-09-20")).toBe(
      "Disposition date can't be in the future.",
    );
  });
});

describe("dispositionOrderError (FR-004)", () => {
  it("blocks a disposition before the acquisition, but not the same day", () => {
    expect(dispositionOrderError("2025-03-01", "2025-02-28")).toBe(
      "Disposition date can't be earlier than the acquisition date.",
    );
    expect(dispositionOrderError("2025-03-01", "2025-03-01")).toBeUndefined();
  });

  it("does not compare when either date is missing", () => {
    expect(dispositionOrderError(null, "2025-02-28")).toBeUndefined();
    expect(dispositionOrderError("2025-03-01", null)).toBeUndefined();
  });
});
