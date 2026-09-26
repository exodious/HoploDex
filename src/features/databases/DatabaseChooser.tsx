import { useEffect, useMemo, useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { Button } from "../../components";
import { CommandFailure } from "../../services/tauriClient";
import { BrandMark } from "../app/BrandMark";
import { ThemeToggle } from "../app/ThemeToggle";
import { useSession } from "../session/sessionStore";
import { CreateDatabaseDialog } from "./CreateDatabaseDialog";
import * as databasesService from "./databasesService";
import { databaseNameOf } from "./paths";
import { RecentDatabaseRow } from "./RecentDatabaseRow";
import type { ChooserRow, OpenElsewhere } from "./RecentDatabaseRow";
import { TakeOverConfirm } from "./TakeOverConfirm";
import type { ChooserNotice, ChooserState } from "./types";
import "../app/AppShell.css";
import "./databases.css";

const WELCOME =
  "HoploDex keeps your collection in an encrypted database file that only your passphrase opens.";

/** The native open dialog's filter: databases and backups, or any file for
 * one that was renamed (research.md §19). */
const DATABASE_FILTERS = [
  { name: "HoploDex databases", extensions: ["hoplodex"] },
  { name: "All files", extensions: ["*"] },
];

/** What a failed open says, in the selected row (contracts/ui-databases.md
 * §1 "Open failures"). `DATABASE_NOT_FOUND` and `DATABASE_OPEN_ELSEWHERE`
 * are shown by the row itself. */
function openFailureText(name: string, error: unknown): string {
  if (!(error instanceof CommandFailure)) return `HoploDex couldn't open ${name}.`;
  switch (error.code) {
    case "PASSPHRASE_INCORRECT":
      return `That passphrase didn't open ${name}. Either the passphrase is wrong, or the file isn't a HoploDex database or is damaged.`;
    case "DATABASE_IN_USE":
      return `${name} is open in another copy of HoploDex, on this computer or another one. Close it there first.`;
    case "DATABASE_NEWER_VERSION":
      return `${name} was last used by a newer version of HoploDex. Update HoploDex to open it. The file has not been changed.`;
    case "DATABASE_DAMAGED":
      return `${name} is damaged and can't be opened.`;
    case "DATABASE_UNREADABLE":
      return `HoploDex can't read ${name}: it doesn't have permission to open the file, or the drive or network holding it isn't available. The file has not been changed.`;
    default:
      return error.message;
  }
}

const BACKUP_FAILURE: Record<Extract<ChooserNotice, { kind: "backupFailed" }>["reason"], string> = {
  locationUnavailable: "the backup location is not available",
  insufficientSpace: "there is not enough space there",
  interrupted: "the backup was interrupted",
  io: "the backup was interrupted",
  databaseUnreachable: "its file could not be reached",
};

/** A notice's sentence (contracts/ui-databases.md §1 "Notices"), or `null`
 * for one with nothing to say, such as an ordinary close. */
function noticeText(notice: ChooserNotice): string | null {
  const name = databaseNameOf(notice.databasePath);
  switch (notice.kind) {
    case "closed":
      return ["lockedByUser", "idle", "sleep", "screenLocked"].includes(notice.reason)
        ? `HoploDex locked ${name}.`
        : null;
    case "takenOver":
      return `${name} was taken over on another computer, so HoploDex stopped saving to it here and closed it.`;
    case "backupFailed":
      return `${name} was not backed up: ${BACKUP_FAILURE[notice.reason]}. Its changes will be backed up at the next close.`;
    case "pendingChangesLost":
      return `Unsaved changes could not be kept when ${name} locked.`;
    case "operationStopped":
      return null;
  }
}

/** Why the last open of a row failed. */
type Failure =
  | { path: string; kind: "message"; message: string }
  | { path: string; kind: "notFound" }
  | { path: string; kind: "elsewhere"; elsewhere: OpenElsewhere };

export interface DatabaseChooserProps {
  /** The database just closed, selected in place of the backend's choice
   * (FR-033). */
  selectPath?: string | null;
}

/** The full-window screen shown whenever no database is open (FR-020,
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
  const [notices, setNotices] = useState<{ id: number; text: string }[]>([]);
  const [creating, setCreating] = useState(false);
  const [announcement, setAnnouncement] = useState("");
  const [takeOver, setTakeOver] = useState<{ row: ChooserRow; machineName: string } | null>(null);

  useEffect(() => {
    let current = true;
    databasesService.getChooserState().then(
      (loaded) => {
        if (!current) return;
        setState(loaded);
        const wanted = selectPath ?? loaded.selectedPath;
        setSelected(loaded.recent.some((r) => r.path === wanted) ? wanted : loaded.selectedPath);
        setNotices(
          loaded.notices.flatMap((notice, id) => {
            const text = noticeText(notice);
            return text ? [{ id, text }] : [];
          }),
        );
      },
      () => current && setLoadError("HoploDex couldn't read its list of databases."),
    );
    return () => {
      current = false;
    };
  }, [selectPath]);

  const rows = useMemo<ChooserRow[]>(() => {
    const known: ChooserRow[] = (state?.recent ?? []).map(({ path, name, available }) => ({
      path,
      name,
      available,
    }));
    return [...picked.filter((row) => !known.some((k) => k.path === row.path)), ...known];
  }, [picked, state]);

  function select(path: string) {
    setSelected(path);
    setFailure(null);
  }

  async function openRow(row: ChooserRow, passphrase: string, takingOver = false) {
    setOpening(row.path);
    setFailure(null);
    setAnnouncement(`Opening ${row.name}…`);
    try {
      // On success the session replaces this screen with the collection.
      if (takingOver) await session.openDatabase(row.path, passphrase, { takeOver: true });
      else await session.openDatabase(row.path, passphrase);
    } catch (error) {
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
        setFailure({ path: row.path, kind: "message", message: openFailureText(row.name, error) });
      }
    } finally {
      setOpening(null);
      setAnnouncement("");
    }
  }

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
    const chosen = await openFileDialog({
      multiple: false,
      directory: false,
      title: `Locate ${row.name}`,
      filters: DATABASE_FILTERS,
    });
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
          r.path === row.path ? { path: chosen, name: databaseNameOf(chosen), available: true } : r,
        ),
      );
    }
    select(chosen);
  }

  async function openAnotherFile() {
    const chosen = await openFileDialog({
      multiple: false,
      directory: false,
      title: "Open a database file",
      filters: DATABASE_FILTERS,
    });
    if (typeof chosen !== "string") return;
    if (!rows.some((row) => row.path === chosen)) {
      setPicked((current) => [
        { path: chosen, name: databaseNameOf(chosen), available: true },
        ...current,
      ]);
    }
    select(chosen);
  }

  function confirmTakeOver(passphrase: string) {
    if (takeOver) void openRow(takeOver.row, passphrase, true);
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
            <ThemeToggle />
          </div>
        </div>
      </header>

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
              <Button
                variant="ghost"
                size="sm"
                icon="close"
                aria-label="Dismiss"
                onClick={() => setNotices((current) => current.filter((n) => n.id !== notice.id))}
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
                          ? `${row.name} is no longer at this location.`
                          : undefined
                      }
                      elsewhere={failed?.kind === "elsewhere" ? failed.elsewhere : undefined}
                      onSelect={() => select(row.path)}
                      onOpen={(passphrase) => void openRow(row, passphrase)}
                      onRemove={() => void removeRow(row)}
                      onLocate={() => void locateRow(row)}
                      onGoBack={() => setFailure(null)}
                      onTakeOver={() =>
                        failed?.kind === "elsewhere" &&
                        setTakeOver({ row, machineName: failed.elsewhere.machineName })
                      }
                    />
                  );
                })}
              </ul>
            )}
            <div
              className={
                firstRun ? "hd-chooser__actions hd-chooser__actions--large" : "hd-chooser__actions"
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
            <TakeOverConfirm
              target={takeOver && { name: takeOver.row.name, machineName: takeOver.machineName }}
              onCancel={() => setTakeOver(null)}
              onConfirm={confirmTakeOver}
            />
          </>
        )}
      </main>

      <p className="hd-sr-only" role="status" aria-live="polite">
        {announcement}
      </p>
    </div>
  );
}
