import type { ReactNode } from "react";
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
  children,
}: FieldFrameProps) {
  return (
    <div
      className={["hd-field", className].filter(Boolean).join(" ")}
      data-disabled={disabled || undefined}
    >
      <label
        className="hd-field__label"
        htmlFor={inputId}
        id={labelId}
        data-required={required || undefined}
      >
        {label}
      </label>
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
