// Amounts are whole U.S. dollars everywhere (FR-037): they are entered,
// stored, sent over IPC and exported as whole dollars, never cents. Currency
// is USD, matching the app's U.S. federal-law scope. Thousands separators
// exist only in `formatDollars`; they are never typed, stored or exported.

const dollars = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
  maximumFractionDigits: 0,
});

/** Formats whole dollars for display ("$1,250"), or an em dash when unset. */
export function formatDollars(amount: number | null): string {
  return amount == null ? "—" : dollars.format(amount);
}

/** The editable text for an amount field ("1250", no grouping), or "" when unset. */
export function dollarsToInput(amount: number | null): string {
  return amount == null ? "" : String(amount);
}

/** The largest amount any money field may hold, in whole dollars (#67). The
 * backend enforces the same cap (`MAX_AMOUNT_DOLLARS` in
 * `src-tauri/src/models/rules.rs`); the two must agree, so change them
 * together. */
export const MAX_DOLLARS = 99_999_999;

/** The message every form shows for an amount above `MAX_DOLLARS`. */
export const TOO_LARGE_MESSAGE = `Enter an amount of ${formatDollars(MAX_DOLLARS)} or less.`;

export type ParsedDollars = { ok: true; dollars: number | null } | { ok: false; error: string };

const WHOLE_DOLLARS_MESSAGE = "Enter whole dollars only, like 1250, with no cents.";

/** Parses user-entered dollars. A "$", thousands commas and spaces are
 * dropped; blank means "no value". A fractional part, a sign, or anything
 * else that isn't digits is an error, never rounded or guessed at. */
export function parseDollars(input: string): ParsedDollars {
  const text = input.replace(/[$,\s]/g, "");
  if (text === "") return { ok: true, dollars: null };
  if (!/^\d+$/.test(text)) return { ok: false, error: WHOLE_DOLLARS_MESSAGE };
  // Compared as text first so a very long run of digits never has to become
  // a (rounded) number to be refused.
  const significant = text.replace(/^0+(?=\d)/, "");
  const amount = Number(significant);
  if (significant.length > String(MAX_DOLLARS).length || amount > MAX_DOLLARS) {
    return { ok: false, error: TOO_LARGE_MESSAGE };
  }
  return { ok: true, dollars: amount };
}
