import { forwardRef, useCallback, useEffect, useImperativeHandle, useState } from "react";
import type { RefObject } from "react";
import type { ZxcvbnFactory } from "@zxcvbn-ts/core";
import "./components.css";

const LABELS = ["Very weak", "Weak", "Fair", "Strong", "Very strong"] as const;
const ADVICE = "Longer is stronger: several unrelated words make a good passphrase.";

let estimator: Promise<ZxcvbnFactory> | null = null;

/** zxcvbn and its dictionaries are large, so they load only when a strength
 * hint is first shown (research.md §18). Everything runs here in the page:
 * no keystroke crosses IPC. */
function loadEstimator(): Promise<ZxcvbnFactory> {
  estimator ??= Promise.all([
    import("@zxcvbn-ts/core"),
    import("@zxcvbn-ts/language-common"),
    import("@zxcvbn-ts/language-en"),
  ]).then(
    ([core, common, en]) =>
      new core.ZxcvbnFactory({
        translations: en.translations,
        graphs: common.adjacencyGraphs,
        dictionary: { ...common.dictionary, ...en.dictionary },
      }),
  );
  return estimator;
}

interface Estimate {
  score: number;
  suggestion: string;
}

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
    const [factory, setFactory] = useState<ZxcvbnFactory | null>(null);
    const [estimate, setEstimate] = useState<Estimate | null>(null);

    useEffect(() => {
      let current = true;
      void loadEstimator().then((loaded) => current && setFactory(loaded));
      return () => {
        current = false;
      };
    }, []);

    const update = useCallback(() => {
      const value = inputRef.current?.value ?? "";
      if (!factory || !value) {
        setEstimate(null);
        return;
      }
      const { score, feedback } = factory.check(value);
      setEstimate({ score, suggestion: feedback.warning || feedback.suggestions[0] || "" });
    }, [factory, inputRef]);

    useImperativeHandle(ref, () => ({ update }), [update]);
    // Catch up with anything typed while zxcvbn was loading.
    useEffect(update, [update]);

    return (
      <div className="hd-strength">
        {estimate && (
          <div className="hd-strength__estimate">
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
          </div>
        )}
        {estimate?.suggestion && (
          <p className="hd-field__hint" data-testid="strength-suggestion">
            {estimate.suggestion}
          </p>
        )}
        <p className="hd-field__hint">{ADVICE}</p>
      </div>
    );
  },
);

StrengthHint.displayName = "StrengthHint";
