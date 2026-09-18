// Amounts are integer cents everywhere past the input boundary
// (data-model.md). Currency is USD, matching the app's U.S. federal-law scope.

const withCents = new Intl.NumberFormat("en-US", { style: "currency", currency: "USD" });
const wholeDollars = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
  maximumFractionDigits: 0,
});
const grouped = new Intl.NumberFormat("en-US", {
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});

/** Formats cents as dollars ("$1,234.56"), or an em dash when unset.
 * `whole` drops ".00" for display totals, keeping any real cents. */
export function formatCents(cents: number | null, options: { whole?: boolean } = {}): string {
  if (cents == null) return "—";
  if (options.whole && cents % 100 === 0) return wholeDollars.format(cents / 100);
  return withCents.format(cents / 100);
}

/** The editable text for an amount field ("1,200.50"), or "" when unset. */
export function centsToInput(cents: number | null): string {
  return cents == null ? "" : grouped.format(cents / 100);
}

export type ParsedDollars = { ok: true; cents: number | null } | { ok: false; error: string };

// Whole dollars (optionally grouped by commas in threes), then up to two
// decimal places.
const AMOUNT = /^(?:\d{1,3}(?:,\d{3})+|\d+)?(?:\.(\d{1,2}))?$/;

/** Parses user-typed dollars into cents. Blank means "no value"; anything
 * that isn't a plain non-negative amount is an error, never a guess. */
export function parseDollars(input: string): ParsedDollars {
  const text = input.replace(/[$\s]/g, "");
  if (text === "") return { ok: true, cents: null };
  const match = AMOUNT.exec(text);
  if (!match || text === ".") {
    return { ok: false, error: "Enter an amount like 1,250.00." };
  }
  const [whole, fraction = ""] = text.replace(/,/g, "").split(".");
  const cents = Number(whole || "0") * 100 + Number(fraction.padEnd(2, "0"));
  return { ok: true, cents };
}
