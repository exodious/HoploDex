import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import * as databasesService from "../databases/databasesService";
import type { DatabaseStatus } from "../databases/types";
import { SessionProvider } from "./SessionProvider";
import { ApplicationFault, FaultBoundary } from "./FaultScreen";
import { createSessionScope } from "./sessionScope";
import * as sessionService from "./sessionService";

/** What the session scope of the open (session id 1) looks like to a mocked service. */
const SCOPE = expect.objectContaining({ id: 1 });
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("../databases/databasesService");
vi.mock("./sessionService");

const status: DatabaseStatus = {
  sessionId: 1,
  path: "/home/sam/Documents/HoploDex/Shared collection.hoplodex",
  name: "Shared collection",
  passphraseSaved: false,
  keyringAvailable: true,
  screenLockSupported: true,
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
    upgraded: null,
  },
};

function Broken(): never {
  throw new Error("a resumed form that couldn't render");
}

describe("A screen that fails to render", () => {
  let quitRequested: () => void;

  beforeEach(() => {
    // React reports the caught error on the console; that is expected here.
    vi.spyOn(console, "error").mockImplementation(() => {});
    vi.mocked(sessionService.getDatabaseStatus).mockReset().mockResolvedValue(status);
    vi.mocked(sessionService.closeDatabase).mockReset().mockResolvedValue({
      backup: "notAttempted",
    });
    vi.mocked(sessionService.quitApplication).mockReset().mockResolvedValue(undefined);
    // The chooser the close leads to.
    vi.mocked(databasesService.getChooserState)
      .mockReset()
      .mockResolvedValue({
        recent: [],
        selectedPath: null,
        keyringAvailable: true,
        screenLockSupported: true,
        suggested: { folder: "/tmp", name: "Mine" },
        notices: [],
      });
    vi.mocked(sessionService.onQuitRequested)
      .mockReset()
      .mockImplementation((handler) => {
        quitRequested = handler;
        return () => {};
      });
  });
  afterEach(() => vi.restoreAllMocks());

  it("leaves the collection's database able to close, and the window able to quit", async () => {
    const user = userEvent.setup();
    render(
      <SessionProvider>
        <Broken />
      </SessionProvider>,
    );

    expect(
      await screen.findByRole("heading", { name: "“Shared collection” couldn't be shown" }),
    ).toBeInTheDocument();
    const close = screen.getByRole("button", { name: "Close the database" });
    expect(close).toHaveFocus();

    act(() => quitRequested());
    await waitFor(() => expect(sessionService.quitApplication).toHaveBeenCalledTimes(1));

    await user.click(close);
    expect(sessionService.closeDatabase).toHaveBeenCalledWith(SCOPE, "closed");
  });

  it("quits from the window's close button when the session itself failed", async () => {
    const user = userEvent.setup();
    render(
      <FaultBoundary fallback={<ApplicationFault />}>
        <Broken />
      </FaultBoundary>,
    );

    expect(screen.getByRole("heading", { name: "HoploDex stopped working" })).toBeInTheDocument();
    act(() => quitRequested());
    expect(sessionService.quitApplication).toHaveBeenCalledTimes(1);

    await user.click(screen.getByRole("button", { name: "Quit HoploDex" }));
    expect(sessionService.quitApplication).toHaveBeenCalledTimes(2);
  });

  // 008 tasks.md T040 (008 US1-4), 008 research.md §6: a render
  // that fails while pending changes are being resumed means their form can't
  // open, so the scope is told; otherwise nothing is.
  function scopeResuming(resuming: boolean) {
    const scope = createSessionScope(1);
    scope.resumed = resuming
      ? {
          draft: {
            formVersion: 1,
            kind: "policy",
            mode: "edit",
            targetId: 7,
            label: "Collector Floater (edit)",
            values: {},
          },
          state: "opening",
        }
      : null;
    scope.resumeFailed = vi.fn().mockResolvedValue(undefined);
    return scope;
  }

  it("reports notOpened to the scope when a render fails while pending changes are resuming", () => {
    const scope = scopeResuming(true);
    render(
      <FaultBoundary scope={scope} fallback={<p>Fault</p>}>
        <Broken />
      </FaultBoundary>,
    );

    expect(screen.getByText("Fault")).toBeInTheDocument();
    expect(scope.resumeFailed).toHaveBeenCalledTimes(1);
  });

  it("reports nothing to the scope when no pending changes are resuming", () => {
    const scope = scopeResuming(false);
    render(
      <FaultBoundary scope={scope} fallback={<p>Fault</p>}>
        <Broken />
      </FaultBoundary>,
    );

    expect(screen.getByText("Fault")).toBeInTheDocument();
    expect(scope.resumeFailed).not.toHaveBeenCalled();
  });
});
