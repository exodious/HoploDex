// Physical details are stored as scaled integers (FR-039): lengths in
// hundredths of an inch, weight in tenths of an ounce, so they stay exact.
// They are entered as decimals, sent over IPC as the scaled integer and
// exported as plain decimals. Anything typed with more precision than the
// stored unit is rounded half up to the nearest storable value, and a
// weight in pounds and/or ounces is converted the same way.

export type ParsedMeasure = { ok: true; value: number | null } | { ok: false; error: string };

/** Typed decimal text as an exact rational: `digits / 10^places`. */
interface Decimal {
  digits: bigint;
  places: number;
}

/** Reads plain decimal text ("16.25", ".5", "18"); null when it isn't one. */
function readDecimal(text: string): Decimal | null {
  if (!/^(\d+|\d*\.\d+)$/.test(text)) return null;
  const [whole = "", fraction = ""] = text.split(".");
  return { digits: BigInt(whole + fraction), places: fraction.length };
}

/** `value` scaled by `10^places`, rounded half up to a whole number. */
function scaleRounded({ digits, places: have }: Decimal, places: number): bigint {
  if (have <= places) return digits * 10n ** BigInt(places - have);
  const divisor = 10n ** BigInt(have - places);
  return (digits * 2n + divisor) / (2n * divisor);
}

const NOT_A_NUMBER = "Enter a number, like 16.25.";
const TOO_LARGE = "That number is too large.";
const NOT_POSITIVE = "Must be greater than 0.";

/** Decimal text → an integer scaled by `10^places`, rounded to the nearest
 * storable unit. Blank means "no value"; zero (typed, or rounded to) is
 * allowed here (see `parseScaled` for the "must be positive" rule). */
function parseNonNegative(input: string, places: number): ParsedMeasure {
  const text = input.trim();
  if (text === "") return { ok: true, value: null };
  const decimal = readDecimal(text);
  if (!decimal) return { ok: false, error: NOT_A_NUMBER };
  const scaled = Number(scaleRounded(decimal, places));
  if (!Number.isSafeInteger(scaled)) return { ok: false, error: TOO_LARGE };
  return { ok: true, value: scaled };
}

/** Like `parseNonNegative`, but zero is an error, including a value too
 * small to round up to one storable unit. */
function parseScaled(input: string, places: number): ParsedMeasure {
  const parsed = parseNonNegative(input, places);
  if (parsed.ok && parsed.value === 0) return { ok: false, error: NOT_POSITIVE };
  return parsed;
}

/** Decimal inches → hundredths of an inch. */
export function parseInches(input: string): ParsedMeasure {
  return parseScaled(input, 2);
}

export type ParsedWeight =
  { ok: true; value: number | null } | { ok: false; errors: { pounds?: string; ounces?: string } };

/** Pounds and ounces boxes → tenths of an ounce, the stored unit. Either box
 * may be blank and both may have decimals ("6.5" lb, "40.75" oz). The total
 * is computed exactly, then rounded half up to the nearest tenth of an
 * ounce (2.53 lb is 40.48 oz, stored as 40.5). */
export function parseWeight(pounds: string, ounces: string): ParsedWeight {
  const lbText = pounds.trim();
  const ozText = ounces.trim();
  const lb = lbText === "" ? null : readDecimal(lbText);
  const oz = ozText === "" ? null : readDecimal(ozText);
  if ((lbText !== "" && !lb) || (ozText !== "" && !oz)) {
    return {
      ok: false,
      errors: {
        pounds: lbText !== "" && !lb ? NOT_A_NUMBER : undefined,
        ounces: ozText !== "" && !oz ? NOT_A_NUMBER : undefined,
      },
    };
  }
  if (!lb && !oz) return { ok: true, value: null };
  // Tenths of an ounce as an exact fraction over 10^places.
  const places = Math.max(lb?.places ?? 0, oz?.places ?? 0);
  const align = (d: Decimal | null, perUnit: bigint) =>
    d ? d.digits * 10n ** BigInt(places - d.places) * perUnit : 0n;
  const tenths = Number(scaleRounded({ digits: align(lb, 160n) + align(oz, 10n), places }, 0));
  if (!Number.isSafeInteger(tenths)) return { ok: false, errors: { pounds: TOO_LARGE } };
  if (tenths === 0) {
    return { ok: false, errors: { [oz ? "ounces" : "pounds"]: NOT_POSITIVE } };
  }
  return { ok: true, value: tenths };
}

function scaledToText(value: number, places: number): string {
  const scale = 10 ** places;
  const whole = Math.floor(value / scale);
  const fraction = String(value % scale)
    .padStart(places, "0")
    .replace(/0+$/, "");
  return fraction === "" ? String(whole) : `${whole}.${fraction}`;
}

/** Hundredths of an inch as "16.25" (no trailing zeros, no unit). */
export function formatInches(hundredths: number): string {
  return scaledToText(hundredths, 2);
}

/** Tenths of an ounce as pounds and ounces: "2 lb 8.5 oz", "1 lb", "8 oz". */
export function formatWeight(tenthsOz: number): string {
  const pounds = Math.floor(tenthsOz / 160);
  const ounces = tenthsOz % 160;
  const parts: string[] = [];
  if (pounds > 0) parts.push(`${pounds} lb`);
  if (ounces > 0 || pounds === 0) parts.push(`${scaledToText(ounces, 1)} oz`);
  return parts.join(" ");
}

/** The editable text for a length field, or "" when unset. */
export function inchesToInput(hundredths: number | null): string {
  return hundredths == null ? "" : formatInches(hundredths);
}

/** The editable pounds and ounces text for a weight, blank when unset:
 * 405 → "2" lb and "8.5" oz, 80 → "" lb and "8" oz. */
export function weightToInputs(tenthsOz: number | null): { pounds: string; ounces: string } {
  if (tenthsOz == null) return { pounds: "", ounces: "" };
  const pounds = Math.floor(tenthsOz / 160);
  const ounces = tenthsOz % 160;
  return {
    pounds: pounds > 0 ? String(pounds) : "",
    ounces: ounces > 0 ? scaledToText(ounces, 1) : "",
  };
}
