import { forwardRef, useImperativeHandle, useRef, useState } from "react";
import type { ReactNode } from "react";
import { StrengthHint } from "./StrengthHint";
import type { StrengthHintHandle } from "./StrengthHint";
import { TextField } from "./TextField";
import "./components.css";

export interface PassphraseFieldHandle {
  /** The typed passphrase, read once on submit. */
  read(): string;
  /** Empties the field and hides the passphrase again. */
  reset(): void;
  focus(): void;
}

export interface PassphraseFieldProps {
  label: string;
  /** `current-password` to open, `new-password` to set one. */
  autoComplete: "current-password" | "new-password";
  /** Shows the strength hint below the field (FR-003). */
  strength?: boolean;
  error?: string;
  hint?: ReactNode;
  disabled?: boolean;
  autoFocus?: boolean;
  required?: boolean;
  id?: string;
  fieldClassName?: string;
  /** Called on each keystroke, without the value, e.g. to clear an error. */
  onInput?: () => void;
}

/** The one input for typing a passphrase (contracts/ui-databases.md §0,
 * FR-007). It is uncontrolled: the value lives only in the input, is read
 * through the handle on submit and reset straight after, and is never put
 * in React state, context or any store. */
export const PassphraseField = forwardRef<PassphraseFieldHandle, PassphraseFieldProps>(
  (
    {
      label,
      autoComplete,
      strength = false,
      error,
      hint,
      disabled,
      autoFocus,
      required,
      id,
      fieldClassName,
      onInput,
    },
    ref,
  ) => {
    const inputRef = useRef<HTMLInputElement>(null);
    const strengthRef = useRef<StrengthHintHandle>(null);
    const [shown, setShown] = useState(false);

    useImperativeHandle(
      ref,
      () => ({
        read: () => inputRef.current?.value ?? "",
        reset: () => {
          if (inputRef.current) inputRef.current.value = "";
          setShown(false);
          strengthRef.current?.update();
        },
        focus: () => inputRef.current?.focus(),
      }),
      [],
    );

    return (
      <div className={["hd-passphrase", fieldClassName].filter(Boolean).join(" ")}>
        <TextField
          ref={inputRef}
          id={id}
          label={label}
          type={shown ? "text" : "password"}
          autoComplete={autoComplete}
          spellCheck={false}
          autoCapitalize="off"
          autoCorrect="off"
          autoFocus={autoFocus}
          required={required}
          disabled={disabled}
          error={error}
          hint={hint}
          onInput={() => {
            strengthRef.current?.update();
            onInput?.();
          }}
          trailing={
            <button
              type="button"
              className="hd-input__text-action"
              aria-pressed={shown}
              aria-label="Show passphrase"
              disabled={disabled}
              onClick={() => setShown((value) => !value)}
            >
              Show
            </button>
          }
        />
        {strength && <StrengthHint ref={strengthRef} inputRef={inputRef} />}
      </div>
    );
  },
);

PassphraseField.displayName = "PassphraseField";
