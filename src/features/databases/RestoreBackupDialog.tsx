import { useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";
import {
  Button,
  ChoiceCards,
  ConfirmDialog,
  Dialog,
  PassphraseField,
  ProgressBar,
} from "../../components";
import type { PassphraseFieldHandle } from "../../components";
import { formatBytes } from "../../lib/bytes";
import { formatDateTime } from "../../lib/dates";
import { CommandFailure } from "../../services/tauriClient";
import { useSession } from "../session/sessionStore";
import * as databasesService from "./databasesService";
import type { BackupInfo, BackupList, RestoreProgress } from "./types";
import "../firearms/forms.css";
import "./databases.css";

const PHASES: Record<RestoreProgress["phase"], string> = {
  copying: "Copying the backup…",
  checking: "Checking the backup…",
  savingCurrent: "Backing up the current database…",
  replacing: "Replacing the database…",
};

export interface RestoreBackupDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** The database being restored. */
  name: string;
  /** For a database that doesn't open (US3-6): its path. Without it, the
   * open database is restored. */
  databasePath?: string;
  /** Opens the backup settings, offered when the backup location stops the
   * restore. */
  onChangeLocation?: () => void;
}

/** Restores a database from one of its backups (contracts/ui-databases.md
 * §9, FR-028). The backup's own passphrase is asked for, since it keeps the
 * one it was made with. The current database is backed up first, or, when
 * it is damaged, kept beside the restored one. */
export function RestoreBackupDialog({
  open,
  onOpenChange,
  name,
  databasePath,
  onChangeLocation,
}: RestoreBackupDialogProps) {
  const [running, setRunning] = useState(false);
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title={`Restore ${name} from a backup`}
      size="lg"
      dismissible={!running}
      bare
    >
      <RestoreForm
        name={name}
        databasePath={databasePath}
        onChangeLocation={onChangeLocation}
        onRunningChange={setRunning}
        onDone={() => onOpenChange(false)}
      />
    </Dialog>
  );
}

function RestoreForm({
  name,
  databasePath,
  onChangeLocation,
  onRunningChange,
  onDone,
}: {
  name: string;
  databasePath?: string;
  onChangeLocation?: () => void;
  onRunningChange: (running: boolean) => void;
  onDone: () => void;
}) {
  const session = useSession();
  const [list, setList] = useState<BackupList | null>(null);
  const [selected, setSelected] = useState<string>("");
  const [confirming, setConfirming] = useState(false);
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<RestoreProgress | null>(null);
  const [fieldError, setFieldError] = useState<string | undefined>();
  const [refusal, setRefusal] = useState<{ text: string; changeLocation: boolean } | null>(null);
  const passphrase = useRef<PassphraseFieldHandle>(null);

  useEffect(() => {
    let current = true;
    databasesService.listBackups(databasePath).then(
      (listed) => {
        if (!current) return;
        setList(listed);
        setSelected(listed.backups[0]?.path ?? "");
      },
      () => current && setList({ folder: "", available: false, backups: [] }),
    );
    return () => {
      current = false;
    };
  }, [databasePath]);

  useEffect(
    () => (running ? databasesService.onRestoreProgress(setProgress) : undefined),
    [running],
  );

  const chosen: BackupInfo | undefined = list?.backups.find((b) => b.path === selected);
  const when = chosen ? formatDateTime(chosen.madeAt) : "";

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    if (!chosen) return;
    // Only checked here; it is read for the restore after the confirmation.
    if (!passphrase.current?.read()) {
      setFieldError("Enter the passphrase of this backup.");
      passphrase.current?.focus();
      return;
    }
    setFieldError(undefined);
    setRefusal(null);
    setConfirming(true);
  }

  async function restore() {
    if (!chosen) return;
    // Read once and emptied at once (FR-007).
    const typed = passphrase.current?.read() ?? "";
    passphrase.current?.reset();
    setRunning(true);
    onRunningChange(true);
    setProgress(null);
    try {
      await session.restoreBackup(chosen.path, typed, databasePath);
      onRunningChange(false);
      onDone();
    } catch (error) {
      setRunning(false);
      onRunningChange(false);
      showFailure(error);
    }
  }

  function showFailure(error: unknown) {
    if (!(error instanceof CommandFailure)) {
      setRefusal({
        text: "The backup couldn't be restored. Nothing has been changed.",
        changeLocation: false,
      });
      return;
    }
    const details = error.details ?? {};
    switch (error.code) {
      case "PASSPHRASE_INCORRECT":
        setFieldError(`That passphrase didn't open the backup from ${when}.`);
        return;
      case "INSUFFICIENT_SPACE":
        setRefusal({
          text: `Restoring needs ${formatBytes(Number(details.bytesNeeded))} free on ${String(details.path)}; ${formatBytes(Number(details.bytesAvailable))} is free. Nothing has been changed.`,
          changeLocation: false,
        });
        return;
      case "BACKUP_LOCATION_UNAVAILABLE": {
        const why =
          details.reason === "insufficientSpace"
            ? "there is not enough space there"
            : "the backup location is not available";
        setRefusal({
          text: `The current ${name} can't be backed up first, because ${why}. Nothing has been changed.`,
          changeLocation: true,
        });
        return;
      }
      case "RESTORE_CANCELLED":
        setRefusal({
          text: `The current ${name} couldn't be backed up, so the restore was cancelled. Nothing has been changed.`,
          changeLocation: false,
        });
        return;
      case "DATABASE_DAMAGED":
        setRefusal({
          text: `The backup from ${when} is damaged and can't be restored. Nothing has been changed.`,
          changeLocation: false,
        });
        return;
      case "OPERATION_STOPPED":
        setRefusal({
          text: "The restore was stopped. Nothing has been changed.",
          changeLocation: false,
        });
        return;
      default:
        setRefusal({ text: error.message, changeLocation: false });
    }
  }

  if (running) {
    const phase = progress?.phase ?? "copying";
    return (
      <div className="hd-dialog__body hd-form-section" aria-live="polite">
        <p>{PHASES[phase]}</p>
        <ProgressBar
          value={progress}
          label="Restore progress"
          formatAmount={formatBytes}
          caption={progress && progress.total === 0 ? "" : undefined}
        />
      </div>
    );
  }

  const backups = list?.backups ?? [];
  return (
    <form className="hd-dialog__form" onSubmit={handleSubmit} noValidate>
      <div className="hd-dialog__body hd-form-section">
        {list === null ? (
          <p className="hd-field__hint">Looking for backups…</p>
        ) : !list.available ? (
          <p className="hd-banner">
            The backup folder {list.folder} isn't available on this computer.
          </p>
        ) : backups.length === 0 ? (
          <p className="hd-banner">
            There are no backups of {name} in {list.folder}.
          </p>
        ) : (
          <ChoiceCards
            label="Choose a backup"
            value={selected}
            onChange={setSelected}
            minCardWidth={480}
            options={backups.map((backup) => ({
              value: backup.path,
              label: `${formatDateTime(backup.madeAt)} — ${formatBytes(backup.sizeBytes)}`,
            }))}
          />
        )}
        {chosen && (
          <>
            <PassphraseField
              ref={passphrase}
              label="Passphrase for this backup"
              autoComplete="current-password"
              hint={`Enter the passphrase ${name} had on ${when}. After restoring, ${name} will open with that passphrase.`}
              error={fieldError}
            />
            <p className="hd-restore__statement">
              {databasePath
                ? "The damaged file will be kept next to it, renamed."
                : `The current ${name} is backed up first, so you can undo this by restoring that backup.`}
            </p>
          </>
        )}
        {refusal && (
          <div className="hd-banner hd-banner--error hd-restore__refusal" role="alert">
            <p>{refusal.text}</p>
            {refusal.changeLocation && onChangeLocation && (
              <Button size="sm" onClick={onChangeLocation}>
                Change backup location…
              </Button>
            )}
          </div>
        )}
      </div>
      <footer className="hd-dialog__footer">
        <Button variant="secondary" onClick={onDone}>
          Cancel
        </Button>
        <Button type="submit" variant="danger" disabled={!chosen}>
          Restore
        </Button>
      </footer>
      <ConfirmDialog
        open={confirming}
        onOpenChange={setConfirming}
        title={`Replace ${name} with the backup from ${when}?`}
        description={
          databasePath
            ? `${name} is replaced by the backup, and the damaged file is kept beside it.`
            : `${name} is replaced by the backup, after the current ${name} is backed up.`
        }
        confirmLabel="Restore"
        onConfirm={() => void restore()}
      />
    </form>
  );
}
