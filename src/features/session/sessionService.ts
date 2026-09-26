import { invoke, listen } from "../../services/tauriClient";
import type { CloseOutcome, CloseReason, DatabaseStatus, NoteKind } from "../databases/types";

// Typed wrappers for the open session's own commands and events (specs/003
// contracts/tauri-commands.md "Commands: the open session", "Events").

/** Fails with `DATABASE_CLOSED` when no database is open. */
export function getDatabaseStatus(): Promise<DatabaseStatus> {
  return invoke<DatabaseStatus>("get_database_status");
}

export function dismissNote(note: NoteKind): Promise<void> {
  return invoke<void>("dismiss_note", { note });
}

/** The normal close, or a switch away (FR-010). Unsaved changes have been
 * dealt with first. */
export function closeDatabase(reason: "closed" | "switched"): Promise<CloseOutcome> {
  return invoke<CloseOutcome>("close_database", { reason });
}

/** Closes the open database, if any, and exits. Never resolves normally. */
export function quitApplication(): Promise<void> {
  return invoke<void>("quit_application");
}

/** `session:closed`'s payload. */
export interface SessionClosed {
  reason: CloseReason;
  databasePath: string;
  outcome?: CloseOutcome;
}

/** The database closed, for whatever reason: drop all collection state. */
export function onSessionClosed(handler: (closed: SessionClosed) => void): () => void {
  return listen<SessionClosed>("session:closed", handler);
}

/** The window's close button or an application quit: ask about unsaved
 * changes, then call {@link quitApplication}. */
export function onQuitRequested(handler: () => void): () => void {
  return listen<unknown>("app:quit-requested", () => handler());
}
