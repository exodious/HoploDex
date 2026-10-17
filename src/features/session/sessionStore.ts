import { createContext, useContext } from "react";
import type { OpenOptions } from "../databases/databasesService";
import type { CreateDatabaseInput, DatabaseStatus, NoteKind } from "../databases/types";

/**
 * The session: which database is open, if any (003 plan.md). Nothing
 * from a collection exists outside an open session: when none is open the
 * chooser is all there is, and closing unmounts every collection view.
 */
export interface SessionState {
  /** The open database, or `null` while the chooser shows. */
  status: DatabaseStatus | null;
  /** Opens the database at `path` with the typed passphrase, or with the
   * one saved on this computer when `passphrase` is `null`; rejects with the
   * command's `CommandFailure` and leaves the chooser showing. */
  openDatabase: (path: string, passphrase: string | null, options?: OpenOptions) => Promise<void>;
  /** Upgrades the database waiting under `awaitingId` (`UPGRADE_NEEDED`) and
   * opens it, saving its backup in `backupFolder` when given instead of the
   * backup location (009 FR-011, FR-012). Rejects with the command's
   * `CommandFailure` and leaves the chooser showing. */
  upgradeDatabase: (awaitingId: number, backupFolder?: string) => Promise<void>;
  /** Creates a database and opens it. */
  createDatabase: (input: CreateDatabaseInput) => Promise<void>;
  /** Closes the open database, or switches away from it, asking first about
   * a form with unsaved input (003 FR-010). */
  closeDatabase: (reason: "closed" | "switched") => Promise<void>;
  /** Locks the open database at once, keeping unsaved input as pending
   * changes (003 FR-033, FR-035, FR-039). */
  lockDatabase: () => Promise<void>;
  refreshStatus: () => Promise<void>;
  dismissNote: (note: NoteKind) => Promise<void>;
  /** Restores the open database, or the damaged one at `databasePath`, from
   * a backup; the restored database is then open (003 FR-028). `upgrade`
   * (only ever `true`) upgrades a backup in an older layout as it is restored
   * (009 FR-019). Rejects with the command's `CommandFailure`. */
  restoreBackup: (
    backupPath: string,
    passphrase: string,
    databasePath?: string,
    upgrade?: true,
  ) => Promise<void>;
  /** The backup settings should open once a database is open: asked for from
   * a failed-backup notice in the chooser (003 contracts/ui-databases.md §1). */
  settingsRequested: boolean;
  requestSettings: () => void;
  clearSettingsRequest: () => void;
}

export const SessionContext = createContext<SessionState | null>(null);

export function useSession(): SessionState {
  const state = useContext(SessionContext);
  if (!state) throw new Error("useSession must be used inside SessionProvider");
  return state;
}
