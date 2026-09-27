import { invoke, listen } from "../../services/tauriClient";
import type {
  BackupProgress,
  CloseOutcome,
  CloseReason,
  DatabaseStatus,
  Draft,
  NoteKind,
  OperationKind,
} from "../databases/types";

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

/** Stops the backup the current close is making (FR-027). Its changes stay
 * waiting for the next close. */
export function skipBackup(): Promise<void> {
  return invoke<void>("skip_backup");
}

/** Closes the open database, if any, and exits. Never resolves normally. */
export function quitApplication(): Promise<void> {
  return invoke<void>("quit_application");
}

/** "Lock now" (FR-033, FR-035): `draft`, the form's unsaved input, is kept
 * as pending changes, then the database closes as a lock. */
export function lockDatabase(draft: Draft | null): Promise<CloseOutcome> {
  return invoke<CloseOutcome>("lock_database", { draft });
}

/** Mirrors the open form's unsaved input into the backend's memory, for a
 * lock it starts on its own (research.md §16); `null` when clean. */
export function stagePendingChanges(draft: Draft | null): Promise<void> {
  return invoke<void>("stage_pending_changes", { draft });
}

/** Resumes or discards the open database's pending changes (FR-039). */
export function resolvePendingChanges(action: "resume" | "discard"): Promise<{ draft?: Draft }> {
  return invoke<{ draft?: Draft }>("resolve_pending_changes", { action });
}

/** Input to the window: the idle time starts again (FR-035). */
export function noteActivity(): Promise<void> {
  return invoke<void>("note_activity");
}

/** A native file or folder dialog opened or closed (research.md §15). */
export function setIdlePaused(paused: boolean): Promise<void> {
  return invoke<void>("set_idle_paused", { reason: "nativeDialog", paused });
}

/** `session:closed`'s payload. */
export interface SessionClosed {
  reason: CloseReason;
  databasePath: string;
  outcome?: CloseOutcome;
  /** What a sleep stopped (FR-037). */
  stoppedOperation?: OperationKind;
}

/** A normal close has started: show the closing screen, as a backup may
 * follow. */
export function onSessionClosing(handler: (closing: { reason: CloseReason }) => void): () => void {
  return listen<{ reason: CloseReason }>("session:closing", handler);
}

/** The backup a close is making, in bytes. */
export function onBackupProgress(handler: (progress: BackupProgress) => void): () => void {
  return listen<BackupProgress>("backup:progress", handler);
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
