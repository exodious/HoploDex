// Physical details are stored as scaled integers (FR-039): lengths in
// hundredths of an inch, weight in tenths of an ounce, so they stay exact.
// They are entered as decimals, sent over IPC as the scaled integer and
// exported as plain decimals; nothing is ever rounded to fit, except a
// weight entered in pounds, which is converted to the nearest tenth of an
// ounce (`parseWeight`).

export type ParsedMeasure = { ok: true; value: number | null } | { ok: false; error: string };

/** "Use at most 2 decimal places." */
export function tooManyPlacesMessage(places: number): string {
  return `Use at most ${places} decimal ${places === 1 ? "place" : "places"}.`;
}

/** True when the digits after the decimal point exceed `places` and are not
 * just a zero fraction (`16.250` fits in two places, `16.255` does not). */
export function hasTooManyPlaces(text: string, places: number): boolean {
  const fraction = text.split(".")[1] ?? "";
  return /[1-9]/.test(fraction.slice(places));
}

/** Parses user-entered decimals into an integer scaled by `10^places`; zero
 * is a valid result here (see `parseScaled` for the "must be positive" rule). */
function parseNonNegative(input: string, places: number): ParsedMeasure {
  const text = input.trim();
  if (text === "") return { ok: true, value: null };
  const match = /^(\d*)(?:\.(\d*))?$/.exec(text);
  if (!match || !/^(\d+|\d*\.\d+)$/.test(text)) {
    return { ok: false, error: "Enter a number, like 16.25." };
  }
  if (hasTooManyPlaces(text, places)) return { ok: false, error: tooManyPlacesMessage(places) };
  const [, whole, fraction = ""] = match;
  const scaled = Number(whole + fraction.slice(0, places).padEnd(places, "0"));
  if (!Number.isSafeInteger(scaled)) return { ok: false, error: "That number is too large." };
  return { ok: true, value: scaled };
}

/** Like `parseNonNegative`, but zero is an error. Blank means "no value";
 * more precision than `places`, a sign, or anything that isn't a number is an
 * error, never rounded or guessed at. */
function parseScaled(input: string, places: number): ParsedMeasure {
  const parsed = parseNonNegative(input, places);
  if (parsed.ok && parsed.value === 0) return { ok: false, error: "Must be greater than 0." };
  return parsed;
}

/** Decimal inches → hundredths of an inch. */
export function parseInches(input: string): ParsedMeasure {
  return parseScaled(input, 2);
}

/** How many decimal places each weight box takes: enough for manufacturer
 * figures like "6.625 lb" or "40.75 oz". */
export const WEIGHT_PLACES = 3;

export type ParsedWeight =
  { ok: true; value: number | null } | { ok: false; errors: { pounds?: string; ounces?: string } };

/** Pounds and ounces boxes → tenths of an ounce, the stored unit. Either box
 * may be blank, both may have decimals ("6.5" lb, "40.75" oz). Unlike the
 * other measures, a value that doesn't land on a tenth of an ounce (2.53 lb
 * is 40.48 oz) is converted to the nearest tenth, because unit conversion
 * can't be exact; each box's own precision is still checked. */
export function parseWeight(pounds: string, ounces: string): ParsedWeight {
  const lb = parseNonNegative(pounds, WEIGHT_PLACES);
  const oz = parseNonNegative(ounces, WEIGHT_PLACES);
  if (!lb.ok || !oz.ok) {
    return {
      ok: false,
      errors: { pounds: lb.ok ? undefined : lb.error, ounces: oz.ok ? undefined : oz.error },
    };
  }
  if (lb.value === null && oz.value === null) return { ok: true, value: null };
  // Thousandths of an ounce, exact, then to the nearest tenth.
  const thousandths = (lb.value ?? 0) * 16 + (oz.value ?? 0);
  const tenths = Math.floor((thousandths + 50) / 100);
  if (tenths === 0) {
    const field = oz.value === null ? "pounds" : "ounces";
    return { ok: false, errors: { [field]: "Must be greater than 0." } };
  }
  if (!Number.isSafeInteger(tenths)) {
    return { ok: false, errors: { pounds: "That number is too large." } };
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
