import { forwardRef } from "react";
import type { ReactNode, TextareaHTMLAttributes } from "react";
import { FieldFrame } from "./Field";
import { useFieldIds } from "./fieldIds";
import "./components.css";

export interface TextAreaProps extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  label: string;
  error?: string;
  hint?: ReactNode;
  fieldClassName?: string;
}

/** Labeled multi-line text input, for free-form fields like notes. */
export const TextArea = forwardRef<HTMLTextAreaElement, TextAreaProps>(
  ({ label, error, hint, id, className, rows = 3, fieldClassName, ...props }, ref) => {
    const { inputId, hintId, errorId, describedBy } = useFieldIds(id, hint, error);

    return (
      <FieldFrame
        inputId={inputId}
        label={label}
        hint={hint}
        hintId={hintId}
        error={error}
        errorId={errorId}
        className={fieldClassName}
      >
        <textarea
          ref={ref}
          id={inputId}
          rows={rows}
          className={["hd-textarea", error && "hd-input--error", className]
            .filter(Boolean)
            .join(" ")}
          aria-invalid={error ? true : undefined}
          aria-describedby={describedBy}
          {...props}
        />
      </FieldFrame>
    );
  },
);

TextArea.displayName = "TextArea";
