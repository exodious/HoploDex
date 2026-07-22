import { invoke as tauriInvoke } from "@tauri-apps/api/core";

/**
 * Mirrors src-tauri/src/commands/error.rs's `CommandError` — the one error
 * shape every IPC command returns, per contracts/tauri-commands.md.
 */
export interface CommandError {
  code: string;
  message: string;
  fieldErrors?: Record<string, string>;
}

/** Thrown by {@link invoke} when a command rejects with a `CommandError`. */
export class CommandFailure extends Error {
  readonly code: string;
  readonly fieldErrors?: Record<string, string>;

  constructor(err: CommandError) {
    super(err.message);
    this.name = "CommandFailure";
    this.code = err.code;
    this.fieldErrors = err.fieldErrors;
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
    return await tauriInvoke<T>(command, args);
  } catch (error) {
    if (isCommandError(error)) {
      throw new CommandFailure(error);
    }
    throw error;
  }
}
