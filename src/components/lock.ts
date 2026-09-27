import { createContext, useContext } from "react";

/** "Ctrl+L", or "⌘L" on a Mac. */
export const LOCK_SHORTCUT =
  typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform) ? "⌘L" : "Ctrl+L";

/** Locks the open database. Provided only while one is open, so a `Dialog`
 * offers its lock button then and only then (contracts/ui-databases.md §4). */
export const LockContext = createContext<(() => void) | null>(null);

export function useLock(): (() => void) | null {
  return useContext(LockContext);
}
