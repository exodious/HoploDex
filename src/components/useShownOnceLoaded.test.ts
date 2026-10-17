import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { LOAD_WAIT_MS, useShownOnceLoaded } from "./useShownOnceLoaded";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("useShownOnceLoaded (#116)", () => {
  it("is false while closed, even when loaded", () => {
    const { result } = renderHook(() => useShownOnceLoaded(false, true));
    expect(result.current).toBe(false);
  });

  it("is true at once when it is opened already loaded", () => {
    const { result } = renderHook(() => useShownOnceLoaded(true, true));
    expect(result.current).toBe(true);
  });

  it("waits for the load, then shows", () => {
    const { result, rerender } = renderHook(({ loaded }) => useShownOnceLoaded(true, loaded), {
      initialProps: { loaded: false },
    });
    expect(result.current).toBe(false);

    rerender({ loaded: true });
    expect(result.current).toBe(true);
  });

  it("shows anyway once the wait is over", () => {
    const { result } = renderHook(() => useShownOnceLoaded(true, false));
    act(() => void vi.advanceTimersByTime(LOAD_WAIT_MS - 1));
    expect(result.current).toBe(false);

    act(() => void vi.advanceTimersByTime(1));
    expect(result.current).toBe(true);
  });

  it("starts the wait again at the next opening", () => {
    const { result, rerender } = renderHook(({ open }) => useShownOnceLoaded(open, false), {
      initialProps: { open: true },
    });
    act(() => void vi.advanceTimersByTime(LOAD_WAIT_MS));
    expect(result.current).toBe(true);

    rerender({ open: false });
    expect(result.current).toBe(false);
    rerender({ open: true });
    expect(result.current).toBe(false);
    act(() => void vi.advanceTimersByTime(LOAD_WAIT_MS));
    expect(result.current).toBe(true);
  });
});
