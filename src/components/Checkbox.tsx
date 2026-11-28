import { forwardRef, useId } from "react";
import type { ReactNode } from "react";
import * as RadixCheckbox from "@radix-ui/react-checkbox";
import { ChangedMark } from "./ChangedMark";
import type { FieldChange } from "./fieldChanges";
import { Icon } from "./Icon";
import "./components.css";

export interface CheckboxProps {
  label: string;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  hint?: ReactNode;
  id?: string;
  disabled?: boolean;
  /** Set while it differs from what is saved: the marker and revert (014 FR-009). */
  change?: FieldChange;
}

/** Accessible checkbox with a fully clickable, always-visible label. */
export const Checkbox = forwardRef<HTMLButtonElement, CheckboxProps>(
  ({ label, checked, onCheckedChange, hint, id, disabled, change }, ref) => {
    const generatedId = useId();
    const checkboxId = id ?? generatedId;
    const hintId = hint ? `${checkboxId}-hint` : undefined;
    const changedId = change ? `${checkboxId}-changed` : undefined;

    return (
      <div className="hd-checkbox" data-disabled={disabled || undefined}>
        <RadixCheckbox.Root
          ref={ref}
          id={checkboxId}
          className="hd-checkbox__box"
          checked={checked}
          onCheckedChange={(state) => onCheckedChange(state === true)}
          disabled={disabled}
          aria-describedby={[changedId, hintId].filter(Boolean).join(" ") || undefined}
        >
          <RadixCheckbox.Indicator className="hd-checkbox__indicator">
            <Icon name="check" size={14} strokeWidth={2.4} />
          </RadixCheckbox.Indicator>
        </RadixCheckbox.Root>
        <div className="hd-checkbox__text">
          <span className="hd-field__label-row">
            <label className="hd-checkbox__label" htmlFor={checkboxId}>
              {label}
            </label>
            {change && changedId && (
              <ChangedMark
                label={label}
                change={change}
                id={changedId}
                focusTarget={() => document.getElementById(checkboxId)}
              />
            )}
          </span>
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
