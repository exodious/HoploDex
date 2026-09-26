import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CommandFailure } from "../../services/tauriClient";
import { DatabaseNotes } from "../databases/DatabaseNotes";
import * as databasesService from "../databases/databasesService";
import type { ChooserState, DatabaseStatus } from "../databases/types";
import { SessionProvider } from "./SessionProvider";
import * as sessionService from "./sessionService";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("../databases/databasesService");
vi.mock("./sessionService");

const PATH = "/home/sam/Documents/HoploDex/Main collection.hoplodex";
const PASSPHRASE = "correct horse battery staple";

const chooser: ChooserState = {
  recent: [
    {
      path: PATH,
      name: "Main collection",
      lastOpenedAt: "2026-09-25T10:00:00Z",
      available: true,
      passphraseSaved: false,
    },
  ],
  selectedPath: PATH,
  keyringAvailable: false,
  screenLockSupported: false,
  suggested: { folder: "/home/sam/Documents/HoploDex", name: "My collection" },
  notices: [],
};

function status(diskEncryption: boolean): DatabaseStatus {
  return {
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
      diskEncryption,
      openedBackup: null,
      restoredWithPassphraseOf: null,
      damagedFileKeptAt: null,
    },
  };
}

const closed = new CommandFailure({ code: "DATABASE_CLOSED", message: "No database is open." });

function renderSession() {
  render(
    <SessionProvider>
      <DatabaseNotes />
      <p>Collection shell</p>
    </SessionProvider>,
  );
}

describe("SessionProvider (User Story 1)", () => {
  beforeEach(() => {
    vi.mocked(sessionService.getDatabaseStatus).mockReset().mockRejectedValue(closed);
    vi.mocked(sessionService.dismissNote).mockReset().mockResolvedValue(undefined);
    vi.mocked(databasesService.getChooserState).mockReset().mockResolvedValue(chooser);
    vi.mocked(databasesService.openDatabase).mockReset();
    vi.mocked(databasesService.createDatabase).mockReset();
  });

  it("shows only the chooser while no database is open", async () => {
    renderSession();

    expect(await screen.findByLabelText("Passphrase for Main collection")).toBeInTheDocument();
    expect(screen.queryByText("Collection shell")).not.toBeInTheDocument();
  });

  it("shows the collection once a database opens", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.openDatabase).mockResolvedValue(status(false));
    renderSession();

    await user.type(
      await screen.findByLabelText("Passphrase for Main collection"),
      `${PASSPHRASE}{Enter}`,
    );

    expect(await screen.findByText("Collection shell")).toBeInTheDocument();
    expect(databasesService.openDatabase).toHaveBeenCalledWith(PATH, PASSPHRASE);
    expect(screen.queryByLabelText("Passphrase for Main collection")).not.toBeInTheDocument();
  });

  it("shows the collection once a database is created", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.createDatabase).mockResolvedValue(status(true));
    renderSession();

    await user.click(await screen.findByRole("button", { name: "Create a new database…" }));
    await user.type(screen.getByLabelText("Passphrase"), PASSPHRASE);
    await user.type(screen.getByLabelText("Confirm passphrase"), PASSPHRASE);
    await user.click(screen.getByRole("checkbox", { name: /I have stored this passphrase/ }));
    await user.click(screen.getByRole("button", { name: "Create database" }));

    expect(await screen.findByText("Collection shell")).toBeInTheDocument();
    expect(databasesService.createDatabase).toHaveBeenCalledWith({
      folder: "/home/sam/Documents/HoploDex",
      name: "My collection",
      passphrase: PASSPHRASE,
      acknowledgedUnrecoverable: true,
    });
  });

  it("picks up a database that is already open, as after a reload of the window", async () => {
    vi.mocked(sessionService.getDatabaseStatus).mockResolvedValue(status(false));
    renderSession();

    expect(await screen.findByText("Collection shell")).toBeInTheDocument();
    expect(databasesService.getChooserState).not.toHaveBeenCalled();
  });

  it("shows the disk-encryption note until it is dismissed (FR-008)", async () => {
    const user = userEvent.setup();
    vi.mocked(sessionService.getDatabaseStatus).mockResolvedValue(status(true));
    renderSession();

    const note = await screen.findByRole("region", { name: "Disk encryption" });
    expect(note).toHaveTextContent(
      "Your collection is encrypted with your passphrase. For extra protection, also turn on your computer's disk encryption: BitLocker on Windows, FileVault on macOS, or LUKS on Linux.",
    );
    expect(screen.getByRole("button", { name: "Why?" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Dismiss" }));

    expect(sessionService.dismissNote).toHaveBeenCalledWith("diskEncryption");
    await waitFor(() =>
      expect(screen.queryByRole("region", { name: "Disk encryption" })).not.toBeInTheDocument(),
    );
    expect(screen.getByText("Collection shell")).toBeInTheDocument();
  });

  it("shows no note for a database whose note was dismissed before", async () => {
    vi.mocked(sessionService.getDatabaseStatus).mockResolvedValue(status(false));
    renderSession();

    await screen.findByText("Collection shell");
    expect(screen.queryByRole("region", { name: "Disk encryption" })).not.toBeInTheDocument();
  });
});
