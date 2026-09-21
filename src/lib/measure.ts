// Physical details are stored as scaled integers (FR-039): lengths in
// hundredths of an inch, weight in tenths of an ounce, so they stay exact.
// They are entered as decimals, sent over IPC as the scaled integer and
// exported as plain decimals; nothing is ever rounded to fit.

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

/** Parses user-entered decimals into an integer scaled by `10^places`. Blank
 * means "no value". More precision than `places`, zero, a sign, or anything
 * that isn't a number is an error, never rounded or guessed at. */
function parseScaled(input: string, places: number): ParsedMeasure {
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
  if (scaled === 0) return { ok: false, error: "Must be greater than 0." };
  return { ok: true, value: scaled };
}

/** Decimal inches → hundredths of an inch. */
export function parseInches(input: string): ParsedMeasure {
  return parseScaled(input, 2);
}

/** Decimal ounces → tenths of an ounce. */
export function parseOunces(input: string): ParsedMeasure {
  return parseScaled(input, 1);
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

/** The editable text for a weight-in-ounces field, or "" when unset. */
export function ouncesToInput(tenthsOz: number | null): string {
  return tenthsOz == null ? "" : scaledToText(tenthsOz, 1);
}
