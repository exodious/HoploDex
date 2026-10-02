import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { track } from "../lib/busy";

/**
 * Mirrors src-tauri/src/commands/error.rs's `CommandError` — the one error
 * shape every IPC command returns, per contracts/tauri-commands.md.
 */
export interface CommandError {
  code: string;
  message: string;
  fieldErrors?: Record<string, string>;
  /** Structured data for specific codes, e.g. `{ path }` for
   * `DATABASE_NOT_FOUND` (specs/003 contracts/tauri-commands.md). */
  details?: Record<string, unknown>;
}

/** Thrown by {@link invoke} when a command rejects with a `CommandError`. */
export class CommandFailure extends Error {
  readonly code: string;
  readonly fieldErrors?: Record<string, string>;
  readonly details?: Record<string, unknown>;

  constructor(err: CommandError) {
    super(err.message);
    this.name = "CommandFailure";
    this.code = err.code;
    this.fieldErrors = err.fieldErrors;
    this.details = err.details;
  }
}

function isCommandError(value: unknown): value is CommandError {
  return (
    typeof value === "object" &&
    value !== null &&
    "code" in value &&
    "message" in value &&
    typeof (value as { code: unknown }).code === "string" &&
    typeof (value as { message: unknown }).message === "string"
  );
}

/**
 * The only path the frontend uses to reach the Tauri backend (database,
 * filesystem, keyring) — per contracts/tauri-commands.md. Wraps
 * `@tauri-apps/api`'s `invoke` to normalize backend failures into a typed
 * {@link CommandFailure} instead of an untyped rejection.
 */
export async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await track(() => tauriInvoke<T>(command, args));
  } catch (error) {
    if (isCommandError(error)) {
      throw new CommandFailure(error);
    }
    throw error;
  }
}

/**
 * Subscribes to a backend event (`session:closed`, `backup:progress`, …)
 * with a typed payload; returns the function that unsubscribes. Safe to call
 * from an effect: stopping before the subscription is ready still
 * unsubscribes once it is.
 */
export function listen<T>(event: string, handler: (payload: T) => void): () => void {
  let unlisten: (() => void) | undefined;
  let stopped = false;
  void tauriListen<T>(event, ({ payload }) => handler(payload)).then((stop) => {
    if (stopped) stop();
    else unlisten = stop;
  });
  return () => {
    stopped = true;
    unlisten?.();
  };
}

/** Files being dragged onto the window from the desktop. Dropped files are
 * delivered as paths — the webview itself gets no `File` for them (WebKitGTK
 * exposes none) — to be handed to a command that reads them. */
export type FileDropEvent =
  { type: "enter"; paths: string[] } | { type: "leave" } | { type: "drop"; paths: string[] };

/** Listens for files dragged onto the window; returns the unsubscribe
 * function. Does nothing outside the Tauri shell (unit tests, a plain
 * browser preview). */
export function listenForFileDrops(handler: (event: FileDropEvent) => void): () => void {
  let unlisten: (() => void) | undefined;
  let stopped = false;
  try {
    void getCurrentWebview()
      .onDragDropEvent(({ payload }) => {
        if (payload.type === "enter") handler({ type: "enter", paths: payload.paths });
        else if (payload.type === "drop") handler({ type: "drop", paths: payload.paths });
        else if (payload.type === "leave") handler({ type: "leave" });
      })
      .then((stop) => {
        if (stopped) stop();
        else unlisten = stop;
      })
      .catch(() => {
        // No drag-and-drop here; the file pickers still work.
      });
  } catch {
    // Not running inside the Tauri shell.
  }
  return () => {
    stopped = true;
    unlisten?.();
  };
}

/** Sets the window's title, as the taskbar and window switcher show it.
 * Does nothing outside the Tauri shell (unit tests, a plain browser
 * preview). */
export function setWindowTitle(title: string): void {
  try {
    void getCurrentWindow()
      .setTitle(title)
      .catch(() => {
        // The title stays as it was; nothing depends on it.
      });
  } catch {
    // Not running inside the Tauri shell.
  }
}
