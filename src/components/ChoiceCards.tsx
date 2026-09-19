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
  required,
  minCardWidth = 150,
}: ChoiceCardsProps<T>) {
  const name = useId();
  const errorId = `${name}-error`;
  return (
    <fieldset
      className="hd-choices"
      aria-describedby={error ? errorId : undefined}
      aria-invalid={error ? true : undefined}
    >
      <legend className="hd-field__label" data-required={required || undefined}>
        {label}
      </legend>
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
              {option.description && (
                <span className="hd-choice__description">{option.description}</span>
              )}
            </span>
          </label>
        ))}
      </div>
      {error && (
        <p id={errorId} className="hd-field__error" role="alert">
          {error}
        </p>
      )}
    </fieldset>
  );
}
