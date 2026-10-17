import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { formatBytes } from "../../lib/bytes";
import { formatDateTime } from "../../lib/dates";
import { CommandFailure } from "../../services/tauriClient";
import { SessionScopeWrapper, TEST_SESSION_ID } from "../../test/sessionScope";
import { SessionContext } from "../session/sessionStore";
import type { SessionState } from "../session/sessionStore";
import * as databasesService from "./databasesService";
import { RestoreBackupDialog } from "./RestoreBackupDialog";
import type { RestoreBackupDialogProps } from "./RestoreBackupDialog";
import type { BackupInfo, BackupList, RestoreProgress } from "./types";

vi.mock("./databasesService");

const SCOPE = expect.objectContaining({ id: TEST_SESSION_ID });
const FOLDER = "/home/sam/Documents/HoploDex/HoploDex backups";
const DATABASE = "/home/sam/Documents/HoploDex/Main collection.hoplodex";
const PASSPHRASE = "the passphrase it used to have";

function backup(madeAt: string, sizeBytes: number): BackupInfo {
  const stamp = madeAt.replace("T", " ").replace(/:/g, "").slice(0, 17);
  return {
    path: `${FOLDER}/Main collection ${stamp} 3fa2c9d1.hoplodex`,
    fileName: `Main collection ${stamp} 3fa2c9d1.hoplodex`,
    madeAt,
    sizeBytes,
    beforeUpgradeTo: null,
  };
}

const newer = backup("2026-09-25T14:30:05", 212_000_000);
const older = backup("2026-09-20T09:00:00", 198_000_000);

let sendProgress: (progress: RestoreProgress) => void = () => {};

function renderDialog(props: Partial<RestoreBackupDialogProps> = {}) {
  const session: SessionState = {
    status: null,
    openDatabase: vi.fn(),
    upgradeDatabase: vi.fn(),
    createDatabase: vi.fn(),
    closeDatabase: vi.fn(),
    lockDatabase: vi.fn(),
    refreshStatus: vi.fn(),
    dismissNote: vi.fn(),
    restoreBackup: vi.fn().mockResolvedValue(undefined),
    settingsRequested: false,
    requestSettings: vi.fn(),
    clearSettingsRequest: vi.fn(),
  };
  const onOpenChange = vi.fn();
  const dialog = (
    <SessionContext.Provider value={session}>
      <RestoreBackupDialog open onOpenChange={onOpenChange} name="Main collection" {...props} />
    </SessionContext.Provider>
  );
  // A database that isn't open has no session to name (from the chooser).
  render(props.databasePath ? dialog : <SessionScopeWrapper>{dialog}</SessionScopeWrapper>);
  return { session, onOpenChange };
}

/** Chooses `choice`, types the passphrase and confirms the replacement. */
async function restore(choice: BackupInfo = older) {
  const user = userEvent.setup();
  await user.click(await screen.findByRole("radio", { name: label(choice) }));
  await user.type(screen.getByLabelText("Passphrase for this backup"), PASSPHRASE);
  await user.click(screen.getByRole("button", { name: "Restore" }));
  const confirm = await screen.findByRole("alertdialog", {
    name: `Replace “Main collection” with the backup from ${formatDateTime(choice.madeAt)}?`,
  });
  await user.click(within(confirm).getByRole("button", { name: "Restore" }));
  return user;
}

function label(choice: BackupInfo) {
  return `${formatDateTime(choice.madeAt)} — ${formatBytes(choice.sizeBytes)}`;
}

describe("RestoreBackupDialog (003 contracts/ui-databases.md §9)", () => {
  beforeEach(() => {
    vi.mocked(databasesService.listBackups)
      .mockReset()
      .mockResolvedValue({
        folder: FOLDER,
        available: true,
        backups: [newer, older],
      });
    vi.mocked(databasesService.onRestoreProgress)
      .mockReset()
      .mockImplementation((handler) => {
        sendProgress = handler;
        return () => {};
      });
    // 009 contracts/ui.md §6: a backup in the current layout needs no upgrade.
    vi.mocked(databasesService.checkBackup).mockReset().mockResolvedValue({ upgradeTo: null });
  });

  it("lists the backups newest first, with date, time and size", async () => {
    renderDialog();

    const radios = await screen.findAllByRole("radio");
    expect(radios).toHaveLength(2);
    expect(radios[0]).toHaveAccessibleName(label(newer));
    expect(radios[1]).toHaveAccessibleName(label(older));
    expect(radios[0]).toBeChecked();
    expect(databasesService.listBackups).toHaveBeenCalledWith(SCOPE, undefined);
  });

  // Regression: it opened on "Looking for backups…" and grew once the list
  // came, moving the centred dialog.
  it("opens only once the backups are listed, on the newest", async () => {
    let listed: (list: BackupList) => void = () => {};
    vi.mocked(databasesService.listBackups).mockReturnValueOnce(
      new Promise((resolve) => (listed = resolve)),
    );
    renderDialog();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    await act(async () => listed({ folder: FOLDER, available: true, backups: [newer, older] }));

    const dialog = screen.getByRole("dialog", { name: "Restore from a backup" });
    expect(within(dialog).queryByText("Looking for backups…")).not.toBeInTheDocument();
    expect(within(dialog).getByRole("radio", { name: label(newer) })).toBeChecked();
  });

  it("opens anyway when listing is slow, and chooses the newest once listed", async () => {
    let listed: (list: BackupList) => void = () => {};
    vi.mocked(databasesService.listBackups).mockReturnValueOnce(
      new Promise((resolve) => (listed = resolve)),
    );
    renderDialog();

    expect(await screen.findByText("Looking for backups…")).toBeInTheDocument();
    await act(async () => listed({ folder: FOLDER, available: true, backups: [newer, older] }));
    expect(screen.getByRole("radio", { name: label(newer) })).toBeChecked();
  });

  it("names the folder when there are no backups", async () => {
    vi.mocked(databasesService.listBackups).mockResolvedValueOnce({
      folder: FOLDER,
      available: true,
      backups: [],
    });
    renderDialog();
    expect(
      await screen.findByText(`There are no backups of this database in ${FOLDER}.`),
    ).toBeInTheDocument();
  });

  it("says when the backup folder isn't available", async () => {
    vi.mocked(databasesService.listBackups).mockResolvedValueOnce({
      folder: "/media/usb/Backups",
      available: false,
      backups: [],
    });
    renderDialog();
    expect(
      await screen.findByText(
        "The backup folder /media/usb/Backups isn't available on this computer.",
      ),
    ).toBeInTheDocument();
  });

  it("says which passphrase to enter and that the current database is backed up first", async () => {
    const user = userEvent.setup();
    renderDialog();
    await user.click(await screen.findByRole("radio", { name: label(older) }));

    expect(
      screen.getByText(
        `Enter the passphrase the database had on ${formatDateTime(older.madeAt)}. After restoring, it opens with that passphrase.`,
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        "Before restoring, the database is backed up as it is now, so you can undo this by restoring that backup.",
      ),
    ).toBeInTheDocument();
    expect(screen.queryByText(/damaged file/)).not.toBeInTheDocument();
  });

  it("restores a damaged database from its path, keeping the damaged file", async () => {
    const { session } = renderDialog({ databasePath: DATABASE });

    // Not the open database, so it is named, in quotes.
    expect(
      await screen.findByRole("dialog", { name: "Restore “Main collection” from a backup" }),
    ).toBeInTheDocument();
    expect(
      await screen.findByText("The damaged file will be kept next to it, renamed."),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Passphrase for this backup")).toHaveAccessibleDescription(
      `Enter the passphrase “Main collection” had on ${formatDateTime(newer.madeAt)}. After restoring, it opens with that passphrase.`,
    );
    expect(screen.queryByText(/is backed up first/)).not.toBeInTheDocument();
    expect(databasesService.listBackups).toHaveBeenCalledWith(null, DATABASE);
    await restore();

    expect(session.restoreBackup).toHaveBeenCalledWith(older.path, PASSPHRASE, DATABASE);
  });

  it("asks before replacing, then restores with the passphrase read from the field", async () => {
    const { session, onOpenChange } = renderDialog();

    await restore();

    expect(session.restoreBackup).toHaveBeenCalledWith(older.path, PASSPHRASE, undefined);
    // The confirmation's submit doesn't reach the form it sits in (issue #44).
    expect(session.restoreBackup).toHaveBeenCalledTimes(1);
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("shows each phase's progress and can't be dismissed while it runs", async () => {
    let finish: () => void = () => {};
    const { session, onOpenChange } = renderDialog();
    vi.mocked(session.restoreBackup).mockReturnValue(
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
    );

    const user = await restore();

    for (const [phase, text] of [
      ["copying", "Copying the backup…"],
      ["checking", "Checking the backup…"],
      ["upgrading", "Upgrading the restored copy…"],
      ["savingCurrent", "Backing up the current database…"],
      ["replacing", "Replacing the database…"],
    ] as const) {
      act(() => sendProgress({ phase, processed: 10, total: phase === "checking" ? 0 : 100 }));
      expect(screen.getByText(text)).toBeInTheDocument();
    }
    expect(screen.getByRole("progressbar", { name: "Restore progress" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Close" })).not.toBeInTheDocument();
    await user.keyboard("{Escape}");
    expect(onOpenChange).not.toHaveBeenCalled();

    await act(async () => finish());
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("puts a wrong backup passphrase on the field", async () => {
    const { session } = renderDialog();
    vi.mocked(session.restoreBackup).mockRejectedValue(
      new CommandFailure({ code: "PASSPHRASE_INCORRECT", message: "The passphrase is incorrect." }),
    );

    await restore();

    expect(
      await screen.findByText(
        `That passphrase didn't open the backup from ${formatDateTime(older.madeAt)}.`,
      ),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Passphrase for this backup")).toHaveValue("");
  });

  it.each([
    [
      new CommandFailure({
        code: "INSUFFICIENT_SPACE",
        message: "There isn't enough free space.",
        details: {
          bytesNeeded: 420_000_000,
          bytesAvailable: 100_000_000,
          path: "/home/sam/Documents/HoploDex",
        },
      }),
      "Restoring needs 420 MB free on /home/sam/Documents/HoploDex; 100 MB is free. Nothing has been changed.",
      false,
    ],
    [
      new CommandFailure({
        code: "BACKUP_LOCATION_UNAVAILABLE",
        message: "The backup folder can't be used.",
        details: { path: FOLDER, reason: "missing" },
      }),
      "The database can't be backed up first, because the backup location is not available. Nothing has been changed.",
      true,
    ],
    [
      new CommandFailure({
        code: "BACKUP_LOCATION_UNAVAILABLE",
        message: "The backup folder can't be used.",
        details: { path: FOLDER, reason: "insufficientSpace" },
      }),
      "The database can't be backed up first, because there is not enough space there. Nothing has been changed.",
      true,
    ],
    [
      new CommandFailure({ code: "RESTORE_CANCELLED", message: "The restore was cancelled." }),
      "The database couldn't be backed up, so the restore was cancelled. Nothing has been changed.",
      false,
    ],
  ])("keeps the dialog open with what stopped it: %s", async (failure, text, offersLocation) => {
    const onChangeLocation = vi.fn();
    const { session, onOpenChange } = renderDialog({ onChangeLocation });
    vi.mocked(session.restoreBackup).mockRejectedValue(failure);

    const user = await restore();

    expect(await screen.findByText(text)).toBeInTheDocument();
    expect(onOpenChange).not.toHaveBeenCalled();
    const change = screen.queryByRole("button", { name: "Change backup location…" });
    if (offersLocation) {
      await user.click(change!);
      expect(onChangeLocation).toHaveBeenCalledTimes(1);
    } else {
      expect(change).not.toBeInTheDocument();
    }
  });
});

// 009 contracts/ui.md §6 and §7
describe("RestoreBackupDialog and backups made in an older layout (009 FR-019)", () => {
  const WHEN = formatDateTime(older.madeAt);
  const UPGRADE_SENTENCE =
    "This backup was made by an earlier version of HoploDex, so it is upgraded for HoploDex 0.2.0 as it is restored. The backup itself stays as it is, and earlier versions can still open it.";

  beforeEach(() => {
    vi.mocked(databasesService.listBackups)
      .mockReset()
      .mockResolvedValue({ folder: FOLDER, available: true, backups: [newer, older] });
    vi.mocked(databasesService.onRestoreProgress)
      .mockReset()
      .mockImplementation((handler) => {
        sendProgress = handler;
        return () => {};
      });
    vi.mocked(databasesService.checkBackup).mockReset().mockResolvedValue({ upgradeTo: null });
  });

  /** Chooses `older`, types the passphrase and presses Restore, which checks. */
  async function pressRestore() {
    const user = userEvent.setup();
    await user.click(await screen.findByRole("radio", { name: label(older) }));
    await user.type(screen.getByLabelText("Passphrase for this backup"), PASSPHRASE);
    await user.click(screen.getByRole("button", { name: "Restore" }));
    return user;
  }

  it("checks the backup before it asks, with the button busy meanwhile", async () => {
    let checked: (answer: { upgradeTo: string | null }) => void = () => {};
    vi.mocked(databasesService.checkBackup).mockReturnValue(
      new Promise((resolve) => (checked = resolve)),
    );
    const { session } = renderDialog();

    await pressRestore();

    expect(databasesService.checkBackup).toHaveBeenCalledExactlyOnceWith(
      SCOPE,
      older.path,
      PASSPHRASE,
      undefined,
    );
    const busy = screen.getByRole("button", { name: "Restore" });
    expect(busy).toHaveAttribute("aria-busy", "true");
    expect(busy).toBeDisabled();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(session.restoreBackup).not.toHaveBeenCalled();

    await act(async () => checked({ upgradeTo: null }));

    const confirm = await screen.findByRole("alertdialog");
    expect(confirm).not.toHaveTextContent("upgraded for HoploDex");
    expect(session.restoreBackup).not.toHaveBeenCalled();
  });

  it("checks a damaged database's restore by its path, with no session", async () => {
    renderDialog({ databasePath: DATABASE });

    await pressRestore();

    expect(await screen.findByRole("alertdialog")).toBeInTheDocument();
    expect(databasesService.checkBackup).toHaveBeenCalledExactlyOnceWith(
      null,
      older.path,
      PASSPHRASE,
      DATABASE,
    );
  });

  it("puts a wrong passphrase from the check on the field, with no confirmation", async () => {
    vi.mocked(databasesService.checkBackup).mockRejectedValue(
      new CommandFailure({ code: "PASSPHRASE_INCORRECT", message: "The passphrase is incorrect." }),
    );
    const { session } = renderDialog();

    await pressRestore();

    expect(
      await screen.findByText(`That passphrase didn't open the backup from ${WHEN}.`),
    ).toBeInTheDocument();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(session.restoreBackup).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Restore" })).toBeEnabled();
  });

  it("says above the footer that a newer version made the backup, with no confirmation", async () => {
    vi.mocked(databasesService.checkBackup).mockRejectedValue(
      new CommandFailure({
        code: "DATABASE_NEWER_VERSION",
        message: "This backup was made by a newer version.",
      }),
    );
    const { session, onOpenChange } = renderDialog();

    await pressRestore();

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(
      "This backup was made by a newer version of HoploDex, so this version can't restore it.",
    );
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(session.restoreBackup).not.toHaveBeenCalled();
    expect(onOpenChange).not.toHaveBeenCalled();
  });

  it("says above the footer that a backup is damaged, when the check finds it so", async () => {
    vi.mocked(databasesService.checkBackup).mockRejectedValue(
      new CommandFailure({ code: "DATABASE_DAMAGED", message: "damaged" }),
    );
    renderDialog();

    await pressRestore();

    expect(await screen.findByRole("alert")).toHaveTextContent(
      `The backup from ${WHEN} is damaged and can't be restored. Nothing has been changed.`,
    );
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });

  it("adds the upgrade sentence to the confirmation and sends upgrade: true", async () => {
    vi.mocked(databasesService.checkBackup).mockResolvedValue({ upgradeTo: "0.2.0" });
    const { session, onOpenChange } = renderDialog();
    const user = await pressRestore();

    const confirm = await screen.findByRole("alertdialog", {
      name: `Replace “Main collection” with the backup from ${WHEN}?`,
    });
    expect(confirm).toHaveTextContent(
      `The backup takes its place, after the database is backed up as it is now. ${UPGRADE_SENTENCE}`,
    );
    await user.click(within(confirm).getByRole("button", { name: "Restore" }));

    expect(session.restoreBackup).toHaveBeenCalledExactlyOnceWith(
      older.path,
      PASSPHRASE,
      undefined,
      true,
    );
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("sends upgrade: true for a damaged database's restore too", async () => {
    vi.mocked(databasesService.checkBackup).mockResolvedValue({ upgradeTo: "0.2.0" });
    const { session } = renderDialog({ databasePath: DATABASE });
    const user = await pressRestore();

    const confirm = await screen.findByRole("alertdialog");
    expect(confirm).toHaveTextContent(
      `The backup takes its place, and the damaged file is kept beside it. ${UPGRADE_SENTENCE}`,
    );
    await user.click(within(confirm).getByRole("button", { name: "Restore" }));

    expect(session.restoreBackup).toHaveBeenCalledExactlyOnceWith(
      older.path,
      PASSPHRASE,
      DATABASE,
      true,
    );
  });

  it("shows why the upgrade of the restored copy failed and stays open", async () => {
    vi.mocked(databasesService.checkBackup).mockResolvedValue({ upgradeTo: "0.2.0" });
    const { session, onOpenChange } = renderDialog();
    vi.mocked(session.restoreBackup).mockRejectedValue(
      new CommandFailure({
        code: "UPGRADE_FAILED",
        message: "The upgrade failed.",
        details: { backupPath: older.path, atBackupLocation: true },
      }),
    );
    const user = await pressRestore();
    const confirm = await screen.findByRole("alertdialog");
    await user.click(within(confirm).getByRole("button", { name: "Restore" }));

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(
      "The backup couldn't be upgraded for this version, so nothing was restored. The database is as it was, and a backup of it was saved before the restore began.",
    );
    expect(onOpenChange).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Restore" })).toBeEnabled();
  });

  it("lists a backup made before an upgrade with the version it preceded (009 FR-013)", async () => {
    const before = {
      ...backup("2026-09-27T09:30:00", 212_000_000),
      beforeUpgradeTo: "0.2.0",
    };
    vi.mocked(databasesService.listBackups).mockResolvedValue({
      folder: FOLDER,
      available: true,
      backups: [before, newer, older],
    });
    renderDialog();

    const radios = await screen.findAllByRole("radio");
    expect(radios).toHaveLength(3);
    expect(radios[0]).toHaveAccessibleName(`${label(before)} — before upgrading to HoploDex 0.2.0`);
    expect(radios[1]).toHaveAccessibleName(label(newer));
    expect(radios[2]).toHaveAccessibleName(label(older));
  });
});
