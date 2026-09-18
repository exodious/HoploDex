import { forwardRef, useId } from "react";
import type { ReactNode } from "react";
import * as RadixCheckbox from "@radix-ui/react-checkbox";
import { Icon } from "./Icon";
import "./components.css";

export interface CheckboxProps {
  label: string;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  hint?: ReactNode;
  id?: string;
  disabled?: boolean;
}

/** Accessible checkbox with a fully clickable, always-visible label. */
export const Checkbox = forwardRef<HTMLButtonElement, CheckboxProps>(
  ({ label, checked, onCheckedChange, hint, id, disabled }, ref) => {
    const generatedId = useId();
    const checkboxId = id ?? generatedId;
    const hintId = hint ? `${checkboxId}-hint` : undefined;

    return (
      <div className="hd-checkbox">
        <RadixCheckbox.Root
          ref={ref}
          id={checkboxId}
          className="hd-checkbox__box"
          checked={checked}
          onCheckedChange={(state) => onCheckedChange(state === true)}
          disabled={disabled}
          aria-describedby={hintId}
        >
          <RadixCheckbox.Indicator className="hd-checkbox__indicator">
            <Icon name="check" size={14} strokeWidth={2.4} />
          </RadixCheckbox.Indicator>
        </RadixCheckbox.Root>
        <div className="hd-checkbox__text">
          <label className="hd-checkbox__label" htmlFor={checkboxId}>
            {label}
          </label>
          {hint && (
            <p id={hintId} className="hd-field__hint">
              {hint}
            </p>
          )}
        </div>
      </div>
    );
  },
);

Checkbox.displayName = "Checkbox";
