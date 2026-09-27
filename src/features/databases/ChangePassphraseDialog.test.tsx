import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { PASSPHRASE_CHECK_DELAY_MS } from "../../components";
import { CLEAR_PASSPHRASE_FIELDS } from "../../components/PassphraseField";
import { formatBytes } from "../../lib/bytes";
import { CommandFailure } from "../../services/tauriClient";
import { SessionContext } from "../session/sessionStore";
import type { SessionState } from "../session/sessionStore";
import { ChangePassphraseDialog } from "./ChangePassphraseDialog";
import * as databasesService from "./databasesService";
import type { PassphraseChangeProgress } from "./types";

vi.mock("./databasesService");

/** The backend's events, sent by the test. */
const events = vi.hoisted(() => {
  const listeners = new Map<string, Set<(event: { payload: unknown }) => void>>();
  return {
    listen: (name: string, handler: (event: { payload: unknown }) => void) => {
      const forName = listeners.get(name) ?? new Set();
      forName.add(handler);
      listeners.set(name, forName);
      return Promise.resolve(() => forName.delete(handler));
    },
    send: (name: string) => listeners.get(name)?.forEach((handler) => handler({ payload: {} })),
  };
});
vi.mock("@tauri-apps/api/event", () => ({ listen: events.listen }));

const CURRENT = "correct horse battery staple";
const NEW = "a much longer passphrase of several words";
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
    settingsRequested: false,
    requestSettings: vi.fn(),
    clearSettingsRequest: vi.fn(),
  };
  const onOpenChange = vi.fn();
  render(
    <SessionContext.Provider value={session}>
      <ChangePassphraseDialog open onOpenChange={onOpenChange} name="Main collection" />
    </SessionContext.Provider>,
  );
  return { session, onOpenChange };
}

async function type(current: string, next: string, confirmation = next) {
  const user = userEvent.setup();
  if (current) await user.type(screen.getByLabelText("Current passphrase"), current);
  if (next) await user.type(screen.getByLabelText("New passphrase"), next);
  if (confirmation) await user.type(screen.getByLabelText("Confirm new passphrase"), confirmation);
  return user;
}

async function fill(current: string, next: string, confirmation = next) {
  const user = await type(current, next, confirmation);
  await user.click(screen.getByRole("button", { name: "Change passphrase" }));
  return user;
}

describe("ChangePassphraseDialog (contracts/ui-databases.md §8)", () => {
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

  it("asks for the current passphrase and the new one twice, with a strength hint", async () => {
    renderDialog();

    expect(screen.getByRole("dialog", { name: "Change the passphrase of Main collection" }));
    expect(screen.getByLabelText("Current passphrase")).toHaveAttribute(
      "autocomplete",
      "current-password",
    );
    expect(screen.getByLabelText("New passphrase")).toHaveAttribute("autocomplete", "new-password");
    expect(screen.getByLabelText("Confirm new passphrase")).toBeInTheDocument();

    await userEvent.setup().type(screen.getByLabelText("New passphrase"), "password");
    expect(await screen.findByRole("meter", { name: "Passphrase strength" })).toBeInTheDocument();
    expect(screen.getAllByRole("meter")).toHaveLength(1);
  });

  it("sends both passphrases once", async () => {
    renderDialog();

    await fill(CURRENT, NEW);

    expect(databasesService.changePassphrase).toHaveBeenCalledWith(CURRENT, NEW);
    expect(databasesService.changePassphrase).toHaveBeenCalledTimes(1);
  });

  it.each([
    ["", NEW, NEW, null],
    [CURRENT, "too short", "too short", ["New passphrase", "Use at least 12 characters."]],
    [CURRENT, NEW, `${NEW}!`, ["Confirm new passphrase", "The passphrases don't match."]],
    [CURRENT, "different words", "different", null],
    [
      CURRENT,
      CURRENT,
      CURRENT,
      ["New passphrase", "Choose a passphrase different from the current one."],
    ],
  ])(
    "checks the fields as they are typed, and waits until all is well (%#)",
    async (current, next, confirmation, problem) => {
      renderDialog();

      await type(current, next, confirmation);

      if (problem) {
        const [field, message] = problem;
        expect(await screen.findByText(message)).toBeInTheDocument();
        expect(screen.getByLabelText(field)).toHaveAccessibleDescription(message);
      } else {
        // An empty field, or a confirmation still on its way, isn't an error.
        await act(() => new Promise((resolve) => setTimeout(resolve, PASSPHRASE_CHECK_DELAY_MS)));
        expect(screen.queryByRole("alert")).not.toBeInTheDocument();
      }
      expect(screen.getByRole("button", { name: "Change passphrase" })).toBeDisabled();
      // Nothing is cleared: the fields are only read.
      expect(screen.getByLabelText("New passphrase")).toHaveValue(next);
      expect(databasesService.changePassphrase).not.toHaveBeenCalled();
    },
  );

  it("waits for a pause before a problem shows, and clears it the moment it is put right", async () => {
    renderDialog();
    const user = await type(CURRENT, NEW, "a much longer passphrase of several wordz");

    // Typed, and not yet paused.
    expect(screen.queryByText("The passphrases don't match.")).not.toBeInTheDocument();
    expect(await screen.findByText("The passphrases don't match.")).toBeInTheDocument();

    await user.type(screen.getByLabelText("Confirm new passphrase"), "{Backspace}s");
    expect(screen.queryByText("The passphrases don't match.")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Change passphrase" })).toBeEnabled();
  });

  it("forgets what it found when the fields are emptied at a lock or sleep (FR-007)", async () => {
    renderDialog();
    await type(CURRENT, NEW);
    expect(screen.getByRole("button", { name: "Change passphrase" })).toBeEnabled();
    // Subscribed once the effects have run.
    await act(async () => {});

    await act(async () => {
      events.send(CLEAR_PASSPHRASE_FIELDS);
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    expect(screen.getByLabelText("New passphrase")).toHaveValue("");
    expect(screen.getByRole("button", { name: "Change passphrase" })).toBeDisabled();
  });

  it("shows each phase in place of the form, and can't be dismissed while it runs", async () => {
    let finish: (value: { oldFileRemoved: boolean; passphraseSaved: boolean }) => void = () => {};
    vi.mocked(databasesService.changePassphrase).mockReturnValue(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    const { onOpenChange } = renderDialog();

    const user = await fill(CURRENT, NEW);

    expect(screen.queryByLabelText("Current passphrase")).not.toBeInTheDocument();
    for (const [phase, text] of [
      ["copying", "Making a copy with the new passphrase…"],
      ["checking", "Checking the new copy…"],
      ["replacing", "Replacing the database…"],
    ] as const) {
      act(() => sendProgress({ phase, processed: 10, total: phase === "copying" ? 100 : 0 }));
      expect(screen.getByText(text)).toBeInTheDocument();
    }
    expect(
      screen.getByRole("progressbar", { name: "Passphrase change progress" }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Close" })).not.toBeInTheDocument();
    await user.keyboard("{Escape}");
    expect(onOpenChange).not.toHaveBeenCalled();

    await act(async () => finish({ oldFileRemoved: true, passphraseSaved: false }));
    expect(screen.getByRole("button", { name: "Done" })).toBeInTheDocument();
  });

  it("says the change is done, that deletion is best effort, and what still opens with the old one", async () => {
    const { session, onOpenChange } = renderDialog();

    const user = await fill(CURRENT, NEW);

    expect(await screen.findByRole("status")).toHaveTextContent(
      "The passphrase of Main collection has been changed. The previous file was deleted securely, as far as this computer allows (see About databases and security). Backups and copies made before now still open with the old passphrase.",
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

    await fill(CURRENT, NEW);

    expect(
      await screen.findByText(
        `The previous file could not be deleted. It is at ${OLD_FILE}, and it opens with the old passphrase.`,
        { exact: false },
      ),
    ).toBeInTheDocument();
    expect(screen.queryByText(/was deleted securely/)).not.toBeInTheDocument();
  });

  it("says how much space it needs", async () => {
    vi.mocked(databasesService.changePassphrase).mockRejectedValue(
      new CommandFailure({
        code: "INSUFFICIENT_SPACE",
        message: "There isn't enough free space.",
        details: { bytesNeeded: 222_600_000, bytesAvailable: 90_000_000, path: "/home/sam" },
      }),
    );
    renderDialog();

    await fill(CURRENT, NEW);

    expect(
      await screen.findByText(
        `Changing the passphrase needs ${formatBytes(222_600_000)} free on the database's drive; ${formatBytes(90_000_000)} is free.`,
      ),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Current passphrase")).toHaveValue("");
  });

  it("puts a wrong current passphrase on its field", async () => {
    vi.mocked(databasesService.changePassphrase).mockRejectedValue(
      new CommandFailure({
        code: "PASSPHRASE_INCORRECT",
        message: "The passphrase is incorrect.",
        fieldErrors: { currentPassphrase: "That isn't the current passphrase." },
      }),
    );
    renderDialog();

    await fill("not the passphrase at all", NEW);

    expect(await screen.findByText("That isn't the current passphrase.")).toBeInTheDocument();
    expect(screen.getByLabelText("Current passphrase")).toHaveAccessibleDescription(
      expect.stringContaining("That isn't the current passphrase."),
    );
    expect(screen.getByLabelText("Current passphrase")).toHaveFocus();
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

    await fill(CURRENT, NEW);

    expect(
      await screen.findByText(
        "The passphrase change was stopped. Main collection still opens with its current passphrase.",
      ),
    ).toBeInTheDocument();
  });
});
