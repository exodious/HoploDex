// Dates are ISO `YYYY-MM-DD` calendar dates on the wire (data-model.md) —
// no time, no zone. Everything here treats them as UTC midnights so
// formatting never shifts a date across a zone boundary.

const DAY_MS = 86_400_000;

function toUtc(iso: string): Date {
  const [y, m, d] = iso.split("-").map(Number);
  return new Date(Date.UTC(y, m - 1, d));
}

function isRealDate(y: number, m: number, d: number): boolean {
  const date = new Date(Date.UTC(y, m - 1, d));
  return date.getUTCFullYear() === y && date.getUTCMonth() === m - 1 && date.getUTCDate() === d;
}

function pad(n: number): string {
  return String(n).padStart(2, "0");
}

export function toIso(y: number, m: number, d: number): string {
  return `${String(y).padStart(4, "0")}-${pad(m)}-${pad(d)}`;
}

/** Today's local calendar date as ISO. */
export function todayIso(): string {
  const now = new Date();
  return toIso(now.getFullYear(), now.getMonth() + 1, now.getDate());
}

/** "Mar 1, 2019" in the given (default: user's) locale, or an em dash. */
export function formatDate(iso: string | null, locale?: string): string {
  if (!iso) return "—";
  return new Intl.DateTimeFormat(locale, {
    year: "numeric",
    month: "short",
    day: "numeric",
    timeZone: "UTC",
  }).format(toUtc(iso));
}

/** A moment (ISO-8601 with a time and zone, such as `2026-09-25T14:30:05Z`)
 * as the user's local date and time. */
export function formatDateTime(iso: string, locale?: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return new Intl.DateTimeFormat(locale, { dateStyle: "long", timeStyle: "short" }).format(date);
}

/** Whole days from `fromIso` to `toIso`; negative once `toIso` has passed. */
export function daysUntil(toIsoDate: string, fromIso: string = todayIso()): number {
  return Math.round((toUtc(toIsoDate).getTime() - toUtc(fromIso).getTime()) / DAY_MS);
}

export type ParsedDate = { ok: true; iso: string | null } | { ok: false; error: string };

/** Accepts `YYYY-MM-DD` (or with slashes) and U.S. `M/D/YYYY` (or with
 * dashes); blank means "no date". */
export function parseDateInput(input: string): ParsedDate {
  const text = input.trim();
  if (text === "") return { ok: true, iso: null };

  let y: number, m: number, d: number;
  const isoMatch = /^(\d{4})[-/](\d{1,2})[-/](\d{1,2})$/.exec(text);
  const usMatch = /^(\d{1,2})[-/](\d{1,2})[-/](\d{4})$/.exec(text);
  if (isoMatch) {
    [y, m, d] = [Number(isoMatch[1]), Number(isoMatch[2]), Number(isoMatch[3])];
  } else if (usMatch) {
    [m, d, y] = [Number(usMatch[1]), Number(usMatch[2]), Number(usMatch[3])];
  } else {
    return { ok: false, error: "Enter a date like 2024-03-14." };
  }

  if (!isRealDate(y, m, d)) {
    return { ok: false, error: "That date doesn't exist." };
  }
  return { ok: true, iso: toIso(y, m, d) };
}

/** FR-003 / FR-004: an acquisition or disposition date can't be after
 * today (today itself is fine). Messages match the backend's, which
 * enforces the same rule. `undefined` means the date is acceptable. */
export function futureDateError(
  iso: string | null,
  label: string,
  today: string = todayIso(),
): string | undefined {
  return iso && iso > today ? `${label} can't be in the future.` : undefined;
}

/** FR-004: a disposition can't predate the acquisition. Nothing to compare
 * unless both dates are recorded. */
export function dispositionOrderError(
  acquiredIso: string | null,
  disposedIso: string | null,
): string | undefined {
  return acquiredIso && disposedIso && disposedIso < acquiredIso
    ? "Disposition date can't be earlier than the acquisition date."
    : undefined;
}
