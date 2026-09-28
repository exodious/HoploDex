import { useEffect, useState } from "react";
import type { FormEvent } from "react";
import { Button, ChoiceCards, ConfirmDialog, Dialog, ProgressBar } from "../../components";
import { formatBytes } from "../../lib/bytes";
import { CommandFailure } from "../../services/tauriClient";
import * as databasesService from "./databasesService";
import type {
  BackupSettingsSaved,
  CountProgress,
  ExistingBackupsChoice,
  OldLocationBackups,
} from "./types";
import "../firearms/forms.css";
import "./databases.css";

/** How long a move or deletion not expected to take long runs before its
 * bar shows, as on the closing screen (contracts/ui-databases.md §5). */
const BAR_DELAY_MS = 1000;

export interface ExistingBackupsDialogProps {
  /** `BACKUPS_AT_OLD_LOCATION`'s details; the question is open while set. */
  question: OldLocationBackups | null;
  /** Where the backups would go. */
  newFolder: string;
  /** The open database's name, for the destructive confirmation. */
  name: string;
  /** Sends the backup settings again with the choice. */
  send: (choice: ExistingBackupsChoice) => Promise<BackupSettingsSaved>;
  /** Nothing was saved: the location goes back to the saved one. */
  onCancel: () => void;
  /** The settings were saved and the backups dealt with. */
  onDone: (saved: BackupSettingsSaved) => void;
}

/** "Backups at the old location" (contracts/ui-databases.md §7; FR-026,
 * US3-4a): asked when the backup location changes and backups of the
 * database are at the old one. Moving or deleting them runs here, with its
 * progress, and can't be dismissed meanwhile. */
export function ExistingBackupsDialog({
  question,
  newFolder,
  name,
  send,
  onCancel,
  onDone,
}: ExistingBackupsDialogProps) {
  const [running, setRunning] = useState(false);
  return (
    <Dialog
      open={question !== null}
      onOpenChange={(open) => !open && onCancel()}
      title="Backups at the old location"
      dismissible={!running}
      bare
    >
      {question && (
        <Question
          question={question}
          newFolder={newFolder}
          name={name}
          send={send}
          onRunningChange={setRunning}
          onCancel={onCancel}
          onDone={onDone}
        />
      )}
    </Dialog>
  );
}

/** Progress of a move (bytes) or a deletion (backups). */
type Running =
  | { choice: "move"; progress: CountProgress | null; showNow: boolean }
  | { choice: "delete"; progress: CountProgress | null; showNow: false };

function Question({
  question,
  newFolder,
  name,
  send,
  onRunningChange,
  onCancel,
  onDone,
}: {
  question: OldLocationBackups;
  newFolder: string;
  name: string;
  send: ExistingBackupsDialogProps["send"];
  onRunningChange: (running: boolean) => void;
  onCancel: () => void;
  onDone: (saved: BackupSettingsSaved) => void;
}) {
  const [choice, setChoice] = useState<ExistingBackupsChoice>("move");
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [running, setRunning] = useState<Running | null>(null);
  const [barDue, setBarDue] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);
  const [notDeleted, setNotDeleted] = useState<string[] | null>(null);

  // Listening from the moment the question shows: a listener takes a round
  // trip to register, and the first event says whether to show the bar at
  // once (SC-005).
  useEffect(() => {
    const stopMove = databasesService.onBackupsMoveProgress(({ processed, total, showNow }) =>
      setRunning((current) =>
        current?.choice === "move"
          ? { choice: "move", progress: { processed, total }, showNow }
          : current,
      ),
    );
    const stopDelete = databasesService.onBackupsDeleteProgress((progress) =>
      setRunning((current) => (current?.choice === "delete" ? { ...current, progress } : current)),
    );
    return () => {
      stopMove();
      stopDelete();
    };
  }, []);

  const runningChoice = running?.choice ?? null;
  useEffect(() => {
    setBarDue(false);
    if (runningChoice === null) return;
    const timer = window.setTimeout(() => setBarDue(true), BAR_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [runningChoice]);

  async function run(chosen: ExistingBackupsChoice) {
    setRefusal(null);
    if (chosen !== "leave") {
      setRunning({ choice: chosen, progress: null, showNow: false });
      onRunningChange(true);
    }
    try {
      const saved = await send(chosen);
      onRunningChange(false);
      onDone(saved);
    } catch (error) {
      setRunning(null);
      onRunningChange(false);
      showFailure(error);
    }
  }

  function showFailure(error: unknown) {
    if (!(error instanceof CommandFailure)) {
      setRefusal("The backup location couldn't be changed. Nothing has been changed.");
      return;
    }
    const details = error.details ?? {};
    switch (error.code) {
      case "INSUFFICIENT_SPACE":
        setRefusal(
          `Moving the backups needs ${formatBytes(Number(details.bytesNeeded))} free in ${newFolder}; ${formatBytes(Number(details.bytesAvailable))} is free. Nothing has been changed.`,
        );
        return;
      case "BACKUP_LOCATION_UNAVAILABLE":
        setRefusal(`${newFolder} isn't available. Nothing has been changed.`);
        return;
      case "BACKUPS_NOT_ALL_DELETED":
        setNotDeleted((details.failedPaths as string[] | undefined) ?? []);
        return;
      default:
        setRefusal(error.message);
    }
  }

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    // A dialog opened from inside another form sends its submit up to it.
    event.stopPropagation();
    if (choice === "delete") setConfirmingDelete(true);
    else void run(choice);
  }

  const size = formatBytes(question.totalBytes);
  const count = `${question.count} ${question.count === 1 ? "backup" : "backups"}`;

  if (notDeleted) {
    const one = notDeleted.length === 1;
    return (
      <div className="hd-dialog__form">
        <div className="hd-dialog__body hd-form-section">
          <div className="hd-banner hd-banner--error hd-existing-backups__failed" role="alert">
            <p>
              {one
                ? "1 backup couldn't be deleted, so the backup location wasn't changed. It is still this database's backup:"
                : `${notDeleted.length} backups couldn't be deleted, so the backup location wasn't changed. They are still this database's backups:`}
            </p>
            <ul>
              {notDeleted.map((path) => (
                <li key={path} className="hd-privacy-note__path">
                  {path}
                </li>
              ))}
            </ul>
          </div>
        </div>
        <footer className="hd-dialog__footer">
          <Button variant="primary" onClick={onCancel}>
            Close
          </Button>
        </footer>
      </div>
    );
  }

  if (running) {
    const moving = running.choice === "move";
    const label = moving ? "Moving the backups" : "Deleting the backups";
    const showBar = running.showNow || barDue;
    return (
      <div className="hd-dialog__body hd-form-section" aria-live="polite">
        <p>{label}…</p>
        {showBar && (
          <ProgressBar
            value={running.progress}
            label={label}
            {...(moving ? { formatAmount: formatBytes } : { unit: "backups" })}
          />
        )}
      </div>
    );
  }

  return (
    <form className="hd-dialog__form" onSubmit={handleSubmit} noValidate>
      <div className="hd-dialog__body hd-form-section">
        <p className="hd-existing-backups__about">
          {count} of the database ({size}) are in{" "}
          <span className="hd-privacy-note__path">{question.folder}</span>.
        </p>
        <ChoiceCards<ExistingBackupsChoice>
          label="What should happen to them?"
          value={choice}
          onChange={(chosen) => {
            setChoice(chosen);
            setRefusal(null);
          }}
          minCardWidth={480}
          options={[
            {
              value: "move",
              label: "Move them to the new location",
              description: `They'll be in ${newFolder}.`,
            },
            {
              value: "leave",
              label: "Leave them where they are",
              description: `HoploDex will no longer list, restore or delete them. They still open directly as a database, and choosing ${question.folder} again makes them this database's backups again.`,
            },
            {
              value: "delete",
              label: "Delete them",
              description: "They're deleted securely, as far as this computer allows.",
            },
          ]}
        />
        {refusal && (
          <p className="hd-banner hd-banner--error" role="alert">
            {refusal}
          </p>
        )}
      </div>
      <footer className="hd-dialog__footer">
        <Button variant="secondary" onClick={onCancel}>
          Cancel
        </Button>
        <Button type="submit" variant="primary">
          Change location
        </Button>
      </footer>
      <ConfirmDialog
        open={confirmingDelete}
        onOpenChange={setConfirmingDelete}
        title={`Delete all ${question.count} backups of “${name}”?`}
        description="They are deleted securely where this computer supports it. This can't be undone."
        confirmLabel="Delete all backups"
        onConfirm={() => void run("delete")}
      />
    </form>
  );
}
