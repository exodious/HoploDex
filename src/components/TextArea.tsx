import { forwardRef, useState } from "react";
import type { ChangeEvent } from "react";
import type { ReactNode, TextareaHTMLAttributes } from "react";
import { FieldFrame } from "./Field";
import { useFieldIds } from "./fieldIds";
import { countCharacters } from "../lib/textLimits";
import "./components.css";
import "./TextArea.css";

export interface TextAreaProps extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  label: string;
  error?: string;
  hint?: ReactNode;
  fieldClassName?: string;
  /** The most characters the field should hold (specs/008-hardening-batch
   * contracts/ui.md §3). From 90% of it a counter, "<n> of <limit>", shows
   * under the field; over it the counter takes the error style. The text is
   * never cut off: the form shows the field's error and refuses to submit. */
  limit?: number;
}

/** The counter shows from this share of the limit. */
const COUNTER_FROM = 0.9;

/** Labeled multi-line text input, for free-form fields like notes. */
export const TextArea = forwardRef<HTMLTextAreaElement, TextAreaProps>(
  (
    { label, error, hint, id, className, rows = 3, fieldClassName, limit, onChange, ...props },
    ref,
  ) => {
    const {
      inputId,
      hintId,
      errorId,
      describedBy: fieldDescribedBy,
    } = useFieldIds(id, hint, error);
    // An uncontrolled field is counted from what it holds as it changes.
    const [typed, setTyped] = useState(() => String(props.defaultValue ?? ""));
    const text = props.value !== undefined ? String(props.value) : typed;
    const count = limit === undefined ? 0 : countCharacters(text.trim());
    const counterShown = limit !== undefined && count >= limit * COUNTER_FROM;
    const over = limit !== undefined && count > limit;
    const counterId = counterShown ? `${inputId}-count` : undefined;
    const describedBy = [fieldDescribedBy, counterId].filter(Boolean).join(" ") || undefined;
    const handleChange = (event: ChangeEvent<HTMLTextAreaElement>) => {
      if (limit !== undefined && props.value === undefined) setTyped(event.target.value);
      onChange?.(event);
    };

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
          onChange={handleChange}
        />
        {counterShown && (
          <p
            id={counterId}
            aria-live="polite"
            className={["hd-textarea__counter", over && "hd-textarea__counter--over"]
              .filter(Boolean)
              .join(" ")}
          >
            {`${count.toLocaleString("en-US")} of ${(limit ?? 0).toLocaleString("en-US")}`}
          </p>
        )}
      </FieldFrame>
    );
  },
);

TextArea.displayName = "TextArea";
