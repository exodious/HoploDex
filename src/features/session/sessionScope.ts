import { createContext, useContext } from "react";
import { CommandFailure, invoke as clientInvoke } from "../../services/tauriClient";
import type { Draft, PendingSummary } from "../databases/types";
import * as sessionService from "./sessionService";

/**
 * What one open of a database owns on the frontend (specs/008-hardening-batch
 * research.md §3, data-model.md "SessionScope"). `SessionProvider` creates one
 * per open from `DatabaseStatus.sessionId` and ends it, synchronously, before
 * anything of the next session can run. A component captures the scope when it
 * renders, so work begun in one session names that session even if it finishes
 * after another has opened.
 */
export interface SessionScope {
  /** `DatabaseStatus.sessionId` of the open this scope belongs to. */
  readonly id: number;
  /** True once {@link end} has run. */
  readonly ended: boolean;
  /**
   * Ends the scope: every clean-up registered with {@link onEnd} runs once,
   * and every later `invoke` is refused. Synchronous and idempotent.
   */
  end(): void;
  /**
   * Calls a command with `HoploDex-Session: <id>`. Once the scope has ended
   * it rejects with `CommandFailure { code: "DATABASE_CLOSED" }` without
   * sending; so does a call whose response arrives after the scope ended, and
   * the response is dropped (FR-002).
   */
  invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
  /**
   * Registers a clean-up to run when the scope ends (what the session keeps
   * for reuse, research.md §4). If the scope has ended already it runs at
   * once. Returns a function that removes it.
   */
  onEnd(cleanUp: () => void): () => void;
  /** Photo id → data URL of the thumbnails loaded in this session. Emptied
   * by {@link end}, so no image outlives its session (research.md §4). */
  readonly thumbnails: Map<number, string>;
  /**
   * The pending changes the user chose to resume, until their form opens
   * (FR-003, research.md §6). Set by `SessionProvider` when `resume` is
   * answered; a form matches it only through this scope. {@link end} drops
   * it.
   */
  resumed: { draft: Draft; state: "opening" } | null;
  /**
   * The form opened with the resumed changes: tells the backend
   * (`resolve_pending_changes("opened")`), which keeps the draft as the
   * session's staged draft. Replaceable: `SessionProvider` wraps it to close
   * the dialog.
   */
  resumeOpened(): Promise<void>;
  /**
   * The form for the resumed changes can't be opened, for `reason`:
   * `resolve_pending_changes("notOpened")`, which blocks the collection again
   * and gives back the summary for the dialog. Drops {@link resumed}.
   * Replaceable like {@link resumeOpened}.
   */
  resumeFailed(reason: string): Promise<PendingSummary | null>;
}

function sessionEnded(): CommandFailure {
  return new CommandFailure({
    code: "DATABASE_CLOSED",
    message: "The database was closed.",
  });
}

function runCleanUp(cleanUp: () => void): void {
  try {
    cleanUp();
  } catch (error) {
    // One clean-up failing must not keep the others, or the end, from running.
    console.error("A session clean-up failed", error);
  }
}

/** Creates the scope of the open whose session id is `id`. */
export function createSessionScope(id: number): SessionScope {
  let ended = false;
  const cleanUps = new Set<() => void>();

  const scope: SessionScope = {
    id,
    thumbnails: new Map(),
    resumed: null,
    async resumeOpened() {
      await sessionService.resolvePendingChanges(scope, "opened");
    },
    async resumeFailed(reason) {
      // Only this computer's own console (constitution V).
      console.warn("The resumed form couldn't be opened:", reason);
      scope.resumed = null;
      const { pending } = await sessionService.resolvePendingChanges(scope, "notOpened");
      return pending ?? null;
    },
    get ended() {
      return ended;
    },
    end() {
      if (ended) return;
      ended = true;
      scope.thumbnails.clear();
      scope.resumed = null;
      const pending = [...cleanUps];
      cleanUps.clear();
      pending.forEach(runCleanUp);
    },
    async invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
      if (ended) throw sessionEnded();
      let result: T;
      try {
        result = await clientInvoke<T>(command, args, { session: id });
      } catch (error) {
        // A failure that arrives after the end is as late as a value.
        if (ended) throw sessionEnded();
        throw error;
      }
      if (ended) throw sessionEnded();
      return result;
    },
    onEnd(cleanUp) {
      if (ended) {
        runCleanUp(cleanUp);
        return () => undefined;
      }
      cleanUps.add(cleanUp);
      return () => {
        cleanUps.delete(cleanUp);
      };
    },
  };
  return scope;
}

export const SessionScopeContext = createContext<SessionScope | null>(null);

/** The scope of the open database; throws outside `SessionProvider`'s tree. */
export function useSessionScope(): SessionScope {
  const scope = useContext(SessionScopeContext);
  if (!scope) throw new Error("useSessionScope must be used inside SessionProvider");
  return scope;
}
