import { forwardRef, useId } from "react";
import type { TextareaHTMLAttributes } from "react";
import "./components.css";

export interface TextAreaProps extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  label: string;
  error?: string;
  hint?: string;
}

/** Labeled multi-line text input, for free-form fields like notes. */
export const TextArea = forwardRef<HTMLTextAreaElement, TextAreaProps>(
  ({ label, error, hint, id, className, rows = 4, ...props }, ref) => {
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
        <textarea
          ref={ref}
          id={inputId}
          rows={rows}
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

TextArea.displayName = "TextArea";
