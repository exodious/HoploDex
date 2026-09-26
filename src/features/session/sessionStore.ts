import { createContext, useContext } from "react";
import type { OpenOptions } from "../databases/databasesService";
import type { CreateDatabaseInput, DatabaseStatus, NoteKind } from "../databases/types";

/**
 * The session: which database is open, if any (specs/003 plan.md). Nothing
 * from a collection exists outside an open session: when none is open the
 * chooser is all there is, and closing unmounts every collection view.
 */
export interface SessionState {
  /** The open database, or `null` while the chooser shows. */
  status: DatabaseStatus | null;
  /** Opens the database at `path`; rejects with the command's
   * `CommandFailure` and leaves the chooser showing. */
  openDatabase: (path: string, passphrase: string, options?: OpenOptions) => Promise<void>;
  /** Creates a database and opens it. */
  createDatabase: (input: CreateDatabaseInput) => Promise<void>;
  /** Closes the open database, or switches away from it, asking first about
   * a form with unsaved input (FR-010). */
  closeDatabase: (reason: "closed" | "switched") => Promise<void>;
  refreshStatus: () => Promise<void>;
  dismissNote: (note: NoteKind) => Promise<void>;
  /** Restores the open database, or the damaged one at `databasePath`, from
   * a backup; the restored database is then open (FR-028). Rejects with the
   * command's `CommandFailure`. */
  restoreBackup: (backupPath: string, passphrase: string, databasePath?: string) => Promise<void>;
  /** The backup settings should open once a database is open: asked for from
   * a failed-backup notice in the chooser (contracts/ui-databases.md §1). */
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
