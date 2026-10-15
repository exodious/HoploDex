import { forwardRef, useCallback, useEffect, useImperativeHandle, useRef, useState } from "react";
import type { RefObject } from "react";
import { estimateStrength, startStrengthWorker } from "./strength/client";
import type { Estimate } from "./strength/estimate";
import "./components.css";

const LABELS = ["Very weak", "Weak", "Fair", "Strong", "Very strong"] as const;
const ADVICE = "Longer is stronger: several unrelated words make a good passphrase.";

export interface StrengthHintHandle {
  /** Re-estimates from the input's current value. */
  update(): void;
}

export interface StrengthHintProps {
  /** The passphrase input. The hint reads it when asked to update and keeps
   * only the estimate, never the value (FR-007). */
  inputRef: RefObject<HTMLInputElement | null>;
}

/** A five-step strength meter with a label and zxcvbn's advice. It only
 * advises: a weak passphrase is never refused for being weak (FR-003). */
export const StrengthHint = forwardRef<StrengthHintHandle, StrengthHintProps>(
  ({ inputRef }, ref) => {
    const [estimate, setEstimate] = useState<Estimate | null>(null);
    // One estimate at a time: what is typed while one is being worked out
    // (zxcvbn may still be loading) is estimated when it comes back, from the
    // input's value then. Nothing here keeps the value.
    const asking = useRef(false);
    const stale = useRef(false);
    const mounted = useRef(true);

    // zxcvbn and its dictionaries are large, so they load only when a
    // strength hint is first shown (research.md §18), in a worker (#102).
    useEffect(() => {
      mounted.current = true;
      startStrengthWorker();
      return () => {
        mounted.current = false;
      };
    }, []);

    const update = useCallback(
      function update() {
        if (asking.current) {
          stale.current = true;
          return;
        }
        const value = inputRef.current?.value ?? "";
        if (!value) {
          setEstimate(null);
          return;
        }
        asking.current = true;
        stale.current = false;
        void estimateStrength(value).then((answer) => {
          asking.current = false;
          if (!mounted.current) return;
          if (stale.current) update();
          else setEstimate(answer);
        });
      },
      [inputRef],
    );

    useImperativeHandle(ref, () => ({ update }), [update]);

    // The advice is always shown. The meter, its label and zxcvbn's
    // suggestion share one row that is in the layout from the start, empty
    // until there is something to say, so a weak passphrase's warning
    // appearing never makes the dialog taller or pushes the fields below it
    // out of view (#41).
    return (
      <div className="hd-strength">
        <p className="hd-field__hint">{ADVICE}</p>
        <div className="hd-strength__estimate" data-testid="strength-estimate">
          {estimate && (
            <>
              <div
                className="hd-strength__meter"
                role="meter"
                aria-label="Passphrase strength"
                aria-valuemin={0}
                aria-valuemax={4}
                aria-valuenow={estimate.score}
                aria-valuetext={LABELS[estimate.score]}
                data-score={estimate.score}
              >
                {LABELS.map((label, step) => (
                  <span
                    key={label}
                    className="hd-strength__step"
                    data-filled={step <= estimate.score || undefined}
                  />
                ))}
              </div>
              <span className="hd-strength__label">{LABELS[estimate.score]}</span>
            </>
          )}
          <p className="hd-field__hint hd-strength__suggestion" data-testid="strength-suggestion">
            {estimate?.suggestion}
          </p>
        </div>
      </div>
    );
  },
);

StrengthHint.displayName = "StrengthHint";
