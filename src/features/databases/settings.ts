import type { CollectionSettings } from "./types";

/** A new database's backup and lock settings, as the `collection_settings`
 * defaults in src-tauri/src/db/migrations/0001_initial.sql give them
 * (FR-024, FR-026, FR-034, FR-038). */
export const DEFAULT_SETTINGS = {
  backupsEnabled: true,
  keepCount: 5,
  idleEnabled: true,
  idleMinutes: 10,
  onScreenLock: false,
} as const;

/** "1 minute", "15 minutes", "2 hours". */
export function minutesLabel(minutes: number): string {
  if (minutes % 60 === 0 && minutes >= 60) {
    const hours = minutes / 60;
    return `${hours} ${hours === 1 ? "hour" : "hours"}`;
  }
  return `${minutes} ${minutes === 1 ? "minute" : "minutes"}`;
}

/** One of a database's settings, as the guide shows it: what it is here,
 * and what it would be had nobody changed it. */
export interface SettingRow {
  label: string;
  value: string;
  /** The default, said the same way as `value`. */
  defaultValue: string;
  /** Something about this computer, such as a folder that isn't there. */
  note?: string;
}

const onOff = (on: boolean) => (on ? "On" : "Off");

/** The backup settings of an open database, beside their defaults. */
export function backupRows(backups: CollectionSettings["backups"]): SettingRow[] {
  const nextToDatabase = "Next to the database";
  return [
    {
      label: "Automatic backups",
      value: onOff(backups.enabled),
      defaultValue: onOff(DEFAULT_SETTINGS.backupsEnabled),
    },
    {
      label: "Backups kept",
      value: String(backups.keepCount),
      defaultValue: String(DEFAULT_SETTINGS.keepCount),
    },
    {
      label: "Folder",
      value: backups.location.kind === "default" ? nextToDatabase : backups.location.path,
      defaultValue: nextToDatabase,
      note: backups.location.available ? undefined : "Not available on this computer",
    },
  ];
}

/** The lock settings of an open database, beside their defaults. */
export function lockRows(
  lock: CollectionSettings["lock"],
  screenLockSupported: boolean,
): SettingRow[] {
  const idle = (enabled: boolean, minutes: number) =>
    enabled ? `After ${minutesLabel(minutes)}` : "Off";
  return [
    {
      label: "Lock when not in use",
      value: idle(lock.idleEnabled, lock.idleMinutes),
      defaultValue: idle(DEFAULT_SETTINGS.idleEnabled, DEFAULT_SETTINGS.idleMinutes),
    },
    {
      label: "Lock at sleep",
      value: onOff(lock.idleEnabled),
      defaultValue: onOff(DEFAULT_SETTINGS.idleEnabled),
    },
    {
      label: "Lock when the screen locks",
      value: onOff(screenLockSupported && lock.onScreenLock),
      defaultValue: onOff(DEFAULT_SETTINGS.onScreenLock),
      note: screenLockSupported ? undefined : "Not available on this computer",
    },
  ];
}
