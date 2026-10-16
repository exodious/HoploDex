import { invoke, listen } from "../../services/tauriClient";
import type { SessionScope } from "./sessionScope";
import type {
  BackupProgress,
  CloseOutcome,
  CloseReason,
  DatabaseStatus,
  Draft,
  NoteKind,
  OperationKind,
  PendingSummary,
} from "../databases/types";

// Typed wrappers for the open session's own commands and events (003
// contracts/tauri-commands.md "Commands: the open session", "Events").

/** Fails with `DATABASE_CLOSED` when no database is open. */
export function getDatabaseStatus(): Promise<DatabaseStatus> {
  return invoke<DatabaseStatus>("get_database_status");
}

export function dismissNote(scope: SessionScope, note: NoteKind): Promise<void> {
  return scope.invoke<void>("dismiss_note", { note });
}

/** The normal close, or a switch away (003 FR-010). Unsaved changes have been
 * dealt with first. */
export function closeDatabase(
  scope: SessionScope,
  reason: "closed" | "switched",
): Promise<CloseOutcome> {
  return scope.invoke<CloseOutcome>("close_database", { reason });
}

/** Stops the backup the current close is making (003 FR-027). Its changes stay
 * waiting for the next close. */
export function skipBackup(): Promise<void> {
  return invoke<void>("skip_backup");
}

/** Closes the open database, if any, and exits. Never resolves normally. */
export function quitApplication(): Promise<void> {
  return invoke<void>("quit_application");
}

/** "Lock now" (003 FR-033, FR-035): `draft`, the form's unsaved input, is kept
 * as pending changes, then the database closes as a lock. */
export function lockDatabase(scope: SessionScope, draft: Draft | null): Promise<CloseOutcome> {
  return scope.invoke<CloseOutcome>("lock_database", { draft });
}

/** Mirrors the open form's unsaved input into the backend's memory, for a
 * lock it starts on its own (003 research.md §16); `null` when clean. */
export function stagePendingChanges(scope: SessionScope, draft: Draft | null): Promise<void> {
  return scope.invoke<void>("stage_pending_changes", { draft });
}

/** Answers the open database's pending changes (003 FR-039, 008 research.md
 * §6): `resume` gives back the draft and serves the collection while its form
 * opens; `opened` says it did; `notOpened` says it couldn't, and gives back
 * the summary for the dialog; `discard` removes them. */
export function resolvePendingChanges(
  scope: SessionScope,
  action: PendingAction,
): Promise<{ draft?: Draft; pending?: PendingSummary }> {
  return scope.invoke<{ draft?: Draft; pending?: PendingSummary }>("resolve_pending_changes", {
    action,
  });
}

export type PendingAction = "resume" | "opened" | "notOpened" | "discard";

/** Input to the window: the idle time starts again (003 FR-035). */
export function noteActivity(): Promise<void> {
  return invoke<void>("note_activity");
}

/** A native file or folder dialog opened or closed (003 research.md §15). */
export function setIdlePaused(paused: boolean): Promise<void> {
  return invoke<void>("set_idle_paused", { reason: "nativeDialog", paused });
}

/** `session:closed`'s payload. */
export interface SessionClosed {
  /** The session that closed: the frontend ends only that session's scope
   * (008 research.md §3). */
  sessionId: number;
  reason: CloseReason;
  databasePath: string;
  outcome?: CloseOutcome;
  /** What a sleep stopped (003 FR-037). */
  stoppedOperation?: OperationKind;
}

/** `session:closing`'s payload. */
export interface SessionClosing {
  sessionId: number;
  reason: CloseReason;
}

/** A normal close has started: show the closing screen, as a backup may
 * follow. */
export function onSessionClosing(handler: (closing: SessionClosing) => void): () => void {
  return listen<SessionClosing>("session:closing", handler);
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
