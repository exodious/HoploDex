import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
import { ToastProvider } from "../../components";
import { formatBytes } from "../../lib/bytes";
import { CommandFailure } from "../../services/tauriClient";
import { DatabaseSettingsDialog } from "./DatabaseSettingsDialog";
import * as databasesService from "./databasesService";
import type {
  BackupInfo,
  BackupProgress,
  BackupSettingsSaved,
  CollectionSettings,
  DatabaseStatus,
  ExistingBackupsOutcome,
} from "./types";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("./databasesService");

const FOLDER = "/home/sam/Documents/HoploDex";
const DEFAULT_BACKUPS = `${FOLDER}/HoploDex backups`;

function settings(location: CollectionSettings["backups"]["location"]): CollectionSettings {
  return {
    backups: { enabled: true, keepCount: 5, location },
    lock: { idleEnabled: true, idleMinutes: 10, onScreenLock: false },
  };
}

function status(
  location: CollectionSettings["backups"]["location"] = {
    kind: "default",
    path: DEFAULT_BACKUPS,
    available: true,
  },
): DatabaseStatus {
  return {
    path: `${FOLDER}/Main collection.hoplodex`,
    name: "Main collection",
    passphraseSaved: false,
    keyringAvailable: false,
    screenLockSupported: false,
    settings: settings(location),
    pendingChanges: null,
    notes: {
      diskEncryption: false,
      openedBackup: null,
      restoredWithPassphraseOf: null,
      damagedFileKeptAt: null,
    },
  };
}

function backup(day: number): BackupInfo {
  return {
    path: `${DEFAULT_BACKUPS}/Main collection 2026-09-${day} 143005 3fa2c9d1.hoplodex`,
    fileName: `Main collection 2026-09-${day} 143005 3fa2c9d1.hoplodex`,
    madeAt: `2026-09-${day}T14:30:05`,
    sizeBytes: 212_000_000,
  };
}

function renderSettings(current: DatabaseStatus = status()) {
  const props = {
    onOpenChange: vi.fn(),
    onSaved: vi.fn(),
    onRestore: vi.fn(),
    onPassphraseSavedChange: vi.fn(),
  };
  render(
    <ToastProvider>
      <DatabaseSettingsDialog open status={current} {...props} />
    </ToastProvider>,
  );
  return props;
}

describe("DatabaseSettingsDialog: Backups (contracts/ui-databases.md §7)", () => {
  beforeEach(() => {
    vi.mocked(databasesService.listBackups)
      .mockReset()
      .mockResolvedValue({
        folder: DEFAULT_BACKUPS,
        available: true,
        backups: [backup(26), backup(25), backup(24)],
      });
    vi.mocked(databasesService.updateBackupSettings).mockReset();
    vi.mocked(databasesService.deleteAllBackups).mockReset();
    vi.mocked(databasesService.onBackupsDeleteProgress)
      .mockReset()
      .mockReturnValue(() => {});
    vi.mocked(openFolderDialog).mockReset();
  });

  it("shows the backup settings, with the default location next to the database", () => {
    renderSettings();

    expect(screen.getByRole("dialog", { name: "Database settings" })).toBeInTheDocument();
    const backups = screen.getByRole("group", { name: "Backups" });
    expect(within(backups).getByRole("checkbox", { name: "Make automatic backups" })).toBeChecked();
    const keep = within(backups).getByLabelText("Keep the latest");
    expect(keep).toHaveValue("5");
    expect(keep.closest(".hd-field")).toHaveClass("hd-field--third");
    expect(within(backups).getByText(DEFAULT_BACKUPS)).toBeInTheDocument();
    expect(within(backups).getByText("(next to the database)")).toBeInTheDocument();
    expect(within(backups).getByRole("button", { name: "Change…" })).toBeInTheDocument();
    expect(within(backups).getByRole("button", { name: "Use the default" })).toBeDisabled();
  });

  it("says when a custom location isn't available on this computer", () => {
    renderSettings(status({ kind: "custom", path: "/media/usb/Backups", available: false }));

    expect(screen.getByText("/media/usb/Backups")).toBeInTheDocument();
    expect(screen.getByText("Not available on this computer")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Use the default" })).toBeEnabled();
  });

  it("states what backups are and aren't (FR-029)", () => {
    renderSettings();

    for (const statement of [
      "Each backup is a complete copy of the collection.",
      "A backup opens only with the passphrase you had when it was made.",
      "Firearms you delete stay in earlier backups until those backups are removed.",
      "Backups on the same disk as the database don't protect against losing that disk.",
    ]) {
      expect(screen.getByText(statement)).toBeInTheDocument();
    }
  });

  it("says old backups are deleted as securely as the computer allows, with the guide a click away (FR-030)", async () => {
    const user = userEvent.setup();
    renderSettings();

    const statement = screen.getByText(/Old backups are deleted securely/);
    expect(statement).toHaveTextContent(
      "Old backups are deleted securely, as far as this computer allows (see About databases and security).",
    );
    await user.click(
      within(statement).getByRole("button", { name: "About databases and security" }),
    );

    expect(
      await screen.findByRole("dialog", { name: "About databases and security" }),
    ).toBeInTheDocument();
  });

  it("offers restoring from a backup", async () => {
    const user = userEvent.setup();
    const props = renderSettings();

    await user.click(screen.getByRole("button", { name: "Restore from a backup…" }));

    expect(props.onRestore).toHaveBeenCalledTimes(1);
  });

  it("asks before deleting all backups, then deletes them", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.deleteAllBackups).mockResolvedValue({
      deletedCount: 3,
      failedPaths: [],
    });
    renderSettings();

    await user.click(screen.getByRole("button", { name: "Delete all backups…" }));

    const confirm = await screen.findByRole("alertdialog", {
      name: "Delete all 3 backups of “Main collection”?",
    });
    expect(confirm).toHaveTextContent(
      "They are deleted securely where this computer supports it. This can't be undone.",
    );
    await user.click(within(confirm).getByRole("button", { name: "Delete all backups" }));
    expect(databasesService.deleteAllBackups).toHaveBeenCalledWith(true);
    expect(await screen.findByText("3 backups were deleted.")).toBeInTheDocument();
  });

  it("chooses a folder, then saves the settings", async () => {
    const user = userEvent.setup();
    vi.mocked(openFolderDialog).mockResolvedValue("/media/usb/Backups");
    vi.mocked(databasesService.updateBackupSettings).mockResolvedValue({
      settings: settings({ kind: "custom", path: "/media/usb/Backups", available: true }),
      existingBackups: null,
    });
    const props = renderSettings();

    await user.click(screen.getByRole("checkbox", { name: "Make automatic backups" }));
    const keep = screen.getByLabelText("Keep the latest");
    await user.clear(keep);
    await user.type(keep, "12");
    await user.click(screen.getByRole("button", { name: "Change…" }));
    expect(await screen.findByText("/media/usb/Backups")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(databasesService.updateBackupSettings).toHaveBeenCalledWith({
      enabled: false,
      keepCount: 12,
      location: { kind: "custom", path: "/media/usb/Backups" },
    });
    await waitFor(() => expect(props.onSaved).toHaveBeenCalled());
    expect(props.onOpenChange).toHaveBeenCalledWith(false);
  });

  it("puts the backend's field errors on the fields", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.updateBackupSettings).mockRejectedValue(
      new CommandFailure({
        code: "VALIDATION_ERROR",
        message: "Check the backup settings.",
        fieldErrors: { keepCount: "Keep between 1 and 100 backups." },
      }),
    );
    const props = renderSettings();

    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(await screen.findByText("Keep between 1 and 100 backups.")).toBeInTheDocument();
    expect(screen.getByLabelText("Keep the latest")).toHaveAccessibleDescription(
      /Keep between 1 and 100 backups\./,
    );
    expect(props.onOpenChange).not.toHaveBeenCalled();
  });
});

describe("DatabaseSettingsDialog: This computer (contracts/ui-databases.md §7, §8)", () => {
  const FACTS =
    "Anyone who can use this computer account, or its keyring while it's unlocked, will be able to open the database without knowing the passphrase.";

  beforeEach(() => {
    vi.mocked(databasesService.listBackups)
      .mockReset()
      .mockResolvedValue({ folder: DEFAULT_BACKUPS, available: true, backups: [] });
    vi.mocked(databasesService.savePassphrase).mockReset();
    vi.mocked(databasesService.forgetSavedPassphrase).mockReset();
  });

  function thisComputer() {
    return screen.getByRole("group", { name: "This computer" });
  }

  it("says the passphrase isn't remembered, and offers to remember it", () => {
    renderSettings({ ...status(), keyringAvailable: true });

    expect(
      within(thisComputer()).getByText(
        "Not remembered: the database asks for its passphrase each time it opens.",
      ),
    ).toBeInTheDocument();
    expect(
      within(thisComputer()).getByRole("button", {
        name: "Remember the passphrase on this computer…",
      }),
    ).toBeInTheDocument();
  });

  it("remembers it after the FR-017 confirmation, with the passphrase typed there", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.savePassphrase).mockResolvedValue({ passphraseSaved: true });
    const props = renderSettings({ ...status(), keyringAvailable: true });

    await user.click(
      within(thisComputer()).getByRole("button", {
        name: "Remember the passphrase on this computer…",
      }),
    );
    const confirm = await screen.findByRole("alertdialog", {
      name: "Remember this database's passphrase?",
    });
    expect(confirm).toHaveTextContent(FACTS);
    expect(confirm).toHaveTextContent(
      "Locking will no longer need the passphrase on this computer, though it still clears the collection from memory, deletes opened document copies, and releases the database for other computers.",
    );
    const field = within(confirm).getByLabelText("Passphrase");
    await user.type(field, "correct horse battery staple");
    // A choice, not a destructive action.
    const remember = within(confirm).getByRole("button", { name: "Remember passphrase" });
    expect(remember).not.toHaveClass("hd-button--danger");
    await user.click(remember);

    expect(databasesService.savePassphrase).toHaveBeenCalledWith("correct horse battery staple");
    await waitFor(() => expect(props.onPassphraseSavedChange).toHaveBeenCalled());
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });

  it("keeps the confirmation open with the error when the passphrase is wrong", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.savePassphrase).mockRejectedValue(
      new CommandFailure({
        code: "PASSPHRASE_INCORRECT",
        message: "The passphrase is incorrect.",
        fieldErrors: { passphrase: "That isn't this database's passphrase." },
      }),
    );
    const props = renderSettings({ ...status(), keyringAvailable: true });

    await user.click(
      within(thisComputer()).getByRole("button", {
        name: "Remember the passphrase on this computer…",
      }),
    );
    const confirm = await screen.findByRole("alertdialog");
    const field = within(confirm).getByLabelText("Passphrase");
    await user.type(field, "a guess");
    await user.click(within(confirm).getByRole("button", { name: "Remember passphrase" }));

    expect(
      await within(confirm).findByText("That isn't this database's passphrase."),
    ).toBeInTheDocument();
    expect(field).toHaveValue("");
    expect(props.onPassphraseSavedChange).not.toHaveBeenCalled();
  });

  it("saves nothing when the confirmation is cancelled", async () => {
    const user = userEvent.setup();
    renderSettings({ ...status(), keyringAvailable: true });

    await user.click(
      within(thisComputer()).getByRole("button", {
        name: "Remember the passphrase on this computer…",
      }),
    );
    const confirm = await screen.findByRole("alertdialog");
    await user.click(within(confirm).getByRole("button", { name: "Cancel" }));

    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(databasesService.savePassphrase).not.toHaveBeenCalled();
  });

  it("shows a saved passphrase and forgets it", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.forgetSavedPassphrase).mockResolvedValue({
      passphraseSaved: false,
    });
    const props = renderSettings({ ...status(), passphraseSaved: true, keyringAvailable: true });

    expect(
      within(thisComputer()).getByText(
        "The passphrase is remembered on this computer: the database opens without asking for it.",
      ),
    ).toBeInTheDocument();
    await user.click(
      within(thisComputer()).getByRole("button", { name: "Forget saved passphrase" }),
    );

    expect(databasesService.forgetSavedPassphrase).toHaveBeenCalledWith();
    await waitFor(() => expect(props.onPassphraseSavedChange).toHaveBeenCalled());
  });

  it("says when this computer has no keyring (FR-019)", () => {
    renderSettings({ ...status(), keyringAvailable: false });

    expect(
      within(thisComputer()).getByText("Not available: this computer has no keyring service."),
    ).toBeInTheDocument();
    expect(within(thisComputer()).queryByRole("button")).not.toBeInTheDocument();
  });
});

describe("DatabaseSettingsDialog: Locking (contracts/ui-databases.md §7, FR-034, FR-036, FR-038)", () => {
  beforeEach(() => {
    vi.mocked(databasesService.listBackups)
      .mockReset()
      .mockResolvedValue({ folder: DEFAULT_BACKUPS, available: true, backups: [] });
    vi.mocked(databasesService.updateBackupSettings)
      .mockReset()
      .mockResolvedValue({
        settings: settings({
          kind: "default",
          path: DEFAULT_BACKUPS,
          available: true,
        }),
        existingBackups: null,
      });
    vi.mocked(databasesService.updateLockSettings).mockReset();
  });

  const locking = () => screen.getByRole("group", { name: "Locking" });

  /** Renders the dialog once its backups have been counted. */
  async function renderSettled(current: DatabaseStatus = status()) {
    const props = renderSettings(current);
    await screen.findByText("There are no backups yet.");
    return props;
  }

  it("comes after Backups and before This computer", async () => {
    await renderSettled();

    const sections = screen.getAllByRole("group");
    expect(sections.map((section) => section.querySelector("legend")?.textContent)).toEqual([
      "Backups",
      "Locking",
      "This computer",
    ]);
  });

  it("offers the idle lock with its duration and says it also covers sleep", async () => {
    const user = userEvent.setup();
    renderSettings();

    const idle = within(locking()).getByRole("checkbox", {
      name: "Lock after a period without use",
    });
    expect(idle).toBeChecked();
    expect(
      within(locking()).getByText(
        "Also locks when the computer goes to sleep. Turning this off stops both.",
      ),
    ).toBeInTheDocument();
    const minutes = within(locking()).getByRole("combobox", { name: "After" });
    expect(minutes).toHaveTextContent("10 minutes");
    expect(minutes.closest(".hd-field")).toHaveClass("hd-field--third");

    await user.click(minutes);
    expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual([
      "1 minute",
      "2 minutes",
      "5 minutes",
      "10 minutes",
      "15 minutes",
      "30 minutes",
      "1 hour",
      "2 hours",
      "4 hours",
    ]);
  });

  it("enables the duration only while the idle lock is on", async () => {
    const user = userEvent.setup();
    renderSettings();

    await user.click(
      within(locking()).getByRole("checkbox", { name: "Lock after a period without use" }),
    );

    expect(within(locking()).getByRole("combobox", { name: "After" })).toBeDisabled();
  });

  it("saves the lock settings with Save", async () => {
    const user = userEvent.setup();
    const props = renderSettings({ ...status(), screenLockSupported: true });

    await user.click(within(locking()).getByRole("combobox", { name: "After" }));
    await user.click(screen.getByRole("option", { name: "30 minutes" }));
    await user.click(
      within(locking()).getByRole("checkbox", {
        name: "Lock when the computer's screen locks",
      }),
    );
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() =>
      expect(databasesService.updateLockSettings).toHaveBeenCalledWith({
        idleEnabled: true,
        idleMinutes: 30,
        onScreenLock: true,
      }),
    );
    await waitFor(() => expect(props.onOpenChange).toHaveBeenCalledWith(false));
  });

  it("shows the screen-lock option as unavailable where the screen lock isn't reported", async () => {
    await renderSettled();

    const onScreenLock = within(locking()).getByRole("checkbox", {
      name: "Lock when the computer's screen locks",
    });
    expect(onScreenLock).toBeDisabled();
    expect(onScreenLock).not.toBeChecked();
    expect(
      within(locking()).getByText(
        "Not available: this computer doesn't tell applications when the screen locks.",
      ),
    ).toBeInTheDocument();
  });

  it("states what locking does", async () => {
    await renderSettled();

    expect(locking()).toHaveTextContent(
      "Locking closes the database: its data is cleared from memory, opened document copies are deleted, a backup is made if one is due, and it's released for other computers.",
    );
    expect(locking()).not.toHaveTextContent("anyone using this computer account");
  });

  it("says a saved passphrase reopens a locked database (FR-036)", async () => {
    await renderSettled({ ...status(), passphraseSaved: true });

    expect(locking()).toHaveTextContent(
      "Because the passphrase is saved on this computer, anyone using this computer account can reopen the database after it locks.",
    );
  });
});

describe("DatabaseSettingsDialog: changing the location (contracts/ui-databases.md §7, FR-026, US3-4a, US3-4b)", () => {
  const USB = "/media/usb/Backups";
  const saved = (existingBackups: ExistingBackupsOutcome | null): BackupSettingsSaved => ({
    settings: settings({ kind: "custom", path: USB, available: true }),
    existingBackups,
  });
  const atOldLocation = () =>
    new CommandFailure({
      code: "BACKUPS_AT_OLD_LOCATION",
      message: "There are backups at the old location.",
      details: { folder: DEFAULT_BACKUPS, count: 3, totalBytes: 636_000_000 },
    });

  beforeEach(() => {
    vi.mocked(databasesService.listBackups)
      .mockReset()
      .mockResolvedValue({
        folder: DEFAULT_BACKUPS,
        available: true,
        backups: [backup(26), backup(25), backup(24)],
      });
    vi.mocked(databasesService.updateBackupSettings).mockReset();
    vi.mocked(databasesService.updateLockSettings)
      .mockReset()
      .mockResolvedValue(settings({ kind: "custom", path: USB, available: true }));
    vi.mocked(databasesService.onBackupsMoveProgress)
      .mockReset()
      .mockReturnValue(() => {});
    vi.mocked(databasesService.onBackupsDeleteProgress)
      .mockReset()
      .mockReturnValue(() => {});
    vi.mocked(openFolderDialog).mockReset().mockResolvedValue(USB);
  });

  /** Chooses the USB folder and a new number kept, then saves. */
  async function changeAndSave(user: ReturnType<typeof userEvent.setup>) {
    const keep = screen.getByLabelText("Keep the latest");
    await user.clear(keep);
    await user.type(keep, "12");
    await user.click(screen.getByRole("button", { name: "Change…" }));
    await screen.findByText(USB);
    await user.click(screen.getByRole("button", { name: "Save" }));
  }

  it("sends the backup settings first and holds back the lock settings until the question is answered", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.updateBackupSettings)
      .mockRejectedValueOnce(atOldLocation())
      .mockResolvedValueOnce(saved({ action: "move", movedCount: 3, leftBehind: null }));
    const props = renderSettings();

    await changeAndSave(user);
    const question = await screen.findByRole("dialog", { name: "Backups at the old location" });
    expect(question).toHaveTextContent(
      `3 backups of the database (${formatBytes(636_000_000)}) are in ${DEFAULT_BACKUPS}.`,
    );
    expect(databasesService.updateLockSettings).not.toHaveBeenCalled();

    await user.click(within(question).getByRole("button", { name: "Change location" }));

    await waitFor(() => expect(props.onOpenChange).toHaveBeenCalledWith(false));
    expect(databasesService.updateBackupSettings).toHaveBeenLastCalledWith({
      enabled: true,
      keepCount: 12,
      location: { kind: "custom", path: USB },
      existingBackups: "move",
    });
    expect(databasesService.updateLockSettings).toHaveBeenCalled();
    expect(props.onSaved).toHaveBeenCalled();
    expect(await screen.findByText(`The backups were moved to ${USB}.`)).toBeInTheDocument();
  });

  it("says when the backups at the old location were deleted", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.updateBackupSettings)
      .mockRejectedValueOnce(atOldLocation())
      .mockResolvedValueOnce(saved({ action: "delete", deletedCount: 3 }));
    renderSettings();

    await changeAndSave(user);
    await user.click(await screen.findByRole("radio", { name: /Delete them/ }));
    await user.click(screen.getByRole("button", { name: "Change location" }));
    await user.click(screen.getByRole("button", { name: "Delete all backups" }));

    expect(
      await screen.findByText(`The backups at ${DEFAULT_BACKUPS} were deleted.`),
    ).toBeInTheDocument();
  });

  it("cancelling the question saves nothing and puts the location back, keeping the other fields' input", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.updateBackupSettings).mockRejectedValueOnce(atOldLocation());
    const props = renderSettings();

    await changeAndSave(user);
    const question = await screen.findByRole("dialog", { name: "Backups at the old location" });
    await user.click(within(question).getByRole("button", { name: "Cancel" }));

    await waitFor(() =>
      expect(
        screen.queryByRole("dialog", { name: "Backups at the old location" }),
      ).not.toBeInTheDocument(),
    );
    const backups = screen.getByRole("group", { name: "Backups" });
    expect(within(backups).getByText(DEFAULT_BACKUPS)).toBeInTheDocument();
    expect(within(backups).getByText("(next to the database)")).toBeInTheDocument();
    expect(screen.getByLabelText("Keep the latest")).toHaveValue("12");
    expect(databasesService.updateBackupSettings).toHaveBeenCalledTimes(1);
    expect(databasesService.updateLockSettings).not.toHaveBeenCalled();
    expect(props.onSaved).not.toHaveBeenCalled();
    expect(props.onOpenChange).not.toHaveBeenCalled();
  });

  it("warns when the old location can't be reached, and Continue leaves its backups", async () => {
    const user = userEvent.setup();
    const old = "/media/old-usb/Backups";
    vi.mocked(databasesService.updateBackupSettings)
      .mockRejectedValueOnce(
        new CommandFailure({
          code: "OLD_BACKUP_LOCATION_UNAVAILABLE",
          message: "The old backup location can't be reached.",
          details: { folder: old },
        }),
      )
      .mockResolvedValueOnce(saved({ action: "leave" }));
    const props = renderSettings(status({ kind: "custom", path: old, available: false }));

    await changeAndSave(user);
    const warning = await screen.findByRole("alertdialog", {
      name: "The old backup location isn't available",
    });
    expect(warning).toHaveTextContent(
      `${old} can't be reached from this computer, so any backups there can't be moved or deleted from here. If you continue, they stay there, and HoploDex no longer manages them.`,
    );
    expect(within(warning).getByRole("button", { name: "Continue" })).not.toHaveClass(
      "hd-button--danger",
    );
    await user.click(within(warning).getByRole("button", { name: "Continue" }));

    await waitFor(() => expect(props.onOpenChange).toHaveBeenCalledWith(false));
    expect(databasesService.updateBackupSettings).toHaveBeenLastCalledWith(
      expect.objectContaining({ existingBackups: "leave" }),
    );
  });

  it("cancelling the unreachable-location warning puts the location back", async () => {
    const user = userEvent.setup();
    const old = "/media/old-usb/Backups";
    vi.mocked(databasesService.updateBackupSettings).mockRejectedValueOnce(
      new CommandFailure({
        code: "OLD_BACKUP_LOCATION_UNAVAILABLE",
        message: "The old backup location can't be reached.",
        details: { folder: old },
      }),
    );
    renderSettings(status({ kind: "custom", path: old, available: false }));

    await changeAndSave(user);
    const warning = await screen.findByRole("alertdialog");
    await user.click(within(warning).getByRole("button", { name: "Cancel" }));

    const backups = screen.getByRole("group", { name: "Backups" });
    await waitFor(() => expect(within(backups).getByText(old)).toBeInTheDocument());
    expect(databasesService.updateLockSettings).not.toHaveBeenCalled();
  });

  it("keeps the settings open with a warning when a move left some behind", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.updateBackupSettings)
      .mockRejectedValueOnce(atOldLocation())
      .mockResolvedValueOnce(
        saved({
          action: "move",
          movedCount: 2,
          leftBehind: { count: 1, folder: DEFAULT_BACKUPS, reason: "nameTaken" },
        }),
      );
    const props = renderSettings();

    await changeAndSave(user);
    await user.click(await screen.findByRole("button", { name: "Change location" }));

    const settingsDialog = screen.getByRole("dialog", { name: "Database settings" });
    const banner = await within(settingsDialog).findByRole("status");
    expect(banner).toHaveTextContent(
      `2 backups were moved. 1 is still in ${DEFAULT_BACKUPS} because a backup with the same name is already in ${USB}. HoploDex no longer manages it there.`,
    );
    expect(banner).toHaveClass("hd-banner");
    expect(banner).not.toHaveClass("hd-banner--error");
    expect(props.onSaved).toHaveBeenCalled();
    expect(props.onOpenChange).not.toHaveBeenCalled();
    expect(databasesService.updateLockSettings).toHaveBeenCalled();
  });

  it.each([
    ["locationUnavailable", `because ${USB} became unavailable`],
    ["insufficientSpace", "because there wasn't enough space there"],
    ["io", "because of an error"],
  ] as const)("says a move stopped by %s left some behind", async (reason, because) => {
    const user = userEvent.setup();
    vi.mocked(databasesService.updateBackupSettings)
      .mockRejectedValueOnce(atOldLocation())
      .mockResolvedValueOnce(
        saved({
          action: "move",
          movedCount: 1,
          leftBehind: { count: 2, folder: DEFAULT_BACKUPS, reason },
        }),
      );
    renderSettings();

    await changeAndSave(user);
    await user.click(await screen.findByRole("button", { name: "Change location" }));

    expect(
      await screen.findByText(
        `1 backup was moved. 2 are still in ${DEFAULT_BACKUPS} ${because}. HoploDex no longer manages them there.`,
      ),
    ).toBeInTheDocument();
  });

  it("shows the move's progress from the backend", async () => {
    const user = userEvent.setup();
    let progress: (value: BackupProgress) => void = () => {};
    vi.mocked(databasesService.onBackupsMoveProgress).mockImplementation((handler) => {
      progress = handler;
      return () => {};
    });
    vi.mocked(databasesService.updateBackupSettings)
      .mockRejectedValueOnce(atOldLocation())
      .mockReturnValueOnce(new Promise(() => {}));
    renderSettings();

    await changeAndSave(user);
    await user.click(await screen.findByRole("button", { name: "Change location" }));
    act(() => progress({ processed: 636_000_000, total: 1_272_000_000, showNow: true }));

    expect(screen.getByRole("progressbar", { name: "Moving the backups" })).toHaveAttribute(
      "aria-valuenow",
      "636000000",
    );
  });
});
