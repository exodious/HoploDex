import { invoke } from "../../services/tauriClient";
import type { DatabaseStatus, NoteKind } from "../databases/types";

// Typed wrappers for the open session's own commands (specs/003
// contracts/tauri-commands.md "Commands: the open session").

/** Fails with `DATABASE_CLOSED` when no database is open. */
export function getDatabaseStatus(): Promise<DatabaseStatus> {
  return invoke<DatabaseStatus>("get_database_status");
}

export function dismissNote(note: NoteKind): Promise<void> {
  return invoke<void>("dismiss_note", { note });
}
