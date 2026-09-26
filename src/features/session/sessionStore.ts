import { createContext, useContext } from "react";
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
  openDatabase: (path: string, passphrase: string) => Promise<void>;
  /** Creates a database and opens it. */
  createDatabase: (input: CreateDatabaseInput) => Promise<void>;
  refreshStatus: () => Promise<void>;
  dismissNote: (note: NoteKind) => Promise<void>;
}

export const SessionContext = createContext<SessionState | null>(null);

export function useSession(): SessionState {
  const state = useContext(SessionContext);
  if (!state) throw new Error("useSession must be used inside SessionProvider");
  return state;
}
