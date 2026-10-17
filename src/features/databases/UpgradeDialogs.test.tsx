import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, within } from "@testing-library/react";
import * as databasesService from "./databasesService";
import { UpgradeProgressDialog } from "./UpgradeDialogs";
import type { UpgradeProgress } from "./types";

vi.mock("./databasesService");

// 009 contracts/ui.md §2 "Upgrade progress" (009 FR-016). The dialog is mounted
// when the user chooses Upgrade, so its second counts from then. It listens to
// `upgrade:progress` itself (databasesService.onUpgradeProgress).

let sendProgress: (progress: UpgradeProgress) => void = () => {};
const stopListening = vi.fn();

function backingUp(processed: number, total: number, showNow = false): UpgradeProgress {
  return { phase: "backingUp", processed, total, showNow };
}

function upgrading(processed = 0, total = 0): UpgradeProgress {
  return { phase: "upgrading", processed, total, showNow: false };
}

function renderProgress() {
  const onCancel = vi.fn();
  const view = render(<UpgradeProgressDialog name="Main collection" onCancel={onCancel} />);
  return { onCancel, ...view };
}

/** The progress dialog, once it shows. */
function dialog() {
  return screen.getByRole("dialog", { name: "Upgrading “Main collection”" });
}

function send(progress: UpgradeProgress) {
  act(() => sendProgress(progress));
}

describe("UpgradeProgressDialog (009 contracts/ui.md §2, FR-016)", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    stopListening.mockReset();
    vi.mocked(databasesService.onUpgradeProgress)
      .mockReset()
      .mockImplementation((handler) => {
        sendProgress = handler;
        return stopListening;
      });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("shows at once when the first progress says showNow", () => {
    renderProgress();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    send(backingUp(0, 212_000_000, true));

    expect(dialog()).toBeInTheDocument();
  });

  it("otherwise shows only when a second has passed since Upgrade was chosen", () => {
    renderProgress();

    send(backingUp(0, 1_000_000, false));
    act(() => void vi.advanceTimersByTime(400));
    send(backingUp(500_000, 1_000_000, false));
    act(() => void vi.advanceTimersByTime(599));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    act(() => void vi.advanceTimersByTime(1));

    expect(dialog()).toBeInTheDocument();
    expect(within(dialog()).getByText("Saving a backup of “Main collection”…")).toBeInTheDocument();
  });

  it("stays away when what it was waiting for is over before the second", () => {
    const { unmount } = renderProgress();
    send(backingUp(0, 1_000_000, false));

    unmount();
    act(() => void vi.advanceTimersByTime(2000));

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("stops listening when it goes", () => {
    const { unmount } = renderProgress();
    expect(stopListening).not.toHaveBeenCalled();

    unmount();

    expect(stopListening).toHaveBeenCalledTimes(1);
  });

  it("backing up: says so, shows the bytes done on a progress bar, and offers only Cancel", () => {
    renderProgress();

    send(backingUp(53_000_000, 212_000_000, true));

    const shown = dialog();
    expect(within(shown).getByText("Saving a backup of “Main collection”…")).toBeInTheDocument();
    const bar = within(shown).getByRole("progressbar");
    expect(bar).toHaveAttribute("aria-valuenow", "53000000");
    expect(bar).toHaveAttribute("aria-valuemax", "212000000");
    expect(
      within(shown)
        .getAllByRole("button")
        .map((button) => button.textContent),
    ).toEqual(["Cancel"]);
    expect(within(shown).queryByRole("button", { name: "Close" })).not.toBeInTheDocument();
  });

  it("Cancel asks for the stop and shows Stopping… meanwhile", () => {
    const { onCancel } = renderProgress();
    send(backingUp(10, 100, true));

    fireEvent.click(within(dialog()).getByRole("button", { name: "Cancel" }));

    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(dialog()).toHaveTextContent("Stopping…");
    expect(within(dialog()).getByRole("button", { name: "Cancel" })).toBeDisabled();
  });

  it("upgrading: says it can't be stopped, has no buttons and ignores Escape", () => {
    const { onCancel } = renderProgress();
    send(backingUp(100, 100, true));
    send(upgrading());

    const shown = dialog();
    expect(
      within(shown).getByText(
        "Upgrading “Main collection”. This can't be stopped once it has started.",
      ),
    ).toBeInTheDocument();
    expect(within(shown).queryByText(/Saving a backup/)).not.toBeInTheDocument();
    expect(within(shown).queryAllByRole("button")).toHaveLength(0);
    // Nothing is counted yet: the bar is indeterminate.
    expect(within(shown).getByRole("progressbar")).not.toHaveAttribute("aria-valuenow");

    fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape" });

    expect(dialog()).toBeInTheDocument();
    expect(onCancel).not.toHaveBeenCalled();
  });

  it("upgrading: counts the steps done when there is more than one", () => {
    renderProgress();
    send(backingUp(100, 100, true));
    send(upgrading(0, 0));
    expect(within(dialog()).getByRole("progressbar")).not.toHaveAttribute("aria-valuenow");

    send(upgrading(2, 3));

    const bar = within(dialog()).getByRole("progressbar");
    expect(bar).toHaveAttribute("aria-valuenow", "2");
    expect(bar).toHaveAttribute("aria-valuemax", "3");
  });

  it("is not dismissed by Escape while the backup is made either, and has no close button", () => {
    const { onCancel } = renderProgress();
    send(backingUp(10, 100, true));

    fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape" });

    expect(dialog()).toBeInTheDocument();
    expect(onCancel).not.toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: "Close" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Lock now" })).not.toBeInTheDocument();
  });

  it("announces a change of phase through one polite live region", () => {
    renderProgress();
    send(backingUp(10, 100, true));
    const line = screen.getByText("Saving a backup of “Main collection”…");
    const region = line.closest("[aria-live]");
    expect(region).toHaveAttribute("aria-live", "polite");

    send(upgrading());

    // The same region now holds the new line, so a screen reader hears it.
    expect(region).toBeInTheDocument();
    expect(region).toHaveTextContent(
      "Upgrading “Main collection”. This can't be stopped once it has started.",
    );
  });
});
