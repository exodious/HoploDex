import { describe, expect, it } from "vitest";
import {
  daysUntil,
  dispositionOrderError,
  formatDate,
  formatRecentDay,
  formatRecentMoment,
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

describe("formatRecentMoment and formatRecentDay (FR-040)", () => {
  const now = new Date(2026, 8, 29, 10, 30);
  const time = (d: Date) => new Intl.DateTimeFormat("en-US", { timeStyle: "short" }).format(d);

  it("names today and yesterday by the local calendar day", () => {
    const early = new Date(2026, 8, 29, 0, 5);
    const lateYesterday = new Date(2026, 8, 28, 23, 55);
    expect(formatRecentMoment(early.toISOString(), now, "en-US")).toBe(`Today at ${time(early)}`);
    expect(formatRecentMoment(lateYesterday.toISOString(), now, "en-US")).toBe(
      `Yesterday at ${time(lateYesterday)}`,
    );
    expect(formatRecentDay(early.toISOString(), now, "en-US")).toBe("today");
    expect(formatRecentDay(lateYesterday.toISOString(), now, "en-US")).toBe("yesterday");
  });

  it("gives an earlier day its date, with the year only when it differs", () => {
    const september = new Date(2026, 8, 12, 20, 5);
    const lastYear = new Date(2025, 11, 30, 9, 0);
    expect(formatRecentMoment(september.toISOString(), now, "en-US")).toBe(
      `September 12 at ${time(september)}`,
    );
    expect(formatRecentDay(lastYear.toISOString(), now, "en-US")).toBe("December 30, 2025");
  });

  it("reads a moment without a zone as local time, as backups are named", () => {
    expect(formatRecentMoment("2026-09-28T17:40:12", now, "en-US")).toBe(
      `Yesterday at ${time(new Date(2026, 8, 28, 17, 40, 12))}`,
    );
  });
});
