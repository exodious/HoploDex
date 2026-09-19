import { useId } from "react";
import { Icon } from "./Icon";
import type { IconName } from "./Icon";
import "./components.css";

export interface SegmentedOption<T extends string> {
  value: T;
  label: string;
  icon?: IconName;
  /** Show only the icon; the label stays as the accessible name and tooltip. */
  iconOnly?: boolean;
}

export interface SegmentedControlProps<T extends string> {
  label: string;
  /** Keep the label for screen readers only, when context makes it obvious. */
  hideLabel?: boolean;
  /** "" means nothing is chosen yet. */
  value: T | "";
  onChange: (value: T) => void;
  options: SegmentedOption<T>[];
  size?: "md" | "sm";
}

/** A single-choice radio group rendered as joined buttons — for short,
 * mutually exclusive option sets (group by, list/tiles, conflict choices).
 * Native radios give arrow-key movement and form semantics for free. */
export function SegmentedControl<T extends string>({
  label,
  hideLabel,
  value,
  onChange,
  options,
  size = "md",
}: SegmentedControlProps<T>) {
  const id = useId();
  return (
    <div
      className={["hd-segmented", size === "sm" && "hd-segmented--sm"].filter(Boolean).join(" ")}
      role="radiogroup"
      aria-labelledby={`${id}-label`}
    >
      <span id={`${id}-label`} className={hideLabel ? "hd-sr-only" : "hd-segmented__label"}>
        {label}
      </span>
      <div className="hd-segmented__track">
        {options.map((option) => (
          <label
            key={option.value}
            className="hd-segmented__option"
            title={option.iconOnly ? option.label : undefined}
          >
            <input
              type="radio"
              name={id}
              value={option.value}
              checked={value === option.value}
              onChange={() => onChange(option.value)}
            />
            <span className="hd-segmented__face">
              {option.icon && <Icon name={option.icon} size={16} />}
              {option.iconOnly ? <span className="hd-sr-only">{option.label}</span> : option.label}
            </span>
          </label>
        ))}
      </div>
    </div>
  );
}
