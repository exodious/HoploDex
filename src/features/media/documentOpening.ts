import { useSyncExternalStore } from "react";
import * as mediaService from "./mediaService";
import type { DocumentOpening } from "./types";

// This computer's "Open documents" setting (007 FR-012): what a document's name does, preview
// it in HoploDex or hand it to another app. It belongs to the computer, not to a database, so it
// is a module-level store rather than part of the collection's state; the Documents fieldset in
// the database settings and every document list read the same value.

let current: DocumentOpening = "preview";
const listeners = new Set<() => void>();

function set(value: DocumentOpening) {
  if (value === current) return;
  current = value;
  listeners.forEach((listener) => listener());
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** Reads the setting from the backend, every time it is called. Until it has been read the
 * value is "preview", the default. Rejects when the backend does, leaving the value as it was. */
export async function loadDocumentOpening(): Promise<void> {
  set(await mediaService.getDocumentOpening());
}

/** Asks the backend to change the setting (which, for "external", shows the native
 * confirmation). The store changes only when the backend says it did: resolves with `changed`. */
export async function chooseDocumentOpening(value: DocumentOpening): Promise<boolean> {
  const { changed } = await mediaService.setDocumentOpening(value);
  if (changed) set(value);
  return changed;
}

/** What this computer's setting is now; the component re-renders when it changes. */
export function useDocumentOpening(): DocumentOpening {
  return useSyncExternalStore(subscribe, () => current);
}
