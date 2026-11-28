import type { ReactNode } from "react";
import { ChangedMark } from "./ChangedMark";
import type { FieldChange } from "./fieldChanges";
import "./components.css";

export interface FieldFrameProps {
  inputId: string;
  label: string;
  /** Rendered as a visible "*" after the label; the control itself should
   * also carry `required`/`aria-required`. */
  required?: boolean;
  hint?: ReactNode;
  hintId?: string;
  error?: string;
  errorId?: string;
  labelId?: string;
  className?: string;
  /** Dims the label and hint along with the control (`--disabled-*` tokens). */
  disabled?: boolean;
  /** Set while the field differs from what is saved (014 FR-006, FR-010). */
  change?: FieldChange;
  /** The "Changed" tag's id, from `useFieldIds`. */
  changedId?: string;
  children: ReactNode;
}

/** The label / control / hint / error stack every form field shares. */
export function FieldFrame({
  inputId,
  label,
  required,
  hint,
  hintId,
  error,
  errorId,
  labelId,
  className,
  disabled,
  change,
  changedId,
  children,
}: FieldFrameProps) {
  const labelElement = (
    <label
      className="hd-field__label"
      htmlFor={inputId}
      id={labelId}
      data-required={required || undefined}
    >
      {label}
    </label>
  );
  return (
    <div
      className={["hd-field", className].filter(Boolean).join(" ")}
      data-disabled={disabled || undefined}
    >
      {change && changedId ? (
        <div className="hd-field__label-row">
          {labelElement}
          <ChangedMark
            label={label}
            change={change}
            id={changedId}
            focusTarget={() => document.getElementById(inputId)}
          />
        </div>
      ) : (
        labelElement
      )}
      {children}
      {hint && (
        <p id={hintId} className="hd-field__hint">
          {hint}
        </p>
      )}
      {error && (
        <p id={errorId} className="hd-field__error" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
