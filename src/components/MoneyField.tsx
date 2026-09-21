import { useState } from "react";
import { parseDollars } from "../lib/money";
import { TextField } from "./TextField";
import type { TextFieldProps } from "./TextField";

export interface MoneyFieldProps extends Omit<
  TextFieldProps,
  "value" | "onChange" | "prefix" | "inputMode"
> {
  /** The digits being edited; parse them with `parseDollars` on submit. */
  value: string;
  onValueChange: (text: string) => void;
}

/** Whole-dollar input (FR-037): "$" prefix, digits only. A typed ".", "," or
 * other character is ignored; a pasted "$", commas or spaces are dropped; a
 * pasted amount with cents is refused with a message, never rounded. Grouping
 * ("$1,250") is display-only and never appears in the field. */
export function MoneyField({ value, onValueChange, error, onPaste, ...props }: MoneyFieldProps) {
  const [pasteError, setPasteError] = useState<string>();
  return (
    <TextField
      {...props}
      error={pasteError ?? error}
      prefix="$"
      inputMode="numeric"
      autoComplete="off"
      value={value}
      onChange={(e) => {
        setPasteError(undefined);
        onValueChange(e.target.value.replace(/\D/g, ""));
      }}
      onPaste={(e) => {
        const parsed = parseDollars(e.clipboardData.getData("text"));
        if (parsed.ok) {
          setPasteError(undefined);
        } else {
          e.preventDefault();
          setPasteError(parsed.error);
        }
        onPaste?.(e);
      }}
    />
  );
}
