import { useEffect, useMemo, useRef, useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { flushSync } from "react-dom";
import { Button } from "../../components";
import { CLEAR_PASSPHRASE_FIELDS } from "../../components/PassphraseField";
import { CommandFailure, listen } from "../../services/tauriClient";
import { BrandMark } from "../app/BrandMark";
import { ThemeToggle } from "../app/ThemeToggle";
import { useSession } from "../session/sessionStore";
import { withIdlePaused } from "../session/useIdleActivity";
import { CreateDatabaseDialog } from "./CreateDatabaseDialog";
import * as databasesService from "./databasesService";
import { backupProblemOf } from "./backupProblem";
import type { BackupProblem } from "./backupProblem";
import { databaseNameOf } from "./paths";
import { RecentDatabaseRow } from "./RecentDatabaseRow";
import { CataloguePlate } from "./plate/CataloguePlate";
import { RestoreBackupDialog } from "./RestoreBackupDialog";
import type { ChooserRow, OpenElsewhere } from "./RecentDatabaseRow";
import { TakeOverConfirm } from "./TakeOverConfirm";
import type {
  ChooserNotice,
  ChooserState,
  OperationKind,
  UpgradeFailedDetails,
  UpgradeNeededDetails,
} from "./types";
import { UpgradeFolderDialog, UpgradeProgressDialog, UpgradeQuestion } from "./UpgradeDialogs";
import { BACKUP_OUT_OF_REACH_REASON } from "./outOfReach";
import "../app/AppShell.css";
import "./databases.css";

const WELCOME =
  "HoploDex keeps your collection in an encrypted database file that only your passphrase opens.";

/** The native open dialog's filter: databases and backups, or any file for
 * one that was renamed (003 research.md §19). */
const DATABASE_FILTERS = [
  { name: "HoploDex databases", extensions: ["hoplodex"] },
  { name: "All files", extensions: ["*"] },
];

/** What a failed open says, in the selected row (003 contracts/ui-databases.md
 * §1 "Open failures"). `DATABASE_NOT_FOUND` and `DATABASE_OPEN_ELSEWHERE`
 * are shown by the row itself. A sentence names a database in quotes. */
function openFailureText(databaseName: string, error: unknown): string {
  const name = `“${databaseName}”`;
  if (!(error instanceof CommandFailure)) return `HoploDex couldn't open ${name}.`;
  switch (error.code) {
    case "PASSPHRASE_INCORRECT":
      if (savedPassphraseFailed(error))
        return `The saved passphrase no longer opens ${name}. Enter its passphrase; the saved copy will be updated.`;
      return `That passphrase didn't open ${name}. Either the passphrase is wrong, or the file isn't a HoploDex database or is damaged.`;
    case "DATABASE_IN_USE":
      return `${name} is open in another copy of HoploDex, on this computer or another one. Close it there first.`;
    case "DATABASE_NEWER_VERSION":
      return `${name} was last used by a newer version of HoploDex, which upgraded it. Update HoploDex to open it. The file has not been changed. A backup saved before that upgrade opens in this version: look for one whose name ends in “before” and a version number, in ${newerVersionFolder(error)}.`;
    case "UPGRADE_FAILED": {
      const { backupPath } = (error.details ?? {}) as Partial<UpgradeFailedDetails>;
      return `HoploDex couldn't upgrade ${name}. It is as it was before, and hasn't been opened. The backup saved before the upgrade is ${backupPath ?? "not known"}.`;
    }
    case "DATABASE_DAMAGED":
      return `${name} is damaged and can't be opened.`;
    case "DISK_FULL": {
      const full = `The disk is full, so HoploDex couldn't open ${name}. Free some space on the drive that holds it, then try again. The file has not been changed.`;
      // A full disk during the upgrade names the backup made before it (009
      // FR-015); one while opening has no backup to name.
      const { backupPath } = (error.details ?? {}) as Partial<UpgradeFailedDetails>;
      return typeof backupPath === "string" && backupPath
        ? `${full} The upgrade didn't happen; the backup made before it is at ${backupPath}.`
        : full;
    }
    case "DATABASE_UNREADABLE":
      return `HoploDex can't read ${name}: it doesn't have permission to open the file, or the drive or network holding it isn't available. The file has not been changed.`;
    default:
      return error.message;
  }
}

/** Where `DATABASE_NEWER_VERSION` says to look for the backup made before the
 * newer version's upgrade (009 FR-018, contracts/ui.md §8). */
function newerVersionFolder(error: CommandFailure): string {
  const folder = error.details?.backupFolder;
  return typeof folder === "string" && folder
    ? folder
    : "the “HoploDex backups” folder next to the database, or the backup location you chose for it";
}

const BACKUP_FAILURE: Record<Extract<ChooserNotice, { kind: "backupFailed" }>["reason"], string> = {
  locationUnavailable: "the backup location is not available",
  insufficientSpace:
    "there isn't enough free space on the drive that holds the backups. Free some space on it, or change the backup location",
  interrupted: "the backup was interrupted",
  io: "the backup was interrupted",
  databaseUnreachable: "its file could not be reached",
  outOfReach: BACKUP_OUT_OF_REACH_REASON,
};

/** The passphrase saved on this computer didn't open the database (003 US5-5). */
function savedPassphraseFailed(error: unknown): boolean {
  return (
    error instanceof CommandFailure &&
    error.code === "PASSPHRASE_INCORRECT" &&
    error.details?.savedPassphraseFailed === true
  );
}

/** Whether a failed open offers restoring from a backup: the database is
 * damaged, or the passphrase didn't open it (at page 1 the two look the
 * same), and this computer knows of backups of it (003 FR-028, US3-6). */
function offersRestore(error: unknown): boolean {
  return (
    error instanceof CommandFailure &&
    (error.code === "DATABASE_DAMAGED" || error.code === "PASSPHRASE_INCORRECT") &&
    error.details?.backupsAvailable === true
  );
}

/** What a sleep stopped, as the notice names it (003 FR-037). */
const STOPPED_OPERATION: Record<OperationKind, string> = {
  backup: "a backup",
  passphraseChange: "the passphrase change",
  restore: "a restore",
  import: "an import",
  export: "an export",
  deleteBackups: "the deletion of its backups",
  moveBackups: "the move of its backups",
  upgrade: "an upgrade",
};

/** "<n> are still in <folder>, and HoploDex no longer manages them there." */
function stillIn(count: number, folder: string) {
  const one = count === 1;
  return `${count} ${one ? "is" : "are"} still in ${folder}, and HoploDex no longer manages ${one ? "it" : "them"} there.`;
}

function stoppedText(notice: Extract<ChooserNotice, { kind: "operationStopped" }>, name: string) {
  if (notice.operation === "deleteBackups") {
    return `The computer went to sleep while the backups of ${name} were being deleted, so it was stopped. ${notice.deletedCount ?? 0} were deleted; the rest are still there. You can delete them in its Database settings.`;
  }
  if (notice.operation === "moveBackups") {
    return `The computer went to sleep while the backups of ${name} were being moved, so it was stopped. ${stillIn(notice.leftBehindCount ?? 0, notice.folder ?? "the old backup location")} The new backup location is kept.`;
  }
  const stopped = `The computer went to sleep while ${STOPPED_OPERATION[notice.operation]} was running, so it was stopped.`;
  if (notice.operation === "import") {
    const count = notice.importedCount ?? 0;
    return `${stopped} ${count} ${count === 1 ? "row was" : "rows were"} imported before it stopped. You can import the file again; rows already imported will be found as matches.`;
  }
  return `${stopped} ${name} is as it was before it started.`;
}

/** A notice's sentence (003 contracts/ui-databases.md §1 "Notices"), or `null`
 * for one with nothing to say, such as an ordinary close. */
function noticeText(notice: ChooserNotice): string | null {
  const name = `“${databaseNameOf(notice.databasePath)}”`;
  switch (notice.kind) {
    case "closed":
      if (!["lockedByUser", "idle", "sleep", "screenLocked"].includes(notice.reason)) return null;
      return notice.reason === "idle" && notice.idleMinutes
        ? `HoploDex locked ${name} after ${notice.idleMinutes} ${notice.idleMinutes === 1 ? "minute" : "minutes"} without use.`
        : `HoploDex locked ${name}.`;
    case "takenOver":
      return `${name} was taken over on another computer, so HoploDex stopped saving to it here and closed it.`;
    case "backupFailed":
      return `${name} was not backed up: ${BACKUP_FAILURE[notice.reason]}. Its changes will be backed up at the next close.`;
    case "pendingChangesLost":
      return `Unsaved changes could not be kept when ${name} locked.`;
    case "operationStopped":
      return stoppedText(notice, name);
    case "backupsLeftBehind":
      return `The backups of ${name} were not all moved. ${stillIn(notice.count, notice.folder)}`;
  }
}

/** A notice as the chooser lists it. */
interface ShownNotice {
  id: number;
  text: string;
  name: string;
  changeLocation: boolean;
}

/** The notices worth a line, numbered from `firstId`. */
function shownNotices(notices: ChooserNotice[], firstId: number): ShownNotice[] {
  return notices.flatMap((notice, index) => {
    const text = noticeText(notice);
    // A backup location that let it down can be changed once the database
    // is open again.
    const changeLocation =
      notice.kind === "backupFailed" &&
      (notice.reason === "locationUnavailable" ||
        notice.reason === "insufficientSpace" ||
        notice.reason === "outOfReach");
    const name = databaseNameOf(notice.databasePath);
    return text ? [{ id: firstId + index, text, name, changeLocation }] : [];
  });
}

/** Why the last open of a row failed. */
type Failure =
  | { path: string; kind: "message"; message: string; restore: boolean }
  | { path: string; kind: "notFound" }
  | { path: string; kind: "elsewhere"; elsewhere: OpenElsewhere };

/** A database that waits to be upgraded, from `UPGRADE_NEEDED`, while the row
 * is busy (009 contracts/ui.md §1-§3). */
interface Upgrade {
  row: ChooserRow;
  awaitingId: number;
  version: string;
  /** The backup location, unless the user chose another folder. */
  backupFolder: string;
  /** `question`: asked, nothing written. `running`: the backup is made, then
   * the upgrade. `folder`: the backup couldn't be saved, so another folder is
   * offered. */
  stage: "question" | "running" | "folder";
  /** The folder chosen for this one backup, once there is one. */
  chosenFolder?: string;
  problem?: BackupProblem;
}

export interface DatabaseChooserProps {
  /** The database just closed, selected in place of the backend's choice
   * (003 FR-033). */
  selectPath?: string | null;
}

/** The full-window screen shown whenever no database is open (003 FR-020,
 * FR-021): the known databases, the selected one's passphrase, and the ways
 * to create or find another. It never shows anything from a collection. */
export function DatabaseChooser({ selectPath = null }: DatabaseChooserProps) {
  const session = useSession();
  const [state, setState] = useState<ChooserState | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  /** Files found with "Open another database file…", not yet in the list. */
  const [picked, setPicked] = useState<ChooserRow[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [opening, setOpening] = useState<string | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  const [notices, setNotices] = useState<ShownNotice[]>([]);
  /** Numbers the notices, including those that come later. */
  const noticeCount = useRef(0);
  const [restoring, setRestoring] = useState<ChooserRow | null>(null);
  const [creating, setCreating] = useState(false);
  const [announcement, setAnnouncement] = useState("");
  const [takeOver, setTakeOver] = useState<{ row: ChooserRow; machineName: string } | null>(null);
  /** Rows whose saved passphrase no longer opens them, which ask for it. */
  const [savedFailed, setSavedFailed] = useState<ReadonlySet<string>>(new Set());
  /** The database waiting for its upgrade, if one is (009 FR-010). */
  const [upgrade, setUpgrade] = useState<Upgrade | null>(null);

  useEffect(() => {
    let current = true;
    databasesService.getChooserState().then(
      (loaded) => {
        if (!current) return;
        setState(loaded);
        const wanted = selectPath ?? loaded.selectedPath;
        setSelected(loaded.recent.some((r) => r.path === wanted) ? wanted : loaded.selectedPath);
        setNotices(shownNotices(loaded.notices, noticeCount.current));
        noticeCount.current += loaded.notices.length;
      },
      () => current && setLoadError("HoploDex couldn't read its list of databases."),
    );
    return () => {
      current = false;
    };
  }, [selectPath]);

  // Notices that come once the chooser shows, such as what a sleep stopped
  // (003 FR-037), are added to those already listed.
  useEffect(
    () =>
      databasesService.onChooserNotices(() => {
        databasesService.getChooserState().then(
          (loaded) => {
            setState(loaded);
            const added = shownNotices(loaded.notices, noticeCount.current);
            noticeCount.current += loaded.notices.length;
            setNotices((current) => [...current, ...added]);
          },
          () => {},
        );
      }),
    [],
  );

  const rows = useMemo<ChooserRow[]>(() => {
    const known: ChooserRow[] = (state?.recent ?? []).map(
      ({
        path,
        name,
        available,
        unavailableReason,
        passphraseSaved,
        lastOpenedAt,
        backups,
        changedSinceLeftAt,
      }) => ({
        path,
        name,
        available,
        unavailableReason,
        passphraseSaved,
        details: { lastOpenedAt, backups, changedSinceLeftAt },
      }),
    );
    return [...picked.filter((row) => !known.some((k) => k.path === row.path)), ...known];
  }, [picked, state]);

  function select(path: string) {
    setSelected(path);
    setFailure(null);
  }

  /** Shows why an open, or an upgrade, failed in the row. */
  function showFailure(row: ChooserRow, error: unknown) {
    if (savedPassphraseFailed(error)) {
      setSavedFailed((current) => new Set(current).add(row.path));
    }
    if (error instanceof CommandFailure && error.code === "DATABASE_NOT_FOUND") {
      markUnavailable(row.path);
      setFailure({ path: row.path, kind: "notFound" });
    } else if (error instanceof CommandFailure && error.code === "DATABASE_OPEN_ELSEWHERE") {
      const details = error.details ?? {};
      setFailure({
        path: row.path,
        kind: "elsewhere",
        elsewhere: {
          machineName: String(details.machineName ?? "another computer"),
          since: String(details.since ?? ""),
        },
      });
    } else {
      setFailure({
        path: row.path,
        kind: "message",
        message: openFailureText(row.name, error),
        restore: offersRestore(error),
      });
    }
  }

  /** Opens `row` with the typed passphrase, or the saved one when it is
   * `null`. */
  async function openRow(
    row: ChooserRow,
    passphrase: string | null,
    options: { remember?: boolean; takingOver?: boolean } = {},
  ) {
    setOpening(row.path);
    setFailure(null);
    setAnnouncement(`Opening “${row.name}”…`);
    // The row stays busy while an upgrade is asked about (009 FR-010).
    let waitingForUpgrade = false;
    try {
      // On success the session replaces this screen with the collection.
      if (options.takingOver) await session.openDatabase(row.path, passphrase, { takeOver: true });
      else if (options.remember)
        await session.openDatabase(row.path, passphrase, { rememberPassphrase: true });
      else await session.openDatabase(row.path, passphrase);
    } catch (error) {
      if (error instanceof CommandFailure && error.code === "UPGRADE_NEEDED") {
        const { awaitingId, version, backupFolder } =
          error.details as unknown as UpgradeNeededDetails;
        waitingForUpgrade = true;
        setUpgrade({ row, awaitingId, version, backupFolder, stage: "question" });
      } else {
        showFailure(row, error);
      }
    } finally {
      if (!waitingForUpgrade) setOpening(null);
      setAnnouncement("");
    }
  }

  /** Gives up on the waiting database: the backend drops it, with nothing
   * written, and the chooser is as before, with no message (009 FR-010). */
  function cancelUpgrade() {
    if (!upgrade) return;
    void stopUpgrade(upgrade.awaitingId);
    setUpgrade(null);
    setOpening(null);
  }

  /** Tells the backend to drop the waiting database, or stop its backup. If
   * it has already gone, there is nothing to do. */
  async function stopUpgrade(awaitingId: number) {
    try {
      await databasesService.cancelUpgrade(awaitingId);
    } catch {
      // Gone already, or past the point it can stop.
    }
  }

  /** Backs up, then upgrades, the waiting database, in the backup location or
   * in `folder`, chosen for this one backup (009 FR-011, FR-012). */
  async function startUpgrade(folder?: string) {
    if (!upgrade) return;
    const { row, awaitingId } = upgrade;
    // The progress dialog listens before the request goes out, so the first
    // progress isn't missed.
    flushSync(() =>
      setUpgrade(
        (current) =>
          current && { ...current, stage: "running", chosenFolder: folder, problem: undefined },
      ),
    );
    try {
      // On success the session replaces this screen with the collection.
      if (folder) await session.upgradeDatabase(awaitingId, folder);
      else await session.upgradeDatabase(awaitingId);
    } catch (error) {
      const tried = folder ?? upgrade.backupFolder;
      const problem = backupProblemOf(error, tried);
      if (problem) {
        // The database still waits.
        setUpgrade((current) => current && { ...current, stage: "folder", problem });
        return;
      }
      setUpgrade(null);
      setOpening(null);
      // A backup stopped by Cancel says nothing (009 contracts/ui.md §2).
      if (!(error instanceof CommandFailure && error.code === "OPERATION_STOPPED")) {
        showFailure(row, error);
      }
      return;
    }
    setUpgrade(null);
    setOpening(null);
  }

  // A sleep or a screen lock drops the waiting database, so its question
  // closes as Cancel (009 contracts/ui.md §1).
  const waitingId = upgrade && upgrade.stage !== "running" ? upgrade.awaitingId : null;
  useEffect(() => {
    if (waitingId === null) return;
    return listen<unknown>(CLEAR_PASSPHRASE_FIELDS, () => {
      void stopUpgrade(waitingId);
      setUpgrade(null);
      setOpening(null);
    });
  }, [waitingId]);

  function markUnavailable(path: string) {
    setState(
      (current) =>
        current && {
          ...current,
          recent: current.recent.map((r) => (r.path === path ? { ...r, available: false } : r)),
        },
    );
    setPicked((current) => current.map((r) => (r.path === path ? { ...r, available: false } : r)));
  }

  function inRecentList(path: string): boolean {
    return state?.recent.some((r) => r.path === path) ?? false;
  }

  async function removeRow(row: ChooserRow) {
    if (inRecentList(row.path)) await databasesService.removeRecentDatabase(row.path);
    setState(
      (current) =>
        current && { ...current, recent: current.recent.filter((r) => r.path !== row.path) },
    );
    setPicked((current) => current.filter((r) => r.path !== row.path));
    if (selected === row.path) setSelected(null);
    setFailure(null);
  }

  async function locateRow(row: ChooserRow) {
    const chosen = await withIdlePaused(() =>
      openFileDialog({
        multiple: false,
        directory: false,
        title: `Locate ${row.name}`,
        filters: DATABASE_FILTERS,
      }),
    );
    if (typeof chosen !== "string") return;
    if (inRecentList(row.path)) {
      const located = await databasesService.locateDatabase(row.path, chosen);
      setState(
        (current) =>
          current && {
            ...current,
            recent: current.recent
              .filter((r) => r.path === row.path || r.path !== located.path)
              .map((r) => (r.path === row.path ? located : r)),
          },
      );
    } else {
      setPicked((current) =>
        current.map((r) =>
          r.path === row.path
            ? {
                path: chosen,
                name: databaseNameOf(chosen),
                available: true,
                passphraseSaved: false,
              }
            : r,
        ),
      );
    }
    select(chosen);
  }

  async function openAnotherFile() {
    const chosen = await withIdlePaused(() =>
      openFileDialog({
        multiple: false,
        directory: false,
        title: "Open a database file",
        filters: DATABASE_FILTERS,
      }),
    );
    if (typeof chosen !== "string") return;
    if (!rows.some((row) => row.path === chosen)) {
      setPicked((current) => [
        { path: chosen, name: databaseNameOf(chosen), available: true, passphraseSaved: false },
        ...current,
      ]);
    }
    select(chosen);
  }

  function confirmTakeOver(passphrase: string) {
    if (takeOver) void openRow(takeOver.row, passphrase, { takingOver: true });
  }

  const firstRun = state !== null && rows.length === 0;
  const actionClass = firstRun ? "hd-chooser__action--large" : undefined;

  return (
    <div className="hd-chooser">
      <header className="hd-topbar">
        <div className="hd-topbar__inner">
          <div className="hd-brand hd-brand--static">
            <BrandMark />
            <span className="hd-brand__name">HoploDex</span>
          </div>
          <div className="hd-topbar__tools">
            <Button variant="ghost" size="sm" onClick={session.showAbout}>
              About HoploDex
            </Button>
            <ThemeToggle />
          </div>
        </div>
      </header>

      <div className="hd-chooser__stage">
        <div className="hd-chooser__stage-inner">
          <main className="hd-chooser__main">
            {loadError && (
              <p className="hd-banner hd-banner--error" role="alert">
                {loadError}
              </p>
            )}
            <div className="hd-chooser__notices" aria-live="polite">
              {notices.map((notice) => (
                <div key={notice.id} className="hd-banner hd-db-note hd-chooser__notice">
                  <p>{notice.text}</p>
                  {notice.changeLocation && (
                    <Button
                      size="sm"
                      onClick={() => {
                        session.requestSettings();
                        setNotices((current) =>
                          current.map((n) =>
                            n.id === notice.id
                              ? {
                                  ...n,
                                  text: `Its backup settings will open when you open “${n.name}”.`,
                                  changeLocation: false,
                                }
                              : n,
                          ),
                        );
                      }}
                    >
                      Change backup location…
                    </Button>
                  )}
                  <Button
                    variant="ghost"
                    size="sm"
                    icon="close"
                    aria-label="Dismiss"
                    onClick={() =>
                      setNotices((current) => current.filter((n) => n.id !== notice.id))
                    }
                  />
                </div>
              ))}
            </div>
            {state && (
              <>
                <h1 className="hd-chooser__title">
                  {firstRun ? "Welcome to HoploDex" : "Open a database"}
                </h1>
                {firstRun ? (
                  <p className="hd-chooser__welcome">{WELCOME}</p>
                ) : (
                  <ul className="hd-chooser__list" aria-label="Recent databases">
                    {rows.map((row) => {
                      const failed = failure?.path === row.path ? failure : null;
                      return (
                        <RecentDatabaseRow
                          key={row.path}
                          entry={row}
                          selected={row.path === selected}
                          disabled={opening !== null && opening !== row.path}
                          opening={opening === row.path}
                          error={failed?.kind === "message" ? failed.message : undefined}
                          unavailableNote={
                            failed?.kind === "notFound"
                              ? `“${row.name}” is no longer at this location.`
                              : undefined
                          }
                          elsewhere={failed?.kind === "elsewhere" ? failed.elsewhere : undefined}
                          savedFailed={savedFailed.has(row.path)}
                          keyringAvailable={state.keyringAvailable}
                          onSelect={() => select(row.path)}
                          onOpen={(passphrase, remember) =>
                            void openRow(row, passphrase, { remember })
                          }
                          onRemove={() => void removeRow(row)}
                          onLocate={() => void locateRow(row)}
                          onGoBack={() => setFailure(null)}
                          onTakeOver={() =>
                            failed?.kind === "elsewhere" &&
                            setTakeOver({ row, machineName: failed.elsewhere.machineName })
                          }
                          onRestore={
                            failed?.kind === "message" && failed.restore
                              ? () => setRestoring(row)
                              : undefined
                          }
                        />
                      );
                    })}
                  </ul>
                )}
                <div
                  className={
                    firstRun
                      ? "hd-chooser__actions hd-chooser__actions--large"
                      : "hd-chooser__actions"
                  }
                >
                  <Button
                    variant={firstRun ? "primary" : "secondary"}
                    icon="plus"
                    className={actionClass}
                    disabled={opening !== null}
                    onClick={() => setCreating(true)}
                  >
                    Create a new database…
                  </Button>
                  <Button
                    variant="secondary"
                    icon="folder"
                    className={actionClass}
                    disabled={opening !== null}
                    onClick={() => void openAnotherFile()}
                  >
                    Open another database file…
                  </Button>
                </div>
                <CreateDatabaseDialog
                  open={creating}
                  onOpenChange={setCreating}
                  suggested={state.suggested}
                  onCreate={session.createDatabase}
                />
                {restoring && (
                  <RestoreBackupDialog
                    open
                    onOpenChange={(open) => !open && setRestoring(null)}
                    name={restoring.name}
                    databasePath={restoring.path}
                  />
                )}
                {upgrade && (
                  <>
                    <UpgradeQuestion
                      open={upgrade.stage === "question"}
                      name={upgrade.row.name}
                      version={upgrade.version}
                      backupFolder={upgrade.backupFolder}
                      onUpgrade={() => void startUpgrade()}
                      onCancel={cancelUpgrade}
                    />
                    {upgrade.stage === "running" && (
                      <UpgradeProgressDialog
                        name={upgrade.row.name}
                        onCancel={() => void stopUpgrade(upgrade.awaitingId)}
                      />
                    )}
                    {upgrade.stage === "folder" && upgrade.problem && (
                      <UpgradeFolderDialog
                        problem={upgrade.problem}
                        name={upgrade.row.name}
                        onChoose={(folder) => void startUpgrade(folder)}
                        onCancel={cancelUpgrade}
                      />
                    )}
                  </>
                )}
                <TakeOverConfirm
                  target={
                    takeOver && { name: takeOver.row.name, machineName: takeOver.machineName }
                  }
                  onCancel={() => setTakeOver(null)}
                  onConfirm={confirmTakeOver}
                />
              </>
            )}
          </main>
          <CataloguePlate className="hd-chooser__plate" />
        </div>
      </div>

      <p className="hd-sr-only" role="status" aria-live="polite">
        {announcement}
      </p>
    </div>
  );
}
