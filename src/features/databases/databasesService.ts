import { invoke } from "../../services/tauriClient";
import type { ChooserState, CreateDatabaseInput, DatabaseStatus, RecentDatabase } from "./types";

// Typed wrappers for choosing, creating and opening databases (specs/003
// contracts/tauri-commands.md). Passphrases are passed straight through as
// command arguments and never kept (FR-007).

export function getChooserState(): Promise<ChooserState> {
  return invoke<ChooserState>("get_chooser_state");
}

export function createDatabase(input: CreateDatabaseInput): Promise<DatabaseStatus> {
  return invoke<DatabaseStatus>("create_database", { ...input });
}

export interface OpenOptions {
  /** Open a database marked open on another computer (FR-032), sent only
   * after the take-over confirmation. */
  takeOver?: boolean;
}

export function openDatabase(
  path: string,
  passphrase: string,
  options: OpenOptions = {},
): Promise<DatabaseStatus> {
  return invoke<DatabaseStatus>("open_database", {
    path,
    passphrase,
    ...(options.takeOver ? { takeOver: true } : {}),
  });
}

/** Takes a database off this computer's recent list. The file is never
 * touched (FR-012). */
export function removeRecentDatabase(path: string): Promise<{ removed: true }> {
  return invoke<{ removed: true }>("remove_recent_database", { path });
}

/** Points an unavailable recent entry at where its file now is (FR-012). */
export function locateDatabase(path: string, newPath: string): Promise<RecentDatabase> {
  return invoke<RecentDatabase>("locate_database", { path, newPath });
}
