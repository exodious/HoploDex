import { afterEach, describe, expect, it, vi } from "vitest";
import { useContext } from "react";
import { act, render, screen } from "@testing-library/react";
import { ToastProvider } from "./Toast";
import { ToastContext } from "./toastContext";

function Notifier() {
  const notify = useContext(ToastContext);
  return (
    <button type="button" onClick={() => notify("Saved.")}>
      Notify
    </button>
  );
}

describe("ToastProvider", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("dismisses a toast after its time", () => {
    vi.useFakeTimers();
    render(
      <ToastProvider>
        <Notifier />
      </ToastProvider>,
    );
    act(() => screen.getByRole("button", { name: "Notify" }).click());
    expect(screen.getByText("Saved.")).toBeInTheDocument();

    act(() => vi.advanceTimersByTime(4000));

    expect(screen.queryByText("Saved.")).not.toBeInTheDocument();
    expect(vi.getTimerCount()).toBe(0);
  });

  // A timer left behind fired after the test environment was gone and failed
  // the run ("window is not defined", from SessionProvider.test.tsx).
  it("leaves no timer behind once it is unmounted", () => {
    vi.useFakeTimers();
    const { unmount } = render(
      <ToastProvider>
        <Notifier />
      </ToastProvider>,
    );
    act(() => screen.getByRole("button", { name: "Notify" }).click());
    expect(vi.getTimerCount()).toBe(1);

    unmount();

    expect(vi.getTimerCount()).toBe(0);
  });
});
