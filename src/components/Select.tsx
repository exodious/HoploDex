import * as RadixSelect from "@radix-ui/react-select";
import type { ReactNode } from "react";
import { FieldFrame } from "./Field";
import { useFieldIds } from "./fieldIds";
import { Icon } from "./Icon";
import "./components.css";

export interface SelectOption {
  value: string;
  label: string;
  /** Secondary text shown under the label in the open list. */
  detail?: string;
}

export interface SelectProps {
  label: string;
  value: string | undefined;
  onValueChange: (value: string) => void;
  options: SelectOption[];
  placeholder?: string;
  error?: string;
  hint?: ReactNode;
  required?: boolean;
  id?: string;
  disabled?: boolean;
  /** Classes on the field's wrapper, such as a width. */
  fieldClassName?: string;
}

/** Labeled, keyboard-navigable select — the dropdown pattern for option
 * lists too long for a segmented control (e.g. choosing a policy). */
export function Select({
  label,
  value,
  onValueChange,
  options,
  placeholder = "Select…",
  error,
  hint,
  required,
  id,
  disabled,
  fieldClassName,
}: SelectProps) {
  const { inputId, hintId, errorId, describedBy } = useFieldIds(id, hint, error);
  const labelId = `${inputId}-label`;

  return (
    <FieldFrame
      inputId={inputId}
      label={label}
      labelId={labelId}
      required={required}
      hint={hint}
      hintId={hintId}
      error={error}
      errorId={errorId}
      className={fieldClassName}
    >
      <RadixSelect.Root value={value} onValueChange={onValueChange} disabled={disabled}>
        <RadixSelect.Trigger
          id={inputId}
          className={["hd-select__trigger", error && "hd-input--error"].filter(Boolean).join(" ")}
          aria-labelledby={labelId}
          aria-invalid={error ? true : undefined}
          aria-describedby={describedBy}
        >
          <RadixSelect.Value placeholder={placeholder} />
          <RadixSelect.Icon className="hd-select__icon">
            <Icon name="chevronRight" size={16} style={{ transform: "rotate(90deg)" }} />
          </RadixSelect.Icon>
        </RadixSelect.Trigger>
        <RadixSelect.Portal>
          <RadixSelect.Content className="hd-select__content" position="popper" sideOffset={4}>
            <RadixSelect.Viewport className="hd-select__viewport">
              {options.map((option) => (
                <RadixSelect.Item
                  key={option.value}
                  value={option.value}
                  className="hd-select__item"
                >
                  <span className="hd-select__item-text">
                    <RadixSelect.ItemText>{option.label}</RadixSelect.ItemText>
                    {option.detail && (
                      <span className="hd-select__item-detail">{option.detail}</span>
                    )}
                  </span>
                  <RadixSelect.ItemIndicator className="hd-select__item-indicator">
                    <Icon name="check" size={16} />
                  </RadixSelect.ItemIndicator>
                </RadixSelect.Item>
              ))}
            </RadixSelect.Viewport>
          </RadixSelect.Content>
        </RadixSelect.Portal>
      </RadixSelect.Root>
    </FieldFrame>
  );
}
