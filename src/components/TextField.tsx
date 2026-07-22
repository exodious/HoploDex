import { forwardRef, useId } from "react";
import type { InputHTMLAttributes } from "react";
import "./components.css";

export interface TextFieldProps extends InputHTMLAttributes<HTMLInputElement> {
  label: string;
  error?: string;
  hint?: string;
}

/** Labeled text input with consistent error/hint rendering (WCAG 2.1 AA: label always associated via htmlFor/aria-describedby). */
export const TextField = forwardRef<HTMLInputElement, TextFieldProps>(
  ({ label, error, hint, id, className, ...props }, ref) => {
    const generatedId = useId();
    const inputId = id ?? generatedId;
    const hintId = hint ? `${inputId}-hint` : undefined;
    const errorId = error ? `${inputId}-error` : undefined;
    const describedBy = [hintId, errorId].filter(Boolean).join(" ") || undefined;

    return (
      <div className="hd-field">
        <label className="hd-field__label" htmlFor={inputId}>
          {label}
        </label>
        <input
          ref={ref}
          id={inputId}
          className={["hd-field__input", error && "hd-field__input--error", className]
            .filter(Boolean)
            .join(" ")}
          aria-invalid={error ? true : undefined}
          aria-describedby={describedBy}
          {...props}
        />
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
  },
);

TextField.displayName = "TextField";
