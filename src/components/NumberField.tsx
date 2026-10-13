import type { KeyboardEvent } from "react";
import { Icon } from "./Icon";
import { TextField } from "./TextField";
import type { TextFieldProps } from "./TextField";
import "./components.css";

export interface NumberFieldProps extends Omit<
  TextFieldProps,
  "value" | "onChange" | "prefix" | "trailing" | "inputMode" | "min" | "max" | "step" | "type"
> {
  /** The text being edited, so a form can say what's wrong with it; parse it on submit. */
  value: string;
  onValueChange: (text: string) => void;
  min: number;
  max: number;
  /** What the buttons and the arrow keys add or take away. */
  step?: number;
  /** Fixed text after the number, such as a unit ("backups"). */
  suffix?: string;
}

/** The whole number a field's text holds, or null when it holds something else. */
function wholeNumber(text: string): number | null {
  const trimmed = text.trim();
  return /^[-+]?\d+$/.test(trimmed) ? Number(trimmed) : null;
}

/** A whole number you can type or step with up/down buttons (and ArrowUp / ArrowDown). The buttons
 * are the app's own rather than the browser's `type="number"` spinners, which look and behave
 * differently in WebKitGTK, WebView2 and WKWebView. Stepping stays within `min`..`max`; typed text
 * is left as typed for the form to check. The buttons are out of the tab order, as the ARIA spin
 * button pattern has it: the keyboard steps from the input. */
export function NumberField({
  value,
  onValueChange,
  min,
  max,
  step = 1,
  suffix,
  label,
  disabled,
  ...props
}: NumberFieldProps) {
  const current = wholeNumber(value);

  function stepBy(direction: 1 | -1) {
    // Text that isn't a number starts from the nearest end of the range.
    const next = current === null ? min : Math.min(max, Math.max(min, current + direction * step));
    onValueChange(String(next));
  }

  function handleKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    props.onKeyDown?.(event);
    if (event.defaultPrevented || disabled) return;
    if (event.key === "ArrowUp" || event.key === "ArrowDown") {
      event.preventDefault();
      stepBy(event.key === "ArrowUp" ? 1 : -1);
    }
  }

  const button = (direction: 1 | -1) => (
    <button
      type="button"
      className="hd-stepper__button"
      tabIndex={-1}
      aria-label={`${direction === 1 ? "Increase" : "Decrease"} ${label}`}
      disabled={
        disabled || (current !== null && (direction === 1 ? current >= max : current <= min))
      }
      // Keeps the focus, and the caret, in the input.
      onMouseDown={(event) => event.preventDefault()}
      onClick={() => stepBy(direction)}
    >
      <Icon name={direction === 1 ? "up" : "chevronDown"} size={12} strokeWidth={2} />
    </button>
  );

  return (
    <TextField
      {...props}
      label={label}
      disabled={disabled}
      role="spinbutton"
      inputMode="numeric"
      autoComplete="off"
      aria-valuemin={min}
      aria-valuemax={max}
      aria-valuenow={current !== null && current >= min && current <= max ? current : undefined}
      value={value}
      onChange={(e) => onValueChange(e.target.value)}
      onKeyDown={handleKeyDown}
      trailing={
        <>
          {suffix && <span className="hd-input__suffix">{suffix}</span>}
          <span className="hd-stepper">
            {button(1)}
            {button(-1)}
          </span>
        </>
      }
    />
  );
}
