import { useEffect, useRef, useState } from "react";
import { Button, placeFocus, ProgressBar } from "../../components";
import { formatBytes } from "../../lib/bytes";
import { BrandMark } from "../app/BrandMark";
import type { BackupProgress } from "../databases/types";
import * as sessionService from "./sessionService";
import "../app/AppShell.css";
import "../databases/databases.css";

/** How long a close runs before a short backup's progress is shown anyway
 * (SC-005). */
const SHOW_AFTER_MS = 1000;

export interface ClosingScreenProps {
  /** The database being closed. */
  name: string;
}

/** Shown in place of the app shell while a normal close runs
 * (contracts/ui-databases.md §5, FR-027). A backup the close makes shows its
 * progress at once when it is expected to take over a second, and otherwise
 * only if the close is still running a second after it began. The backup
 * can be skipped; its changes then wait for the next close. */
export function ClosingScreen({ name }: ClosingScreenProps) {
  const [progress, setProgress] = useState<BackupProgress | null>(null);
  const [late, setLate] = useState(false);
  const [skipping, setSkipping] = useState(false);
  const skip = useRef<HTMLButtonElement>(null);

  useEffect(() => sessionService.onBackupProgress(setProgress), []);
  useEffect(() => {
    const timer = setTimeout(() => setLate(true), SHOW_AFTER_MS);
    return () => clearTimeout(timer);
  }, []);

  const backingUp = progress !== null && (progress.showNow || late);
  useEffect(() => {
    if (backingUp) placeFocus(skip.current);
  }, [backingUp]);

  async function skipBackup() {
    setSkipping(true);
    // The close carries on either way; a skip that arrives too late just
    // finds the backup already made.
    await sessionService.skipBackup().catch(() => {});
  }

  return (
    <div className="hd-chooser hd-closing">
      <header className="hd-topbar">
        <div className="hd-topbar__inner">
          <div className="hd-brand hd-brand--static">
            <BrandMark />
            <span className="hd-brand__name">HoploDex</span>
          </div>
        </div>
      </header>
      <main className="hd-closing__main">
        <section className="hd-closing__panel" aria-live="polite">
          <h1 className="hd-closing__title">Closing “{name}”…</h1>
          {backingUp && (
            <div className="hd-closing__backup">
              <p>Backing up “{name}”…</p>
              <ProgressBar value={progress} label="Backup progress" formatAmount={formatBytes} />
              <div className="hd-closing__skip">
                <Button
                  ref={skip}
                  variant="secondary"
                  disabled={skipping}
                  onClick={() => void skipBackup()}
                >
                  {skipping ? "Skipping…" : "Skip this backup"}
                </Button>
                <p className="hd-closing__note">Its changes will be backed up next time.</p>
              </div>
            </div>
          )}
        </section>
      </main>
    </div>
  );
}
