import { createContext, useContext } from "react";
import { CommandFailure, invoke as clientInvoke } from "../../services/tauriClient";

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

  return {
    id,
    get ended() {
      return ended;
    },
    end() {
      if (ended) return;
      ended = true;
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
}

export const SessionScopeContext = createContext<SessionScope | null>(null);

/** The scope of the open database; throws outside `SessionProvider`'s tree. */
export function useSessionScope(): SessionScope {
  const scope = useContext(SessionScopeContext);
  if (!scope) throw new Error("useSessionScope must be used inside SessionProvider");
  return scope;
}
