import { useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";
import {
  Button,
  Dialog,
  MIN_PASSPHRASE_CHARS,
  PASSPHRASE_TOO_SHORT,
  PASSPHRASE_UNCHANGED,
  PASSPHRASES_DIFFER,
  PassphraseField,
  ProgressBar,
  usePassphraseChecks,
} from "../../components";
import type { PassphraseFieldHandle } from "../../components";
import { formatBytes } from "../../lib/bytes";
import { CommandFailure } from "../../services/tauriClient";
import { useSession } from "../session/sessionStore";
import * as databasesService from "./databasesService";
import { DatabaseGuideLink } from "./DatabaseGuide";
import type { PassphraseChanged, PassphraseChangeProgress } from "./types";
import "../firearms/forms.css";
import "./databases.css";

const PHASES: Record<PassphraseChangeProgress["phase"], string> = {
  copying: "Making a copy with the new passphrase…",
  checking: "Checking the new copy…",
  replacing: "Replacing the database…",
};

export interface ChangePassphraseDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/** Changes the open database's passphrase (contracts/ui-databases.md §8,
 * FR-015, FR-016). A copy with the new passphrase is made, checked and put
 * in place of the database, which is untouched until then, and the previous
 * file is deleted securely. */
export function ChangePassphraseDialog({ open, onOpenChange }: ChangePassphraseDialogProps) {
  const [running, setRunning] = useState(false);
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="Change passphrase"
      dismissible={!running}
      bare
    >
      {/* Mounted only while open, so every opening starts empty. */}
      <ChangePassphraseForm onRunningChange={setRunning} onDone={() => onOpenChange(false)} />
    </Dialog>
  );
}

type Field = "currentPassphrase" | "newPassphrase" | "confirmation";
type Errors = Partial<Record<Field, string>>;

function ChangePassphraseForm({
  onRunningChange,
  onDone,
}: {
  onRunningChange: (running: boolean) => void;
  onDone: () => void;
}) {
  const session = useSession();
  const [errors, setErrors] = useState<Errors>({});
  const [refusal, setRefusal] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<PassphraseChangeProgress | null>(null);
  const [changed, setChanged] = useState<PassphraseChanged | null>(null);
  // The field to focus once the form shows again: an error can come back
  // while the progress is showing in its place.
  const [focusOn, setFocusOn] = useState<Field | null>(null);
  const currentPassphrase = useRef<PassphraseFieldHandle>(null);
  const newPassphrase = useRef<PassphraseFieldHandle>(null);
  const confirmation = useRef<PassphraseFieldHandle>(null);
  // Checked as they are typed; Change passphrase waits until all is well.
  const checks = usePassphraseChecks({
    current: currentPassphrase,
    passphrase: newPassphrase,
    confirmation,
  });

  /** A keystroke in `field`: whatever the last attempt said about it is
   * out of date. */
  function edited(field: Field) {
    setErrors((current) => (current[field] ? { ...current, [field]: undefined } : current));
    checks.check();
  }

  useEffect(
    () => (running ? databasesService.onPassphraseChangeProgress(setProgress) : undefined),
    [running],
  );

  useEffect(() => {
    if (running || !focusOn) return;
    const fields = { currentPassphrase, newPassphrase, confirmation };
    fields[focusOn].current?.focus();
    setFocusOn(null);
  }, [running, focusOn]);

  function focusFirst(found: Errors) {
    const order: Field[] = ["currentPassphrase", "newPassphrase", "confirmation"];
    setFocusOn(order.find((field) => found[field]) ?? null);
  }

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    // Each passphrase is read once and its field emptied at once, whatever
    // happens next (FR-007).
    const current = currentPassphrase.current?.read() ?? "";
    const next = newPassphrase.current?.read() ?? "";
    const confirmed = confirmation.current?.read() ?? "";
    for (const field of [currentPassphrase, newPassphrase, confirmation]) field.current?.reset();
    checks.clear();

    // The button already waits for all of this; checked again all the same.
    const found: Errors = {};
    if (!current) found.currentPassphrase = "Enter the current passphrase.";
    if ([...next.normalize("NFC")].length < MIN_PASSPHRASE_CHARS) {
      found.newPassphrase = PASSPHRASE_TOO_SHORT;
    } else if (next.normalize("NFC") === current.normalize("NFC")) {
      found.newPassphrase = PASSPHRASE_UNCHANGED;
    } else if (next !== confirmed) {
      found.confirmation = PASSPHRASES_DIFFER;
    }
    setErrors(found);
    setRefusal(null);
    if (Object.keys(found).length > 0) {
      focusFirst(found);
      return;
    }

    setProgress(null);
    setRunning(true);
    onRunningChange(true);
    try {
      const result = await databasesService.changePassphrase(current, next);
      setChanged(result);
      void session.refreshStatus();
    } catch (error) {
      showFailure(error);
    } finally {
      setRunning(false);
      onRunningChange(false);
    }
  }

  function showFailure(error: unknown) {
    if (!(error instanceof CommandFailure)) {
      setRefusal("The passphrase couldn't be changed.");
      return;
    }
    if (error.fieldErrors) {
      const found: Errors = {
        currentPassphrase: error.fieldErrors.currentPassphrase,
        newPassphrase: error.fieldErrors.newPassphrase,
      };
      setErrors(found);
      focusFirst(found);
      if (found.currentPassphrase || found.newPassphrase) return;
    }
    const details = error.details ?? {};
    switch (error.code) {
      case "INSUFFICIENT_SPACE":
        setRefusal(
          `Changing the passphrase needs ${formatBytes(Number(details.bytesNeeded))} free on the database's drive; ${formatBytes(Number(details.bytesAvailable))} is free.`,
        );
        return;
      case "OPERATION_STOPPED":
        setRefusal(
          "The passphrase change was stopped. The database still opens with its current passphrase.",
        );
        return;
      default:
        setRefusal(error.message);
    }
  }

  if (running) {
    const phase = progress?.phase ?? "copying";
    return (
      <div className="hd-dialog__body hd-form-section" aria-live="polite">
        <p>{PHASES[phase]}</p>
        <ProgressBar
          value={progress}
          label="Passphrase change progress"
          formatAmount={formatBytes}
          caption={progress && progress.total === 0 ? "" : undefined}
        />
      </div>
    );
  }

  if (changed) {
    return (
      <>
        <div className="hd-dialog__body hd-form-section" role="status">
          <p>
            {changed.oldFileRemoved ? (
              <>
                The passphrase has been changed. The previous file was deleted securely, as far as
                this computer allows (see <DatabaseGuideLink />
                ). Backups and copies made before now still open with the old passphrase.
              </>
            ) : (
              `The passphrase has been changed. The previous file could not be deleted. It is at ${changed.oldFilePath ?? ""}, and it opens with the old passphrase. Backups and copies made before now still open with the old passphrase too.`
            )}
          </p>
        </div>
        <footer className="hd-dialog__footer">
          <Button variant="primary" onClick={onDone}>
            Done
          </Button>
        </footer>
      </>
    );
  }

  return (
    <form className="hd-dialog__form" onSubmit={handleSubmit} noValidate>
      <div className="hd-dialog__body hd-form-section">
        <PassphraseField
          ref={currentPassphrase}
          label="Current passphrase"
          autoComplete="current-password"
          required
          error={errors.currentPassphrase}
          onInput={() => edited("currentPassphrase")}
          fieldClassName="hd-field--half"
        />
        <div className="hd-form-grid hd-form-grid--2">
          <PassphraseField
            ref={newPassphrase}
            label="New passphrase"
            autoComplete="new-password"
            required
            strength
            error={errors.newPassphrase ?? checks.errors.passphrase}
            onInput={() => edited("newPassphrase")}
          />
          <PassphraseField
            ref={confirmation}
            label="Confirm new passphrase"
            autoComplete="new-password"
            required
            error={errors.confirmation ?? checks.errors.confirmation}
            onInput={() => edited("confirmation")}
            onBlur={checks.check}
          />
        </div>
        {refusal && (
          <p className="hd-banner hd-banner--error" role="alert">
            {refusal}
          </p>
        )}
      </div>
      <footer className="hd-dialog__footer">
        <Button variant="secondary" onClick={onDone}>
          Cancel
        </Button>
        <Button type="submit" variant="primary" disabled={!checks.ready}>
          Change passphrase
        </Button>
      </footer>
    </form>
  );
}
