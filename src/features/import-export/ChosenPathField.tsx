import type { ReactNode } from "react";
import { Button } from "../../components";
import { FieldFrame } from "../../components/Field";
import { useFieldIds } from "../../components/fieldIds";
import "./importExport.css";

export interface ChosenPathFieldProps {
  label: string;
  /** What was picked, or that nothing was. */
  value: ReactNode;
  /** The button that opens the chooser, "Choose a folder…". */
  chooseLabel: string;
  onChoose: () => void;
  hint?: ReactNode;
  error?: string;
  disabled?: boolean;
}

/** A file or folder the user picks in the OS's chooser, which the backend
 * opens and keeps the answer of: the field shows what was picked and has no
 * text box, since a typed path is never taken (code scanning alerts
 * `paths/cwe-73`). The import's files, the export's folder and a new
 * database's folder use it. */
export function ChosenPathField({
  label,
  value,
  chooseLabel,
  onChoose,
  hint,
  error,
  disabled,
}: ChosenPathFieldProps) {
  const { inputId, hintId, errorId, describedBy } = useFieldIds(undefined, hint, error);
  return (
    <FieldFrame
      inputId={inputId}
      label={label}
      required
      hint={hint}
      hintId={hintId}
      error={error}
      errorId={errorId}
      disabled={disabled}
    >
      <div className="hd-io-chosen">
        <output id={inputId} className="hd-io-chosen__value" aria-describedby={describedBy}>
          {value}
        </output>
        <Button size="sm" onClick={onChoose} disabled={disabled} aria-describedby={describedBy}>
          {chooseLabel}
        </Button>
      </div>
    </FieldFrame>
  );
}
