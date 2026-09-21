import { TextField } from "./TextField";
import type { TextFieldProps } from "./TextField";

export interface DecimalFieldProps extends Omit<
  TextFieldProps,
  "value" | "onChange" | "prefix" | "inputMode"
> {
  /** The text being edited; parse it with `parseInches`/`parseWeight` on submit. */
  value: string;
  onValueChange: (text: string) => void;
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
 * digits and one decimal point only. Extra decimal places are kept as typed;
 * the parser rounds to the stored unit on submit. */
export function DecimalField({ value, onValueChange, error, ...props }: DecimalFieldProps) {
  return (
    <TextField
      {...props}
      error={error}
      inputMode="decimal"
      autoComplete="off"
      value={value}
      onChange={(e) => onValueChange(sanitize(e.target.value))}
    />
  );
}
