import { useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { Button, ConfirmDialog, Dialog, ProgressBar } from "../../components";
import { formatBytes } from "../../lib/bytes";
import { withIdlePaused } from "../session/useIdleActivity";
import * as databasesService from "./databasesService";
import type { BackupProblem } from "./backupProblem";
import type { UpgradeProgress } from "./types";
import { outOfReachText } from "./outOfReach";

// The upgrade question, its progress and the choice of another folder for
// its backup: the dialogs of 009 contracts/ui.md §1-§3.

/** How long an upgrade runs before a short backup's progress is shown anyway
 * (009 contracts/ui.md §2; 003 SC-005). */
const SHOW_AFTER_MS = 1000;

export interface UpgradeQuestionProps {
  /** Whether the question is up. */
  open: boolean;
  /** The database's name, without quotes. */
  name: string;
  /** The version of this build. */
  version: string;
  /** Where the backup goes unless another folder is chosen. */
  backupFolder: string;
  onUpgrade: () => void;
  /** **Cancel**, Escape or closing the dialog: nothing has been written. */
  onCancel: () => void;
}

/** "Upgrade “<name>” for HoploDex <version>?" (009 contracts/ui.md §1, FR-010):
 * a plain question with **Upgrade** focused, so Enter upgrades. */
export function UpgradeQuestion({
  open,
  name,
  version,
  backupFolder,
  onUpgrade,
  onCancel,
}: UpgradeQuestionProps) {
  const quoted = `“${name}”`;
  // The dialog closes itself after Upgrade, which isn't a cancel.
  const confirmed = useRef(false);
  return (
    <ConfirmDialog
      open={open}
      onOpenChange={(next) => {
        if (next) return;
        if (confirmed.current) confirmed.current = false;
        else onCancel();
      }}
      title={`Upgrade ${quoted} for HoploDex ${version}?`}
      description={`${quoted} was made by an earlier version of HoploDex. To open it, this version needs to upgrade it. First, HoploDex saves a backup of it as it is now, in ${backupFolder}. After the upgrade, earlier versions of HoploDex can open that backup, but not ${quoted} itself.`}
      confirmLabel="Upgrade"
      destructive={false}
      onConfirm={() => {
        confirmed.current = true;
        onUpgrade();
      }}
    />
  );
}

export interface UpgradeProgressDialogProps {
  /** The database's name, without quotes. */
  name: string;
  /** **Cancel** while the backup is made. */
  onCancel: () => void;
}

/** "Upgrading “<name>”" (009 contracts/ui.md §2, FR-016). Mounted when
 * **Upgrade** is chosen: it shows at once when the backup's first progress
 * says it will take a while, and otherwise only once a second has passed.
 * The backup can be stopped; the upgrade itself can't, so its phase has no
 * buttons, and neither phase can be dismissed. */
export function UpgradeProgressDialog({ name, onCancel }: UpgradeProgressDialogProps) {
  const [progress, setProgress] = useState<UpgradeProgress | null>(null);
  const [forced, setForced] = useState(false);
  const [late, setLate] = useState(false);
  const [stopping, setStopping] = useState(false);

  useEffect(
    () =>
      databasesService.onUpgradeProgress((heard) => {
        setProgress(heard);
        if (heard.showNow) setForced(true);
      }),
    [],
  );
  useEffect(() => {
    const timer = setTimeout(() => setLate(true), SHOW_AFTER_MS);
    return () => clearTimeout(timer);
  }, []);

  const quoted = `“${name}”`;
  const upgrading = progress !== null && progress.phase === "upgrading" ? progress : null;
  const line = upgrading
    ? `Upgrading ${quoted}. This can't be stopped once it has started.`
    : stopping
      ? "Stopping…"
      : `Saving a backup of ${quoted}…`;

  return (
    <Dialog
      open={forced || late}
      onOpenChange={() => {}}
      title={`Upgrading ${quoted}`}
      size="sm"
      dismissible={false}
      footer={
        upgrading ? undefined : (
          <Button
            variant="secondary"
            disabled={stopping}
            onClick={() => {
              setStopping(true);
              onCancel();
            }}
          >
            Cancel
          </Button>
        )
      }
    >
      <div className="hd-form-section">
        {/* One live region whose text changes, so a screen reader hears the
            change from saving the backup to upgrading. */}
        <p aria-live="polite">{line}</p>
        {upgrading ? (
          // A step count only when there is more than one step.
          <ProgressBar
            value={upgrading.total > 1 ? upgrading : null}
            label="Upgrade progress"
            unit="steps"
            caption={upgrading.total > 1 ? undefined : ""}
          />
        ) : (
          <ProgressBar value={progress} label="Backup progress" formatAmount={formatBytes} />
        )}
      </div>
    </Dialog>
  );
}

/** What follows "…in <folder>" in the dialog's description: a colon and the
 * reason, or, for a folder out of this copy's reach, a sentence that names the
 * folder and gives the override command (011 T100, 011 FR-004). */
function reasonText(problem: BackupProblem): string {
  switch (problem.reason) {
    case "missing":
      return ": the folder isn't there; it may be on a drive that isn't connected.";
    case "notWritable":
      return ": HoploDex isn't allowed to save files there.";
    case "insufficientSpace":
      return problem.bytesNeeded !== undefined && problem.bytesAvailable !== undefined
        ? `: there isn't enough free space (it needs ${formatBytes(problem.bytesNeeded)}; ${formatBytes(problem.bytesAvailable)} is free).`
        : ": there isn't enough free space.";
    case "outOfReach":
      return `. ${outOfReachText("folder", problem.folder)}`;
    case "failed":
      return ": saving it failed part way.";
  }
}

export interface UpgradeFolderDialogProps {
  /** Why the backup couldn't be saved. The dialog is up while it is mounted. */
  problem: BackupProblem;
  /** The database's name, without quotes. */
  name: string;
  /** A folder was chosen for this one backup. */
  onChoose: (folder: string) => void;
  /** **Cancel**, Escape or closing the dialog. */
  onCancel: () => void;
}

/** "Save the backup somewhere else?" (009 contracts/ui.md §3, FR-012), with
 * the same folder picker the settings dialog uses. Dismissing the picker
 * leaves the dialog up. */
export function UpgradeFolderDialog({
  problem,
  name,
  onChoose,
  onCancel,
}: UpgradeFolderDialogProps) {
  const [choosing, setChoosing] = useState(false);
  const quoted = `“${name}”`;

  async function choose(event: FormEvent) {
    event.preventDefault();
    if (choosing) return;
    setChoosing(true);
    try {
      const chosen = await withIdlePaused(() =>
        openFileDialog({
          directory: true,
          multiple: false,
          title: `Choose a folder for the backup of ${name}`,
        }),
      );
      if (typeof chosen === "string") onChoose(chosen);
    } finally {
      setChoosing(false);
    }
  }

  return (
    <Dialog
      open
      onOpenChange={(open) => !open && onCancel()}
      title="Save the backup somewhere else?"
      description={`HoploDex couldn't save the backup it makes before upgrading ${quoted} in ${problem.folder}${reasonText(problem)} You can save this one backup in a folder you choose instead, or cancel. ${quoted} isn't upgraded without a backup.`}
      bare
    >
      <form className="hd-dialog__form" onSubmit={(event) => void choose(event)} noValidate>
        <footer className="hd-dialog__footer">
          <Button variant="secondary" onClick={onCancel}>
            Cancel
          </Button>
          <Button type="submit" variant="primary" autoFocus pending={choosing}>
            Choose a folder…
          </Button>
        </footer>
      </form>
    </Dialog>
  );
}
