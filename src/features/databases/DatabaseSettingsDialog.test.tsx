import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
import { CommandFailure } from "../../services/tauriClient";
import { DatabaseSettingsDialog } from "./DatabaseSettingsDialog";
import * as databasesService from "./databasesService";
import type { BackupInfo, CollectionSettings, DatabaseStatus } from "./types";

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
  };
  render(<DatabaseSettingsDialog open status={current} {...props} />);
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

    expect(screen.getByRole("dialog", { name: "Main collection settings" })).toBeInTheDocument();
    const backups = screen.getByRole("group", { name: "Backups" });
    expect(within(backups).getByRole("checkbox", { name: "Make automatic backups" })).toBeChecked();
    const keep = within(backups).getByLabelText("Keep the latest");
    expect(keep).toHaveValue("5");
    expect(keep.closest(".hd-field")).toHaveClass("hd-field--quarter");
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
      name: "Delete all 3 backups of Main collection?",
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
    vi.mocked(databasesService.updateBackupSettings).mockResolvedValue(
      settings({ kind: "custom", path: "/media/usb/Backups", available: true }),
    );
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
