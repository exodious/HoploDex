import { invoke } from "../../services/tauriClient";
import type { ChooserState, CreateDatabaseInput, DatabaseStatus } from "./types";

// Typed wrappers for choosing, creating and opening databases (specs/003
// contracts/tauri-commands.md). Passphrases are passed straight through as
// command arguments and never kept (FR-007).

export function getChooserState(): Promise<ChooserState> {
  return invoke<ChooserState>("get_chooser_state");
}

export function createDatabase(input: CreateDatabaseInput): Promise<DatabaseStatus> {
  return invoke<DatabaseStatus>("create_database", { ...input });
}

export function openDatabase(path: string, passphrase: string): Promise<DatabaseStatus> {
  return invoke<DatabaseStatus>("open_database", { path, passphrase });
}
