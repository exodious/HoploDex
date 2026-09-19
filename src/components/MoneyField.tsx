import { centsToInput, parseDollars } from "../lib/money";
import { TextField } from "./TextField";
import type { TextFieldProps } from "./TextField";

export interface MoneyFieldProps extends Omit<
  TextFieldProps,
  "value" | "onChange" | "prefix" | "inputMode"
> {
  /** The raw text being edited; parse it with `parseDollars` on submit. */
  value: string;
  onValueChange: (text: string) => void;
}

/** Dollar amount input: "$" prefix, accepts "1,250.00" or "1250", and
 * tidies a valid entry to grouped form when the field loses focus. */
export function MoneyField({ value, onValueChange, onBlur, ...props }: MoneyFieldProps) {
  return (
    <TextField
      {...props}
      prefix="$"
      inputMode="decimal"
      autoComplete="off"
      value={value}
      onChange={(e) => onValueChange(e.target.value)}
      onBlur={(e) => {
        const parsed = parseDollars(value);
        if (parsed.ok && parsed.cents != null) onValueChange(centsToInput(parsed.cents));
        onBlur?.(e);
      }}
    />
  );
}
