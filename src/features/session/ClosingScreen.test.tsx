import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { BackupProgress, CloseReason, DatabaseStatus } from "../databases/types";
import * as databasesService from "../databases/databasesService";
import { ClosingScreen } from "./ClosingScreen";
import { SessionProvider } from "./SessionProvider";
import * as sessionService from "./sessionService";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("../databases/databasesService");
vi.mock("./sessionService");

const PATH = "/home/sam/Documents/HoploDex/Main collection.hoplodex";

const status: DatabaseStatus = {
  path: PATH,
  name: "Main collection",
  passphraseSaved: false,
  settings: {
    backups: {
      enabled: true,
      keepCount: 5,
      location: {
        kind: "default",
        path: "/home/sam/Documents/HoploDex/HoploDex backups",
        available: true,
      },
    },
    lock: { idleEnabled: true, idleMinutes: 10, onScreenLock: false },
  },
  pendingChanges: null,
  notes: {
    diskEncryption: false,
    openedBackup: null,
    restoredWithPassphraseOf: null,
    damagedFileKeptAt: null,
  },
};

let sendProgress: (progress: BackupProgress) => void = () => {};
let sendClosing: (closing: { reason: CloseReason }) => void = () => {};

beforeEach(() => {
  vi.mocked(sessionService.onBackupProgress)
    .mockReset()
    .mockImplementation((handler) => {
      sendProgress = handler;
      return () => {};
    });
  vi.mocked(sessionService.onSessionClosing)
    .mockReset()
    .mockImplementation((handler) => {
      sendClosing = handler;
      return () => {};
    });
  vi.mocked(sessionService.onSessionClosed)
    .mockReset()
    .mockReturnValue(() => {});
  vi.mocked(sessionService.onQuitRequested)
    .mockReset()
    .mockReturnValue(() => {});
  vi.mocked(sessionService.skipBackup).mockReset().mockResolvedValue(undefined);
});

afterEach(() => {
  vi.useRealTimers();
});

describe("ClosingScreen (contracts/ui-databases.md §5, SC-005)", () => {
  it("replaces the collection with the closing screen when a close starts", async () => {
    vi.mocked(sessionService.getDatabaseStatus).mockResolvedValue(status);
    vi.mocked(databasesService.getChooserState).mockReset();
    render(
      <SessionProvider>
        <p>Collection shell</p>
      </SessionProvider>,
    );
    expect(await screen.findByText("Collection shell")).toBeInTheDocument();

    act(() => sendClosing({ reason: "closed" }));

    expect(screen.getByRole("heading", { name: "Closing Main collection…" })).toBeInTheDocument();
    expect(screen.queryByText("Collection shell")).not.toBeInTheDocument();
  });

  it("shows the backup bar at once when the copy is expected to be long", () => {
    render(<ClosingScreen name="Main collection" />);

    act(() => sendProgress({ processed: 0, total: 300_000_000, showNow: true }));

    expect(screen.getByText("Backing up Main collection…")).toBeInTheDocument();
    const bar = screen.getByRole("progressbar", { name: "Backup progress" });
    expect(bar).toHaveAttribute("aria-valuenow", "0");
    act(() => sendProgress({ processed: 150_000_000, total: 300_000_000, showNow: true }));
    expect(bar).toHaveAttribute("aria-valuenow", "150000000");
  });

  it("shows the bar for a short copy only if the close is still running a second later", () => {
    vi.useFakeTimers();
    render(<ClosingScreen name="Main collection" />);

    act(() => sendProgress({ processed: 0, total: 4_000_000, showNow: false }));
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Closing Main collection…" })).toBeInTheDocument();

    act(() => vi.advanceTimersByTime(999));
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
    act(() => vi.advanceTimersByTime(1));
    expect(screen.getByRole("progressbar", { name: "Backup progress" })).toBeInTheDocument();
  });

  it("skips the backup, saying the changes will be backed up next time", async () => {
    const user = userEvent.setup();
    render(<ClosingScreen name="Main collection" />);
    act(() => sendProgress({ processed: 0, total: 300_000_000, showNow: true }));

    expect(screen.getByText("Its changes will be backed up next time.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Skip this backup" }));

    expect(sessionService.skipBackup).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("button", { name: "Skipping…" })).toBeDisabled();
  });
});
