import { browser, createDatabase, expect } from "../support/ui";

/**
 * THROWAWAY PROBE (specs/008-hardening-batch tasks T006-T008, research.md
 * §2). Proves the `HoploDex-Session` request header reaches a command's
 * `ScopedSession` argument on both of Tauri's IPC paths, on this OS:
 *
 *   1. the `ipc://` custom protocol (`http://ipc.localhost` on Windows), the
 *      path every call takes while `fetch` to it works, and
 *   2. the `postMessage` fallback (`window.ipc.postMessage`).
 *
 * `list_firearms` is called with the header (served) and without it
 * (`DATABASE_CLOSED`), plus with a stale id (`DATABASE_CLOSED`).
 *
 * HOW THE FALLBACK IS FORCED. Tauri's `ipc-protocol.js` (tauri 2.x
 * `scripts/ipc-protocol.js`) sends each message with a `fetch` to the custom
 * protocol and, when that `fetch` rejects, logs "IPC custom protocol failed,
 * Tauri will now use the postMessage interface instead", sets its private
 * `customProtocolIpcFailed` flag and re-sends through `window.ipc.postMessage`
 * for the rest of the page's life. It looks up the global `fetch` on every
 * call, so this spec replaces `window.fetch` with one that rejects every
 * request to the IPC protocol. The call then can only succeed through
 * `postMessage`, and the header must survive it (`options.headers` ->
 * `InvokeMessage::headers()`). The page cannot go back to the custom protocol
 * afterwards, so the custom-protocol tests run first, and the page is reloaded
 * only by relaunching the app. To force the fallback by hand in any session,
 * run in the page:
 *
 *   const f = window.fetch;
 *   window.fetch = (u, ...a) =>
 *     /^(ipc:\/\/|https?:\/\/ipc\.localhost)/.test(String(u?.url ?? u))
 *       ? Promise.reject(new TypeError("blocked")) : f(u, ...a);
 *
 * and make one command call; every later call goes through `postMessage`.
 *
 * Run it alone: `npm run test:e2e -- --spec e2e/specs/zz-session-header-probe.e2e.ts`
 * (after `npm run build`). Delete it once T006-T008 are recorded.
 */

const HEADER = "HoploDex-Session";

interface Outcome {
  ok: boolean;
  /** The result when it succeeded; the `CommandError` when it failed. */
  value: unknown;
}

/** Calls `list_firearms` from the page with `header` (undefined = none),
 * resolving to its outcome instead of throwing. `"current"` sends the open
 * database's session id from `get_database_status`. */
async function listFirearms(header: "current" | "stale" | "none"): Promise<Outcome> {
  return browser.execute(
    async (mode: string, name: string) => {
      type Invoke = (cmd: string, args: unknown, options?: unknown) => Promise<unknown>;
      const invoke = (window as unknown as { __TAURI_INTERNALS__: { invoke: Invoke } })
        .__TAURI_INTERNALS__.invoke;
      const status = (await invoke("get_database_status", {})) as { sessionId: number };
      const options =
        mode === "none"
          ? undefined
          : {
              headers: {
                [name]: String(mode === "current" ? status.sessionId : status.sessionId + 1000),
              },
            };
      try {
        const value = await invoke("list_firearms", { input: {} }, options);
        return { ok: true, value };
      } catch (error) {
        return { ok: false, value: error };
      }
    },
    header,
    HEADER,
  );
}

/** Replaces `window.fetch` so every request to the IPC protocol rejects, and
 * counts the requests it saw (`window.__ipcFetches`). */
async function blockIpcProtocol() {
  await browser.execute(() => {
    const w = window as unknown as { __ipcFetches?: number; __ipcWarned?: string[] };
    const original = window.fetch.bind(window);
    w.__ipcFetches = 0;
    w.__ipcWarned = [];
    const warn = console.warn.bind(console);
    console.warn = (...args: unknown[]) => {
      w.__ipcWarned!.push(String(args[0]));
      warn(...args);
    };
    window.fetch = ((input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(
        (input as Request | undefined)?.url ?? (input as URL | string | undefined) ?? "",
      );
      if (/^(ipc:\/\/|https?:\/\/ipc\.localhost)/.test(url)) {
        w.__ipcFetches!++;
        return Promise.reject(new TypeError("blocked by the probe"));
      }
      return original(input, init);
    }) as typeof window.fetch;
  });
}

describe("HoploDex-Session header reaches ScopedSession", () => {
  before(async () => {
    await createDatabase({ name: "Probe" });
  });

  it("serves list_firearms with the header over the ipc:// custom protocol", async () => {
    const served = await listFirearms("current");
    expect(served.ok).toBe(true);
    expect(Array.isArray((served.value as { groups: unknown[] }).groups)).toBe(true);
  });

  it("refuses list_firearms without the header over the custom protocol", async () => {
    const refused = await listFirearms("none");
    expect(refused.ok).toBe(false);
    expect((refused.value as { code: string }).code).toBe("DATABASE_CLOSED");
  });

  it("refuses list_firearms with another session's id over the custom protocol", async () => {
    const refused = await listFirearms("stale");
    expect(refused.ok).toBe(false);
    expect((refused.value as { code: string }).code).toBe("DATABASE_CLOSED");
  });

  describe("over the postMessage fallback", () => {
    before(async () => {
      await blockIpcProtocol();
      // The first call hits the blocked fetch, logs the fallback warning and
      // is re-sent through postMessage; every later call goes there directly.
      await listFirearms("none");
    });

    it("has left the custom protocol", async () => {
      const seen = await browser.execute(() => {
        const w = window as unknown as { __ipcFetches: number; __ipcWarned: string[] };
        return { fetches: w.__ipcFetches, warned: w.__ipcWarned };
      });
      // Exactly one rejected attempt (the one that tripped the fallback).
      expect(seen.fetches).toBe(1);
      expect(seen.warned.some((m) => m.includes("IPC custom protocol failed"))).toBe(true);
    });

    it("serves list_firearms with the header", async () => {
      const served = await listFirearms("current");
      expect(served.ok).toBe(true);
      expect(Array.isArray((served.value as { groups: unknown[] }).groups)).toBe(true);
    });

    it("refuses list_firearms without the header", async () => {
      const refused = await listFirearms("none");
      expect(refused.ok).toBe(false);
      expect((refused.value as { code: string }).code).toBe("DATABASE_CLOSED");
    });

    it("refuses list_firearms with another session's id", async () => {
      const refused = await listFirearms("stale");
      expect(refused.ok).toBe(false);
      expect((refused.value as { code: string }).code).toBe("DATABASE_CLOSED");
    });

    it("never touched the custom protocol again", async () => {
      const fetches = await browser.execute(
        () => (window as unknown as { __ipcFetches: number }).__ipcFetches,
      );
      expect(fetches).toBe(1);
    });
  });
});
