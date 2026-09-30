import { useId } from "react";
import type { ReactNode } from "react";
import "./components.css";

export interface ChoiceCard<T extends string> {
  value: T;
  label: string;
  description?: string;
  art?: ReactNode;
}

export interface ChoiceCardsProps<T extends string> {
  label: string;
  value: T | "";
  onChange: (value: T) => void;
  options: ChoiceCard<T>[];
  error?: string;
  /** A note under the cards, in the hint style of the other fields. */
  hint?: string;
  required?: boolean;
  /** Minimum card width; cards wrap onto more rows below it. */
  minCardWidth?: number;
}

/** A radio group of larger cards, for choices that benefit from a picture
 * or a sentence of explanation (firearm type, coverage kind, disposition). */
export function ChoiceCards<T extends string>({
  label,
  value,
  onChange,
  options,
  error,
  hint,
  required,
  minCardWidth = 150,
}: ChoiceCardsProps<T>) {
  const name = useId();
  const labelId = `${name}-label`;
  const errorId = `${name}-error`;
  const hintId = `${name}-hint`;
  // A labelled radiogroup rather than a <fieldset>: WebKitGTK counts a
  // fieldset's legend twice when a dialog first sizes itself, leaving the
  // dialog too tall until something else changes its layout.
  return (
    <div
      role="radiogroup"
      className="hd-choices"
      aria-labelledby={labelId}
      aria-describedby={[error && errorId, hint && hintId].filter(Boolean).join(" ") || undefined}
      aria-invalid={error ? true : undefined}
      aria-required={required || undefined}
    >
      <span id={labelId} className="hd-field__label" data-required={required || undefined}>
        {label}
      </span>
      <div
        className={["hd-choices__grid", error && "hd-choices__grid--error"]
          .filter(Boolean)
          .join(" ")}
        style={{ gridTemplateColumns: `repeat(auto-fit, minmax(${minCardWidth}px, 1fr))` }}
      >
        {options.map((option) => (
          <label key={option.value} className="hd-choice">
            <input
              type="radio"
              name={name}
              value={option.value}
              checked={value === option.value}
              onChange={() => onChange(option.value)}
              required={required}
            />
            <span className="hd-choice__face">
              {option.art && <span className="hd-choice__art">{option.art}</span>}
              <span className="hd-choice__label">{option.label}</span>
              {/* Keeps a space between label and description in the radio's
                  accessible name without relying on the column layout; the
                  flex container drops it visually. */}
              {option.description && " "}
              {option.description && (
                <span className="hd-choice__description">{option.description}</span>
              )}
            </span>
          </label>
        ))}
      </div>
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
