import { useId } from "react";
import * as RadixSelect from "@radix-ui/react-select";
import "./components.css";

export interface SelectOption {
  value: string;
  label: string;
}

export interface SelectProps {
  label: string;
  value: string | undefined;
  onValueChange: (value: string) => void;
  options: SelectOption[];
  placeholder?: string;
  error?: string;
  id?: string;
  disabled?: boolean;
}

/** Labeled, keyboard-navigable select — the only dropdown pattern the app uses. */
export function Select({
  label,
  value,
  onValueChange,
  options,
  placeholder = "Select…",
  error,
  id,
  disabled,
}: SelectProps) {
  const generatedId = useId();
  const selectId = id ?? generatedId;
  const errorId = error ? `${selectId}-error` : undefined;

  return (
    <div className="hd-field">
      <label className="hd-field__label" id={`${selectId}-label`} htmlFor={selectId}>
        {label}
      </label>
      <RadixSelect.Root value={value} onValueChange={onValueChange} disabled={disabled}>
        <RadixSelect.Trigger
          id={selectId}
          className={["hd-select__trigger", error && "hd-field__input--error"]
            .filter(Boolean)
            .join(" ")}
          aria-labelledby={`${selectId}-label`}
          aria-invalid={error ? true : undefined}
          aria-describedby={errorId}
        >
          <RadixSelect.Value placeholder={placeholder} />
          <RadixSelect.Icon className="hd-select__icon">▾</RadixSelect.Icon>
        </RadixSelect.Trigger>
        <RadixSelect.Portal>
          <RadixSelect.Content className="hd-select__content" position="popper" sideOffset={4}>
            <RadixSelect.Viewport>
              {options.map((option) => (
                <RadixSelect.Item
                  key={option.value}
                  value={option.value}
                  className="hd-select__item"
                >
                  <RadixSelect.ItemText>{option.label}</RadixSelect.ItemText>
                  <RadixSelect.ItemIndicator className="hd-select__item-indicator">
                    ✓
                  </RadixSelect.ItemIndicator>
                </RadixSelect.Item>
              ))}
            </RadixSelect.Viewport>
          </RadixSelect.Content>
        </RadixSelect.Portal>
      </RadixSelect.Root>
      {error && (
        <p id={errorId} className="hd-field__error" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
