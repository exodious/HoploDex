import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { renderHook } from "@testing-library/react";
import * as sessionService from "./sessionService";
import { pauseIdleForFileInput, useIdleActivity, withIdlePaused } from "./useIdleActivity";

vi.mock("./sessionService");

describe("useIdleActivity (research.md §15)", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.mocked(sessionService.noteActivity).mockReset().mockResolvedValue(undefined);
  });
  afterEach(() => vi.useRealTimers());

  it.each(["keydown", "pointerdown", "pointermove", "wheel", "touchstart"])(
    "reports %s as input",
    (event) => {
      renderHook(() => useIdleActivity(true));

      window.dispatchEvent(new Event(event));

      expect(sessionService.noteActivity).toHaveBeenCalledTimes(1);
    },
  );

  it("reports at most once a second, on the leading and the trailing edge", () => {
    renderHook(() => useIdleActivity(true));

    window.dispatchEvent(new Event("pointermove"));
    expect(sessionService.noteActivity).toHaveBeenCalledTimes(1);
    for (let i = 0; i < 20; i++) {
      vi.advanceTimersByTime(40);
      window.dispatchEvent(new Event("pointermove"));
    }
    expect(sessionService.noteActivity).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(200);
    expect(sessionService.noteActivity).toHaveBeenCalledTimes(2);

    vi.advanceTimersByTime(5000);
    expect(sessionService.noteActivity).toHaveBeenCalledTimes(2);
    window.dispatchEvent(new Event("keydown"));
    expect(sessionService.noteActivity).toHaveBeenCalledTimes(3);
  });

  it("reports nothing while no database is open, and stops when unmounted", () => {
    const { rerender, unmount } = renderHook(({ active }) => useIdleActivity(active), {
      initialProps: { active: false },
    });
    window.dispatchEvent(new Event("keydown"));
    expect(sessionService.noteActivity).not.toHaveBeenCalled();

    rerender({ active: true });
    window.dispatchEvent(new Event("keydown"));
    window.dispatchEvent(new Event("keydown"));
    unmount();
    vi.advanceTimersByTime(2000);

    expect(sessionService.noteActivity).toHaveBeenCalledTimes(1);
  });
});

describe("withIdlePaused", () => {
  beforeEach(() => {
    vi.mocked(sessionService.setIdlePaused).mockReset().mockResolvedValue(undefined);
  });

  it("pauses the idle clock around a native dialog", async () => {
    const calls: string[] = [];
    vi.mocked(sessionService.setIdlePaused).mockImplementation(async (paused) => {
      calls.push(`paused ${paused}`);
    });

    const chosen = await withIdlePaused(async () => {
      calls.push("dialog");
      return "/home/sam/export.csv";
    });

    expect(chosen).toBe("/home/sam/export.csv");
    expect(calls).toEqual(["paused true", "dialog", "paused false"]);
  });

  it("resumes it even when the dialog fails", async () => {
    await expect(
      withIdlePaused(async () => {
        throw new Error("no dialog");
      }),
    ).rejects.toThrow("no dialog");

    expect(vi.mocked(sessionService.setIdlePaused).mock.calls).toEqual([[true], [false]]);
  });

  it("still opens the dialog when the pause can't be sent", async () => {
    vi.mocked(sessionService.setIdlePaused).mockRejectedValue(new Error("closed"));

    await expect(withIdlePaused(async () => "chosen")).resolves.toBe("chosen");
  });
});

describe("pauseIdleForFileInput", () => {
  let input: HTMLInputElement;

  beforeEach(() => {
    vi.mocked(sessionService.setIdlePaused).mockReset().mockResolvedValue(undefined);
    input = document.createElement("input");
    input.type = "file";
  });

  const pauses = () => vi.mocked(sessionService.setIdlePaused).mock.calls.map(([paused]) => paused);

  it.each(["change", "cancel"])("pauses on click and resumes on %s", (end) => {
    pauseIdleForFileInput(input);

    input.dispatchEvent(new Event("click"));
    expect(pauses()).toEqual([true]);
    input.dispatchEvent(new Event(end));

    expect(pauses()).toEqual([true, false]);
  });

  it("resumes on the window's next focus when the input says nothing", () => {
    pauseIdleForFileInput(input);

    input.dispatchEvent(new Event("click"));
    window.dispatchEvent(new Event("focus"));

    expect(pauses()).toEqual([true, false]);
  });

  it("resumes only once per click", () => {
    pauseIdleForFileInput(input);

    input.dispatchEvent(new Event("click"));
    window.dispatchEvent(new Event("focus"));
    input.dispatchEvent(new Event("change"));
    window.dispatchEvent(new Event("focus"));

    expect(pauses()).toEqual([true, false]);
  });

  it("stops listening once detached", () => {
    const detach = pauseIdleForFileInput(input);
    input.dispatchEvent(new Event("click"));

    detach();
    window.dispatchEvent(new Event("focus"));
    input.dispatchEvent(new Event("click"));

    expect(pauses()).toEqual([true]);
  });
});
