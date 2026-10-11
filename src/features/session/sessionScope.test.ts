import { renderHook } from "@testing-library/react";
import { createElement, type ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as tauriClient from "../../services/tauriClient";
import { CommandFailure } from "../../services/tauriClient";
import { SessionScopeContext, createSessionScope, useSessionScope } from "./sessionScope";

vi.mock("../../services/tauriClient", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../services/tauriClient")>()),
  invoke: vi.fn(),
}));

beforeEach(() => {
  vi.mocked(tauriClient.invoke).mockReset();
});

describe("SessionScope.invoke", () => {
  it("sends the command through tauriClient with the scope's id as the session", async () => {
    vi.mocked(tauriClient.invoke).mockResolvedValue({ groups: [] });
    const scope = createSessionScope(42);

    const result = await scope.invoke("list_firearms", { input: { query: "x" } });

    expect(result).toEqual({ groups: [] });
    expect(tauriClient.invoke).toHaveBeenCalledTimes(1);
    expect(tauriClient.invoke).toHaveBeenCalledWith(
      "list_firearms",
      { input: { query: "x" } },
      { session: 42 },
    );
  });

  it("rejects with DATABASE_CLOSED after end() without calling tauriClient.invoke", async () => {
    const scope = createSessionScope(42);
    scope.end();

    const failure = (await scope
      .invoke("list_firearms")
      .catch((e: unknown) => e)) as CommandFailure;

    expect(failure).toBeInstanceOf(CommandFailure);
    expect(failure.code).toBe("DATABASE_CLOSED");
    expect(tauriClient.invoke).not.toHaveBeenCalled();
  });

  it("rejects a response that resolves after end() and never returns its value", async () => {
    let finish!: (value: string) => void;
    vi.mocked(tauriClient.invoke).mockReturnValue(new Promise((resolve) => (finish = resolve)));
    const scope = createSessionScope(42);

    const call = scope.invoke<string>("get_photo_data");
    const outcome = call.then(
      (value) => ({ value }),
      (error: unknown) => ({ error }),
    );
    scope.end();
    finish("data:image/png;base64,AAAA");
    const settled = await outcome;

    expect(settled).not.toHaveProperty("value");
    const failure = (settled as { error: CommandFailure }).error;
    expect(failure).toBeInstanceOf(CommandFailure);
    expect(failure.code).toBe("DATABASE_CLOSED");
  });

  it("rejects a failure that arrives after end() as DATABASE_CLOSED too", async () => {
    let fail!: (reason: unknown) => void;
    vi.mocked(tauriClient.invoke).mockReturnValue(new Promise((_, reject) => (fail = reject)));
    const scope = createSessionScope(42);

    const call = scope.invoke("get_firearm", { id: 1 }).catch((e: unknown) => e);
    scope.end();
    fail(new CommandFailure({ code: "NOT_FOUND", message: "Gone." }));

    expect(((await call) as CommandFailure).code).toBe("DATABASE_CLOSED");
  });

  it("passes a failure that arrives before the end through unchanged", async () => {
    const original = new CommandFailure({ code: "NOT_FOUND", message: "Gone." });
    vi.mocked(tauriClient.invoke).mockRejectedValue(original);
    const scope = createSessionScope(42);

    await expect(scope.invoke("get_firearm", { id: 1 })).rejects.toBe(original);
  });
});

describe("SessionScope.end", () => {
  it("is synchronous and idempotent, and runs each clean-up once", () => {
    const scope = createSessionScope(1);
    const first = vi.fn();
    const second = vi.fn();
    scope.onEnd(first);
    scope.onEnd(second);
    expect(scope.ended).toBe(false);

    scope.end();
    expect(scope.ended).toBe(true);
    expect(first).toHaveBeenCalledTimes(1);
    expect(second).toHaveBeenCalledTimes(1);

    scope.end();
    expect(first).toHaveBeenCalledTimes(1);
    expect(second).toHaveBeenCalledTimes(1);
  });

  it("does not run a clean-up that was removed", () => {
    const scope = createSessionScope(1);
    const cleanUp = vi.fn();
    scope.onEnd(cleanUp)();

    scope.end();

    expect(cleanUp).not.toHaveBeenCalled();
  });

  it("runs the other clean-ups when one throws", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const scope = createSessionScope(1);
    const after = vi.fn();
    scope.onEnd(() => {
      throw new Error("boom");
    });
    scope.onEnd(after);

    scope.end();

    expect(after).toHaveBeenCalledTimes(1);
  });

  it("runs a clean-up registered after the end at once", () => {
    const scope = createSessionScope(1);
    scope.end();
    const cleanUp = vi.fn();

    scope.onEnd(cleanUp);

    expect(cleanUp).toHaveBeenCalledTimes(1);
  });
});

describe("useSessionScope", () => {
  it("returns the provided scope", () => {
    const scope = createSessionScope(5);
    const wrapper = ({ children }: { children: ReactNode }) =>
      createElement(SessionScopeContext.Provider, { value: scope }, children);

    const { result } = renderHook(() => useSessionScope(), { wrapper });

    expect(result.current).toBe(scope);
  });

  it("throws outside a provider", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);

    expect(() => renderHook(() => useSessionScope())).toThrow(/inside SessionProvider/);
  });
});
