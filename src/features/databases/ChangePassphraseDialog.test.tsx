import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen } from "@testing-library/react";
import { render, TEST_SESSION_ID } from "../../test/sessionScope";
import userEvent from "@testing-library/user-event";
import { formatBytes } from "../../lib/bytes";
import { CommandFailure } from "../../services/tauriClient";
import { SessionContext } from "../session/sessionStore";
import type { SessionState } from "../session/sessionStore";
import { ChangePassphraseDialog } from "./ChangePassphraseDialog";
import * as databasesService from "./databasesService";
import type { PassphraseChangeProgress } from "./types";

const SCOPE = expect.objectContaining({ id: TEST_SESSION_ID });
vi.mock("./databasesService");

const OLD_FILE = "/home/sam/Documents/HoploDex/.Main collection.hoplodex.old";

let sendProgress: (progress: PassphraseChangeProgress) => void = () => {};

function renderDialog() {
  const session: SessionState = {
    status: null,
    openDatabase: vi.fn(),
    createDatabase: vi.fn(),
    closeDatabase: vi.fn(),
    lockDatabase: vi.fn(),
    refreshStatus: vi.fn().mockResolvedValue(undefined),
    dismissNote: vi.fn(),
    restoreBackup: vi.fn(),
    upgradeDatabase: vi.fn(),
    settingsRequested: false,
    requestSettings: vi.fn(),
    clearSettingsRequest: vi.fn(),
    showAbout: vi.fn(),
  };
  const onOpenChange = vi.fn();
  render(
    <SessionContext.Provider value={session}>
      <ChangePassphraseDialog open onOpenChange={onOpenChange} />
    </SessionContext.Provider>,
  );
  return { session, onOpenChange };
}

/** The change's command, held until the test lets it answer. */
function held() {
  let finish: (value: { oldFileRemoved: boolean; passphraseSaved: boolean }) => void = () => {};
  vi.mocked(databasesService.changePassphrase).mockReturnValue(
    new Promise((resolve) => {
      finish = resolve;
    }),
  );
  return (value = { oldFileRemoved: true, passphraseSaved: false }) =>
    act(async () => finish(value));
}

describe("ChangePassphraseDialog (003 contracts/ui-databases.md §8)", () => {
  beforeEach(() => {
    vi.mocked(databasesService.changePassphrase)
      .mockReset()
      .mockResolvedValue({ oldFileRemoved: true, passphraseSaved: false });
    vi.mocked(databasesService.onPassphraseChangeProgress)
      .mockReset()
      .mockImplementation((handler) => {
        sendProgress = handler;
        return () => {};
      });
  });

  // Amended 2026-11-04 (003 FR-007): both passphrases are typed in the native
  // dialog the command shows, so the page shows nothing until it closes.
  it("starts the change at once, once, and shows nothing while the passphrases are asked for", async () => {
    held();
    renderDialog();

    await act(async () => {});
    expect(databasesService.changePassphrase).toHaveBeenCalledExactlyOnceWith(SCOPE);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/passphrase/i)).not.toBeInTheDocument();
  });

  it("closes, with nothing shown, when the passphrase dialog is cancelled", async () => {
    vi.mocked(databasesService.changePassphrase).mockRejectedValue(
      new CommandFailure({ code: "PASSPHRASE_CANCELLED", message: "No passphrase was entered." }),
    );
    const { onOpenChange } = renderDialog();

    await act(async () => {});
    expect(onOpenChange).toHaveBeenCalledWith(false);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("shows each phase once the copy begins, and can't be dismissed while it runs", async () => {
    const finish = held();
    const { onOpenChange } = renderDialog();
    const user = userEvent.setup();

    for (const [phase, text] of [
      ["copying", "Making a copy with the new passphrase…"],
      ["checking", "Checking the new copy…"],
      ["replacing", "Replacing the database…"],
    ] as const) {
      act(() => sendProgress({ phase, processed: 10, total: phase === "copying" ? 100 : 0 }));
      expect(await screen.findByText(text)).toBeInTheDocument();
    }
    expect(
      screen.getByRole("progressbar", { name: "Passphrase change progress" }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Close" })).not.toBeInTheDocument();
    await user.keyboard("{Escape}");
    expect(onOpenChange).not.toHaveBeenCalled();

    await finish();
    expect(screen.getByRole("button", { name: "Done" })).toBeInTheDocument();
  });

  it("says the change is done, that deletion is best effort, and what still opens with the old one", async () => {
    const { session, onOpenChange } = renderDialog();
    const user = userEvent.setup();

    expect(await screen.findByRole("status")).toHaveTextContent(
      "The passphrase has been changed. The previous file was deleted securely, as far as this computer allows (see About databases and security). Backups and copies made before now still open with the old passphrase.",
    );
    expect(session.refreshStatus).toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "About databases and security" }));
    expect(
      await screen.findByRole("dialog", { name: "About databases and security" }),
    ).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await user.click(screen.getByRole("button", { name: "Done" }));
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("says where the previous file is when it couldn't be deleted", async () => {
    vi.mocked(databasesService.changePassphrase).mockResolvedValue({
      oldFileRemoved: false,
      oldFilePath: OLD_FILE,
      passphraseSaved: false,
    });
    renderDialog();

    expect(
      await screen.findByText(
        `The previous file could not be deleted. It is at ${OLD_FILE}, and it opens with the old passphrase.`,
        { exact: false },
      ),
    ).toBeInTheDocument();
    expect(screen.queryByText(/was deleted securely/)).not.toBeInTheDocument();
  });

  it("says how much space it needs, and can try again", async () => {
    vi.mocked(databasesService.changePassphrase).mockRejectedValueOnce(
      new CommandFailure({
        code: "INSUFFICIENT_SPACE",
        message: "There isn't enough free space.",
        details: { bytesNeeded: 222_600_000, bytesAvailable: 90_000_000, path: "/home/sam" },
      }),
    );
    renderDialog();
    const user = userEvent.setup();

    expect(
      await screen.findByText(
        `Changing the passphrase needs ${formatBytes(222_600_000)} free on the database's drive; ${formatBytes(90_000_000)} is free.`,
      ),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Try again…" }));
    expect(await screen.findByRole("status")).toHaveTextContent("The passphrase has been changed.");
    expect(databasesService.changePassphrase).toHaveBeenCalledTimes(2);
  });

  it("says a stopped change changed nothing", async () => {
    vi.mocked(databasesService.changePassphrase).mockRejectedValue(
      new CommandFailure({
        code: "OPERATION_STOPPED",
        message: "The operation was stopped before it finished.",
        details: { operation: "passphraseChange" },
      }),
    );
    renderDialog();

    expect(
      await screen.findByText(
        "The passphrase change was stopped. The database still opens with its current passphrase.",
      ),
    ).toBeInTheDocument();
  });
});
