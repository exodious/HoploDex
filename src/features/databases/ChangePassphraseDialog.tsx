import { useEffect, useRef, useState } from "react";
import { Button, Dialog, ProgressBar } from "../../components";
import { formatBytes } from "../../lib/bytes";
import { CommandFailure } from "../../services/tauriClient";
import { useSessionScope } from "../session/sessionScope";
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

/** Changes the open database's passphrase (003 contracts/ui-databases.md §8,
 * FR-015, FR-016). A copy with the new passphrase is made, checked and put
 * in place of the database, which is untouched until then, and the previous
 * file is deleted securely. Both passphrases are asked for in the native
 * dialog, never in the page (amended 2026-11-04): this one shows the change's
 * progress and result once that dialog has closed, and nothing before. */
export function ChangePassphraseDialog({ open, onOpenChange }: ChangePassphraseDialogProps) {
  const session = useSession();
  const scope = useSessionScope();
  const [progress, setProgress] = useState<PassphraseChangeProgress | null>(null);
  const [running, setRunning] = useState(false);
  const [changed, setChanged] = useState<PassphraseChanged | null>(null);
  const [refusal, setRefusal] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);
  // Read through refs, so a new render never starts the change again: one
  // run, one native dialog, per opening (or per Try again).
  const latest = useRef({ onOpenChange, scope, session });
  latest.current = { onOpenChange, scope, session };

  /** The change under way, one per attempt: React may run an effect twice
   * for one opening (StrictMode), and each run must not ask again. */
  const run = useRef<{ attempt: number; change: Promise<PassphraseChanged> } | null>(null);

  useEffect(() => {
    if (!open) return;
    let live = true;
    if (run.current?.attempt !== attempt) {
      setChanged(null);
      setRefusal(null);
      setProgress(null);
      setRunning(true);
      run.current = { attempt, change: databasesService.changePassphrase(latest.current.scope) };
    }
    // The first progress event says the native dialog closed and the copy
    // began.
    const stop = databasesService.onPassphraseChangeProgress((next) => {
      if (live) setProgress(next);
    });
    run.current.change
      .then((result) => {
        if (!live) return;
        setChanged(result);
        void latest.current.session.refreshStatus();
      })
      .catch((error: unknown) => {
        if (!live) return;
        if (error instanceof CommandFailure && error.code === "PASSPHRASE_CANCELLED") {
          latest.current.onOpenChange(false);
          return;
        }
        setRefusal(failureText(error));
      })
      .finally(() => {
        if (live) setRunning(false);
      });
    return () => {
      live = false;
      stop();
    };
    // `attempt` starts it again after a refusal.
  }, [open, attempt]);

  const shown = open && (changed !== null || refusal !== null || progress !== null);
  return (
    <Dialog
      open={shown}
      onOpenChange={onOpenChange}
      title="Change passphrase"
      dismissible={!running}
      bare
    >
      {running ? (
        <div className="hd-dialog__body hd-form-section" aria-live="polite">
          <p>{PHASES[progress?.phase ?? "copying"]}</p>
          <ProgressBar
            value={progress}
            label="Passphrase change progress"
            formatAmount={formatBytes}
            caption={progress && progress.total === 0 ? "" : undefined}
          />
        </div>
      ) : changed ? (
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
            <Button variant="primary" autoFocus onClick={() => onOpenChange(false)}>
              Done
            </Button>
          </footer>
        </>
      ) : (
        <>
          <div className="hd-dialog__body hd-form-section">
            <p className="hd-banner hd-banner--error" role="alert">
              {refusal}
            </p>
          </div>
          <footer className="hd-dialog__footer">
            <Button variant="secondary" onClick={() => onOpenChange(false)}>
              Close
            </Button>
            <Button variant="primary" autoFocus onClick={() => setAttempt((n) => n + 1)}>
              Try again…
            </Button>
          </footer>
        </>
      )}
    </Dialog>
  );
}

/** What stopped a change, which leaves the database opening with its
 * current passphrase. */
function failureText(error: unknown): string {
  if (!(error instanceof CommandFailure)) return "The passphrase couldn't be changed.";
  const details = error.details ?? {};
  switch (error.code) {
    case "INSUFFICIENT_SPACE":
      return `Changing the passphrase needs ${formatBytes(Number(details.bytesNeeded))} free on the database's drive; ${formatBytes(Number(details.bytesAvailable))} is free.`;
    case "OPERATION_STOPPED":
      return "The passphrase change was stopped. The database still opens with its current passphrase.";
    default:
      return error.message;
  }
}
