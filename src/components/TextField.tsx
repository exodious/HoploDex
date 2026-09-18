import { forwardRef } from "react";
import type { InputHTMLAttributes, ReactNode } from "react";
import { FieldFrame } from "./Field";
import { useFieldIds } from "./fieldIds";
import "./components.css";

export interface TextFieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "prefix"> {
  label: string;
  error?: string;
  hint?: ReactNode;
  /** Fixed text shown inside the start of the control, e.g. "$". */
  prefix?: string;
  /** A control rendered inside the end of the input, e.g. a picker button. */
  trailing?: ReactNode;
  fieldClassName?: string;
}

/** Labeled text input with consistent error/hint rendering (WCAG 2.1 AA: label always associated via htmlFor/aria-describedby). */
export const TextField = forwardRef<HTMLInputElement, TextFieldProps>(
  (
    { label, error, hint, id, className, prefix, trailing, fieldClassName, required, ...props },
    ref,
  ) => {
    const { inputId, hintId, errorId, describedBy } = useFieldIds(id, hint, error);

    return (
      <FieldFrame
        inputId={inputId}
        label={label}
        required={required}
        hint={hint}
        hintId={hintId}
        error={error}
        errorId={errorId}
        className={fieldClassName}
      >
        <div
          className={[
            "hd-input",
            error && "hd-input--error",
            props.disabled && "hd-input--disabled",
          ]
            .filter(Boolean)
            .join(" ")}
        >
          {prefix && (
            <span className="hd-input__prefix" aria-hidden>
              {prefix}
            </span>
          )}
          <input
            ref={ref}
            id={inputId}
            className={["hd-input__control", className].filter(Boolean).join(" ")}
            aria-invalid={error ? true : undefined}
            aria-describedby={describedBy}
            aria-required={required || undefined}
            {...props}
          />
          {trailing}
        </div>
      </FieldFrame>
    );
  },
);

TextField.displayName = "TextField";
