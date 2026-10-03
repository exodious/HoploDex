import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useDebounced } from "./searchHooks";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("useDebounced", () => {
  it("is not busy on first render, and passes the value on after the delay", () => {
    const { result, rerender } = renderHook(({ value }) => useDebounced(value, 150), {
      initialProps: { value: "" },
    });
    expect(window.__hoplodexBusy).toBe(0);

    rerender({ value: "colt" });
    expect(result.current).toBe("");
    expect(window.__hoplodexBusy).toBe(1);

    act(() => void vi.advanceTimersByTime(150));
    expect(result.current).toBe("colt");
    expect(window.__hoplodexBusy).toBe(0);
  });

  it("stays busy through a burst of typing, counting it once", () => {
    const { result, rerender } = renderHook(({ value }) => useDebounced(value, 150), {
      initialProps: { value: "" },
    });
    rerender({ value: "c" });
    act(() => void vi.advanceTimersByTime(100));
    rerender({ value: "co" });
    expect(window.__hoplodexBusy).toBe(1);
    act(() => void vi.advanceTimersByTime(100));
    expect(result.current).toBe("");
    expect(window.__hoplodexBusy).toBe(1);

    act(() => void vi.advanceTimersByTime(50));
    expect(result.current).toBe("co");
    expect(window.__hoplodexBusy).toBe(0);
  });

  it("stops counting when the field goes away mid-delay", () => {
    const { rerender, unmount } = renderHook(({ value }) => useDebounced(value, 150), {
      initialProps: { value: "" },
    });
    rerender({ value: "colt" });
    expect(window.__hoplodexBusy).toBe(1);

    unmount();

    expect(window.__hoplodexBusy).toBe(0);
  });
});
