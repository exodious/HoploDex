import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { formatBytes } from "../../lib/bytes";
import { CommandFailure } from "../../services/tauriClient";
import { ExistingBackupsDialog } from "./ExistingBackupsDialog";
import * as databasesService from "./databasesService";
import type {
  BackupProgress,
  BackupSettingsSaved,
  CountProgress,
  ExistingBackupsChoice,
} from "./types";

vi.mock("./databasesService");

const OLD = "/home/sam/Documents/HoploDex/HoploDex backups";
const NEW = "/media/usb/Backups";
const QUESTION = { folder: OLD, count: 3, totalBytes: 636_000_000 };

const SAVED: BackupSettingsSaved = {
  settings: {
    backups: {
      enabled: true,
      keepCount: 5,
      location: { kind: "custom", path: NEW, available: true },
    },
    lock: { idleEnabled: true, idleMinutes: 10, onScreenLock: false },
  },
  existingBackups: { action: "move", movedCount: 3, leftBehind: null },
};

let moveProgress: (progress: BackupProgress) => void = () => {};
let deleteProgress: (progress: CountProgress) => void = () => {};

function renderQuestion(
  send = vi.fn<(choice: ExistingBackupsChoice) => Promise<BackupSettingsSaved>>(),
) {
  const props = { send, onCancel: vi.fn(), onDone: vi.fn() };
  render(
    <ExistingBackupsDialog question={QUESTION} newFolder={NEW} name="Main collection" {...props} />,
  );
  return props;
}

const question = () => screen.getByRole("dialog", { name: "Backups at the old location" });
const changeLocation = () => screen.getByRole("button", { name: "Change location" });

/** A send that waits until `finish` is called. */
function pendingSend() {
  let finish: (saved: BackupSettingsSaved) => void = () => {};
  const send = vi.fn(
    () =>
      new Promise<BackupSettingsSaved>((resolve) => {
        finish = resolve;
      }),
  );
  return { send, finish: (saved: BackupSettingsSaved) => finish(saved) };
}

describe("ExistingBackupsDialog (contracts/ui-databases.md §7, FR-026, US3-4a)", () => {
  beforeEach(() => {
    vi.mocked(databasesService.onBackupsMoveProgress)
      .mockReset()
      .mockImplementation((handler) => {
        moveProgress = handler;
        return () => {};
      });
    vi.mocked(databasesService.onBackupsDeleteProgress)
      .mockReset()
      .mockImplementation((handler) => {
        deleteProgress = handler;
        return () => {};
      });
  });

  it("asks what to do with the backups, with their count, size and folder, Move first and chosen", () => {
    renderQuestion();

    expect(question()).toHaveTextContent(
      `3 backups of the database (${formatBytes(636_000_000)}) are in ${OLD}.`,
    );
    const choices = within(question()).getByRole("radiogroup", {
      name: "What should happen to them?",
    });
    const radios = within(choices).getAllByRole("radio");
    expect(
      radios.map(
        (radio) => radio.closest("label")?.querySelector(".hd-choice__label")?.textContent,
      ),
    ).toEqual(["Move them to the new location", "Leave them where they are", "Delete them"]);
    expect(radios[0]).toBeChecked();
    expect(within(choices).getByText(`They'll be in ${NEW}.`)).toBeInTheDocument();
    expect(
      within(choices).getByText(
        `HoploDex will no longer list, restore or delete them. They still open directly as a database, and choosing ${OLD} again makes them this database's backups again.`,
      ),
    ).toBeInTheDocument();
    expect(
      within(choices).getByText("They're deleted securely, as far as this computer allows."),
    ).toBeInTheDocument();
  });

  it("moves them, then reports what was done", async () => {
    const user = userEvent.setup();
    const send = vi.fn().mockResolvedValue(SAVED);
    const props = renderQuestion(send);

    await user.click(changeLocation());

    expect(send).toHaveBeenCalledWith("move");
    await waitFor(() => expect(props.onDone).toHaveBeenCalledWith(SAVED));
  });

  it("leaves them", async () => {
    const user = userEvent.setup();
    const send = vi.fn().mockResolvedValue({ ...SAVED, existingBackups: { action: "leave" } });
    renderQuestion(send);

    await user.click(screen.getByRole("radio", { name: /Leave them where they are/ }));
    await user.click(changeLocation());

    expect(send).toHaveBeenCalledWith("leave");
  });

  it("deletes them only after the destructive confirmation, and cancelling it returns to the question", async () => {
    const user = userEvent.setup();
    const send = vi
      .fn()
      .mockResolvedValue({ ...SAVED, existingBackups: { action: "delete", deletedCount: 3 } });
    renderQuestion(send);

    await user.click(screen.getByRole("radio", { name: /Delete them/ }));
    await user.click(changeLocation());
    let confirm = screen.getByRole("alertdialog", {
      name: "Delete all 3 backups of “Main collection”?",
    });
    expect(confirm).toHaveTextContent(
      "They are deleted securely where this computer supports it. This can't be undone.",
    );
    await user.click(within(confirm).getByRole("button", { name: "Cancel" }));
    expect(send).not.toHaveBeenCalled();
    expect(screen.getByRole("radio", { name: /Delete them/ })).toBeChecked();

    await user.click(changeLocation());
    confirm = screen.getByRole("alertdialog");
    await user.click(within(confirm).getByRole("button", { name: "Delete all backups" }));
    expect(send).toHaveBeenCalledWith("delete");
  });

  it("cancels without sending anything", async () => {
    const user = userEvent.setup();
    const props = renderQuestion();

    await user.click(screen.getByRole("button", { name: "Cancel" }));

    expect(props.send).not.toHaveBeenCalled();
    expect(props.onCancel).toHaveBeenCalled();
  });

  it("shows the move's progress at once when it will be long, and can't be dismissed or locked meanwhile", async () => {
    const user = userEvent.setup();
    const { send, finish } = pendingSend();
    const props = renderQuestion(send);

    await user.click(changeLocation());
    expect(question()).toHaveTextContent("Moving the backups…");
    expect(within(question()).queryByRole("radiogroup")).not.toBeInTheDocument();
    expect(within(question()).queryByRole("button", { name: "Close" })).not.toBeInTheDocument();
    expect(within(question()).queryByRole("button", { name: "Lock now" })).not.toBeInTheDocument();
    act(() => moveProgress({ processed: 0, total: 1_272_000_000, showNow: true }));
    expect(
      within(question()).getByRole("progressbar", { name: "Moving the backups" }),
    ).toBeInTheDocument();
    await user.keyboard("{Escape}");
    expect(props.onCancel).not.toHaveBeenCalled();

    act(() => finish(SAVED));
    await waitFor(() => expect(props.onDone).toHaveBeenCalled());
  });

  it("shows a short move's progress only after a second", async () => {
    const user = userEvent.setup();
    const { send } = pendingSend();
    renderQuestion(send);

    await user.click(changeLocation());
    act(() => moveProgress({ processed: 0, total: 1_000, showNow: false }));
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
    expect(
      await screen.findByRole("progressbar", { name: "Moving the backups" }, { timeout: 2000 }),
    ).toBeInTheDocument();
  });

  it("counts the deletion in backups", async () => {
    const user = userEvent.setup();
    const { send } = pendingSend();
    renderQuestion(send);

    await user.click(screen.getByRole("radio", { name: /Delete them/ }));
    await user.click(changeLocation());
    await user.click(screen.getByRole("button", { name: "Delete all backups" }));
    expect(question()).toHaveTextContent("Deleting the backups…");
    act(() => deleteProgress({ processed: 1, total: 3 }));
    const bar = await screen.findByRole(
      "progressbar",
      { name: "Deleting the backups" },
      { timeout: 2000 },
    );
    expect(bar).toHaveAttribute("aria-valuenow", "1");
    expect(question()).toHaveTextContent("1 of 3 backups");
  });

  it("says why a move was refused, with nothing changed and the question still open", async () => {
    const user = userEvent.setup();
    const send = vi.fn().mockRejectedValueOnce(
      new CommandFailure({
        code: "INSUFFICIENT_SPACE",
        message: "There isn't enough free space.",
        details: { bytesNeeded: 667_800_000, bytesAvailable: 10_000_000, path: NEW },
      }),
    );
    renderQuestion(send);

    await user.click(changeLocation());

    expect(await within(question()).findByRole("alert")).toHaveTextContent(
      `Moving the backups needs ${formatBytes(667_800_000)} free in ${NEW}; ${formatBytes(10_000_000)} is free. Nothing has been changed.`,
    );
    expect(within(question()).getByRole("radiogroup")).toBeInTheDocument();

    send.mockRejectedValueOnce(
      new CommandFailure({
        code: "BACKUP_LOCATION_UNAVAILABLE",
        message: "The backup folder can't be used.",
        details: { path: NEW, reason: "missing" },
      }),
    );
    await user.click(changeLocation());
    await waitFor(() =>
      expect(within(question()).getByRole("alert")).toHaveTextContent(
        `${NEW} isn't available. Nothing has been changed.`,
      ),
    );
  });

  it("says which backups couldn't be deleted, and Close returns to the settings", async () => {
    const user = userEvent.setup();
    const stuck = `${OLD}/Main collection 2026-09-25 143005 3fa2c9d1.hoplodex`;
    const send = vi.fn().mockRejectedValue(
      new CommandFailure({
        code: "BACKUPS_NOT_ALL_DELETED",
        message: "Some backups couldn't be deleted.",
        details: { deletedCount: 2, failedPaths: [stuck] },
      }),
    );
    const props = renderQuestion(send);

    await user.click(screen.getByRole("radio", { name: /Delete them/ }));
    await user.click(changeLocation());
    await user.click(screen.getByRole("button", { name: "Delete all backups" }));

    expect(
      await within(question()).findByText(
        "1 backup couldn't be deleted, so the backup location wasn't changed. It is still this database's backup:",
      ),
    ).toBeInTheDocument();
    expect(within(question()).getByText(stuck)).toBeInTheDocument();
    const footer = question().querySelector("footer") as HTMLElement;
    expect(
      within(footer)
        .getAllByRole("button")
        .map((button) => button.textContent),
    ).toEqual(["Close"]);
    await user.click(within(footer).getByRole("button", { name: "Close" }));
    expect(props.onCancel).toHaveBeenCalled();
  });
});
