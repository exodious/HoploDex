import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen } from "@tauri-apps/api/event";
import { CommandFailure, invoke, listen } from "./tauriClient";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

beforeEach(() => {
  vi.mocked(tauriInvoke).mockReset();
  vi.mocked(tauriListen).mockReset();
});

describe("invoke", () => {
  it("turns a rejection carrying details into a CommandFailure with the same details", async () => {
    vi.mocked(tauriInvoke).mockRejectedValue({
      code: "DATABASE_OPEN_ELSEWHERE",
      message: "This database is open on Workshop PC.",
      details: { machineName: "Workshop PC", since: "2026-09-26T10:00:00Z" },
    });

    const failure = (await invoke("open_database", { path: "/x.hoplodex" }).catch(
      (e: unknown) => e,
    )) as CommandFailure;

    expect(failure).toBeInstanceOf(CommandFailure);
    expect(failure.code).toBe("DATABASE_OPEN_ELSEWHERE");
    expect(failure.message).toBe("This database is open on Workshop PC.");
    expect(failure.details).toEqual({ machineName: "Workshop PC", since: "2026-09-26T10:00:00Z" });
  });

  it("leaves details undefined when the backend sends none", async () => {
    vi.mocked(tauriInvoke).mockRejectedValue({
      code: "VALIDATION_ERROR",
      message: "Check the highlighted fields.",
      fieldErrors: { name: "Enter a name." },
    });

    const failure = (await invoke("create_database").catch((e: unknown) => e)) as CommandFailure;

    expect(failure).toBeInstanceOf(CommandFailure);
    expect(failure.fieldErrors).toEqual({ name: "Enter a name." });
    expect(failure.details).toBeUndefined();
  });
});

describe("listen", () => {
  it("subscribes with a typed payload and returns a function that unsubscribes", async () => {
    const unlisten = vi.fn();
    let deliver: ((event: { payload: unknown }) => void) | undefined;
    vi.mocked(tauriListen).mockImplementation((_event, handler) => {
      deliver = handler as (event: { payload: unknown }) => void;
      return Promise.resolve(unlisten);
    });
    const handler = vi.fn<(payload: { reason: string }) => void>();

    const stop = listen<{ reason: string }>("session:closing", handler);
    await Promise.resolve();
    deliver?.({ payload: { reason: "idle" } });

    expect(tauriListen).toHaveBeenCalledWith("session:closing", expect.any(Function));
    expect(handler).toHaveBeenCalledWith({ reason: "idle" });

    stop();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it("unsubscribes even when stopped before the subscription is ready", async () => {
    const unlisten = vi.fn();
    let ready: ((stop: () => void) => void) | undefined;
    vi.mocked(tauriListen).mockImplementation(
      () =>
        new Promise((resolve) => {
          ready = resolve;
        }),
    );

    const stop = listen("session:closed", vi.fn());
    stop();
    ready?.(unlisten);
    await Promise.resolve();

    expect(unlisten).toHaveBeenCalledTimes(1);
  });
});
