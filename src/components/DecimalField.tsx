import { hasTooManyPlaces, tooManyPlacesMessage } from "../lib/measure";
import { TextField } from "./TextField";
import type { TextFieldProps } from "./TextField";

export interface DecimalFieldProps extends Omit<
  TextFieldProps,
  "value" | "onChange" | "prefix" | "inputMode"
> {
  /** The text being edited; parse it with `parseInches`/`parseOunces` on submit. */
  value: string;
  onValueChange: (text: string) => void;
  /** How many decimal places the value may have. */
  places: number;
}

/** What the field keeps of typed or pasted text: a "$", commas and spaces
 * are dropped, then only digits and the first "." remain. */
function sanitize(text: string): string {
  const cleaned = text.replace(/[^\d.]/g, "");
  const point = cleaned.indexOf(".");
  return point < 0
    ? cleaned
    : cleaned.slice(0, point + 1) + cleaned.slice(point + 1).replace(/\./g, "");
}

/** Decimal input for a measurement (FR-039), modelled on `MoneyField`:
 * digits and one decimal point only. A value with more decimal places than
 * `places` gets a field-level message and is left as typed, never rounded. */
export function DecimalField({ value, onValueChange, places, error, ...props }: DecimalFieldProps) {
  const placesError = hasTooManyPlaces(value, places) ? tooManyPlacesMessage(places) : undefined;
  return (
    <TextField
      {...props}
      error={error ?? placesError}
      inputMode="decimal"
      autoComplete="off"
      value={value}
      onChange={(e) => onValueChange(sanitize(e.target.value))}
    />
  );
}
