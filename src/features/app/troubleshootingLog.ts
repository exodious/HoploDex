import { invoke } from "../../services/tauriClient";

/** The troubleshooting log's state (013 contracts/tauri-commands.md). The page
 * gets a state and a path, never the file's contents (013 FR-024). */
export type LogState = "off" | "on" | "detailed";

export interface TroubleshootingLog {
  state: LogState;
  /** The file's real path, or null when the app's folders can't be named. */
  path: string | null;
  exists: boolean;
  /** Meant to be on, but the file can't be written. */
  failed: boolean;
}

export function getTroubleshootingLog(): Promise<TroubleshootingLog> {
  return invoke<TroubleshootingLog>("get_troubleshooting_log");
}

export function setTroubleshootingLog(state: LogState): Promise<TroubleshootingLog> {
  return invoke<TroubleshootingLog>("set_troubleshooting_log", { state });
}

export function showTroubleshootingLog(): Promise<void> {
  return invoke<void>("show_troubleshooting_log");
}
