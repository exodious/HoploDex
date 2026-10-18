import { useContext, useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";
import {
  Button,
  ChoiceCards,
  ConfirmDialog,
  Dialog,
  PassphraseField,
  ProgressBar,
  useShownOnceLoaded,
} from "../../components";
import type { PassphraseFieldHandle } from "../../components";
import { formatBytes } from "../../lib/bytes";
import { formatDateTime } from "../../lib/dates";
import { CommandFailure } from "../../services/tauriClient";
import { SessionScopeContext } from "../session/sessionScope";
import { useSession } from "../session/sessionStore";
import * as databasesService from "./databasesService";
import type { BackupInfo, BackupList, RestoreProgress } from "./types";
import "../firearms/forms.css";
import "./databases.css";

const PHASES: Record<RestoreProgress["phase"], string> = {
  copying: "Copying the backup…",
  checking: "Checking the backup…",
  upgrading: "Upgrading the restored copy…",
  savingCurrent: "Backing up the current database…",
  replacing: "Replacing the database…",
};

export interface RestoreBackupDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** The database being restored. */
  name: string;
  /** For a database that doesn't open (003 US3-6): its path. Without it, the
   * open database is restored. */
  databasePath?: string;
  /** Opens the backup settings, offered when the backup location stops the
   * restore. */
  onChangeLocation?: () => void;
}

/** Restores a database from one of its backups (003 contracts/ui-databases.md
 * §9, FR-028). The backup's own passphrase is asked for, since it keeps the
 * one it was made with. The current database is backed up first, or, when
 * it is damaged, kept beside the restored one. The open database is "the
 * database"; one restored from the chooser is named, in quotes. */
export function RestoreBackupDialog({
  open,
  onOpenChange,
  name,
  databasePath,
  onChangeLocation,
}: RestoreBackupDialogProps) {
  // None from the chooser, where the database restored isn't open (`databasePath`).
  const scope = useContext(SessionScopeContext);
  const [running, setRunning] = useState(false);
  const [list, setList] = useState<BackupList | null>(null);

  // The list is read before the dialog shows, so it opens at its full size
  // rather than opening on "Looking for backups…" and then growing (#116);
  // a slow network folder opens it anyway, showing that it is still looking.
  useEffect(() => {
    let current = true;
    databasesService.listBackups(scope, databasePath).then(
      (listed) => current && setList(listed),
      () => current && setList({ folder: "", available: false, backups: [] }),
    );
    return () => {
      current = false;
    };
  }, [scope, databasePath]);
  const shown = useShownOnceLoaded(open, list !== null);

  return (
    <Dialog
      open={shown}
      onOpenChange={onOpenChange}
      title={databasePath ? `Restore “${name}” from a backup` : "Restore from a backup"}
      size="lg"
      dismissible={!running}
      bare
    >
      <RestoreForm
        list={list}
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
  list,
  name,
  databasePath,
  onChangeLocation,
  onRunningChange,
  onDone,
}: {
  list: BackupList | null;
  name: string;
  databasePath?: string;
  onChangeLocation?: () => void;
  onRunningChange: (running: boolean) => void;
  onDone: () => void;
}) {
  const session = useSession();
  const scope = useContext(SessionScopeContext);
  const [selected, setSelected] = useState<string>(list?.backups[0]?.path ?? "");
  const [confirming, setConfirming] = useState(false);
  // 009 FR-019: the check before the confirmation, and what it found.
  const [checking, setChecking] = useState(false);
  const [upgradeTo, setUpgradeTo] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<RestoreProgress | null>(null);
  const [fieldError, setFieldError] = useState<string | undefined>();
  const [refusal, setRefusal] = useState<{ text: string; changeLocation: boolean } | null>(null);
  const passphrase = useRef<PassphraseFieldHandle>(null);

  // When the list arrives after the dialog opened, the newest is chosen.
  useEffect(() => {
    if (list) setSelected((chosen) => chosen || (list.backups[0]?.path ?? ""));
  }, [list]);

  useEffect(
    () => (running ? databasesService.onRestoreProgress(setProgress) : undefined),
    [running],
  );

  const chosen: BackupInfo | undefined = list?.backups.find((b) => b.path === selected);
  const when = chosen ? formatDateTime(chosen.madeAt) : "";
  const quoted = `“${name}”`;

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    if (!chosen || checking) return;
    const typed = passphrase.current?.read();
    if (!typed) {
      setFieldError("Enter the passphrase of this backup.");
      passphrase.current?.focus();
      return;
    }
    setFieldError(undefined);
    setRefusal(null);
    // The backup is opened and read first, so the confirmation can say
    // whether it will be upgraded (009 contracts/ui.md §6). It is read again
    // for the restore after the confirmation.
    setChecking(true);
    try {
      const check = await databasesService.checkBackup(scope, chosen.path, typed, databasePath);
      setUpgradeTo(check.upgradeTo);
      setConfirming(true);
    } catch (error) {
      showFailure(error);
    } finally {
      setChecking(false);
    }
  }

  async function restore() {
    if (!chosen) return;
    // Read once and emptied at once (003 FR-007).
    const typed = passphrase.current?.read() ?? "";
    passphrase.current?.reset();
    setRunning(true);
    onRunningChange(true);
    setProgress(null);
    try {
      await (upgradeTo
        ? session.restoreBackup(chosen.path, typed, databasePath, true)
        : session.restoreBackup(chosen.path, typed, databasePath));
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
        passphrase.current?.focus();
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
          text: `The database can't be backed up first, because ${why}. Nothing has been changed.`,
          changeLocation: true,
        });
        return;
      }
      case "RESTORE_CANCELLED":
        setRefusal({
          text: "The database couldn't be backed up, so the restore was cancelled. Nothing has been changed.",
          changeLocation: false,
        });
        return;
      case "DATABASE_DAMAGED":
        setRefusal({
          text: `The backup from ${when} is damaged and can't be restored. Nothing has been changed.`,
          changeLocation: false,
        });
        return;
      case "DATABASE_NEWER_VERSION":
        setRefusal({
          text: "This backup was made by a newer version of HoploDex, so this version can't restore it.",
          changeLocation: false,
        });
        return;
      case "UPGRADE_FAILED":
        setRefusal({
          text: "The backup couldn't be upgraded for this version, so nothing was restored. The database is as it was.",
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
    <form className="hd-dialog__form" onSubmit={(event) => void handleSubmit(event)} noValidate>
      <div className="hd-dialog__body hd-form-section">
        {list === null ? (
          <p className="hd-field__hint">Looking for backups…</p>
        ) : !list.available ? (
          <p className="hd-banner">
            The backup folder {list.folder} isn't available on this computer.
          </p>
        ) : backups.length === 0 ? (
          <p className="hd-banner">
            There are no backups of {databasePath ? quoted : "this database"} in {list.folder}.
          </p>
        ) : (
          <ChoiceCards
            label="Choose a backup"
            value={selected}
            onChange={setSelected}
            minCardWidth={480}
            options={backups.map((backup) => ({
              value: backup.path,
              label:
                `${formatDateTime(backup.madeAt)} — ${formatBytes(backup.sizeBytes)}` +
                (backup.beforeUpgradeTo
                  ? ` — before upgrading to HoploDex ${backup.beforeUpgradeTo}`
                  : ""),
            }))}
          />
        )}
        {chosen && (
          <>
            <PassphraseField
              ref={passphrase}
              label="Passphrase for this backup"
              autoComplete="current-password"
              hint={`Enter the passphrase ${databasePath ? quoted : "the database"} had on ${when}. After restoring, it opens with that passphrase.`}
              error={fieldError}
            />
            <p className="hd-restore__statement">
              {databasePath
                ? "The damaged file will be kept next to it, renamed."
                : "Before restoring, the database is backed up as it is now, so you can undo this by restoring that backup."}
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
        <Button type="submit" variant="danger" disabled={!chosen} pending={checking}>
          Restore
        </Button>
      </footer>
      <ConfirmDialog
        open={confirming}
        onOpenChange={setConfirming}
        title={`Replace ${quoted} with the backup from ${when}?`}
        description={
          <>
            {databasePath
              ? "The backup takes its place, and the damaged file is kept beside it."
              : "The backup takes its place, after the database is backed up as it is now."}
            {upgradeTo &&
              ` This backup was made by an earlier version of HoploDex, so it is upgraded for HoploDex ${upgradeTo} as it is restored. The backup itself stays as it is, and earlier versions can still open it.`}
          </>
        }
        confirmLabel="Restore"
        onConfirm={() => void restore()}
      />
    </form>
  );
}
