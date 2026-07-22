import { forwardRef, useId } from "react";
import * as RadixCheckbox from "@radix-ui/react-checkbox";
import "./components.css";

export interface CheckboxProps {
  label: string;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  id?: string;
  disabled?: boolean;
}

/** Accessible checkbox with a fully clickable, always-visible label. */
export const Checkbox = forwardRef<HTMLButtonElement, CheckboxProps>(
  ({ label, checked, onCheckedChange, id, disabled }, ref) => {
    const generatedId = useId();
    const checkboxId = id ?? generatedId;

    return (
      <div className="hd-checkbox">
        <RadixCheckbox.Root
          ref={ref}
          id={checkboxId}
          className="hd-checkbox__box"
          checked={checked}
          onCheckedChange={(state) => onCheckedChange(state === true)}
          disabled={disabled}
        >
          <RadixCheckbox.Indicator className="hd-checkbox__indicator">✓</RadixCheckbox.Indicator>
        </RadixCheckbox.Root>
        <label className="hd-checkbox__label" htmlFor={checkboxId}>
          {label}
        </label>
      </div>
    );
  },
);

Checkbox.displayName = "Checkbox";
