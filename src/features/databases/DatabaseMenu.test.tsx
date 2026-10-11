import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import * as sessionService from "../session/sessionService";
import { SessionProvider } from "../session/SessionProvider";
import { useDirtyForm } from "../session/usePendingDraft";
import * as databasesService from "./databasesService";
import { DatabaseMenu } from "./DatabaseMenu";
import type { ChooserState, DatabaseStatus } from "./types";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("./databasesService");
vi.mock("../session/sessionService");

const PATH = "/home/sam/Documents/HoploDex/Main collection.hoplodex";

const status = {
  sessionId: 1,
  path: PATH,
  name: "Main collection",
  passphraseSaved: false,
  keyringAvailable: false,
  screenLockSupported: false,
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
} satisfies DatabaseStatus;

const chooser: ChooserState = {
  recent: [
    {
      path: PATH,
      name: "Main collection",
      lastOpenedAt: "2026-09-25T10:00:00Z",
      available: true,
      passphraseSaved: false,
      backups: null,
      changedSinceLeftAt: null,
    },
  ],
  selectedPath: PATH,
  keyringAvailable: false,
  screenLockSupported: false,
  suggested: { folder: "/home/sam/Documents/HoploDex", name: "My collection" },
  notices: [],
};

/** A form with unsaved input, registered as the real forms are. */
function DirtyForm() {
  useDirtyForm({
    label: "Glock 19 (edit)",
    isDirty: true,
    submit: async () => true,
    draft: {
      formVersion: 1,
      kind: "firearm",
      mode: "edit",
      targetId: 3,
      values: { make: "Glock" },
    },
  });
  return null;
}

function renderMenu(withDirtyForm = false) {
  render(
    <SessionProvider>
      <DatabaseMenu />
      {withDirtyForm && <DirtyForm />}
    </SessionProvider>,
  );
}

describe("DatabaseMenu (contracts/ui-databases.md §4)", () => {
  beforeEach(() => {
    vi.mocked(sessionService.getDatabaseStatus).mockReset().mockResolvedValue(status);
    vi.mocked(sessionService.closeDatabase)
      .mockReset()
      .mockResolvedValue({ backup: "notAttempted" });
    vi.mocked(databasesService.getChooserState).mockReset().mockResolvedValue(chooser);
    vi.mocked(databasesService.listBackups).mockReset().mockResolvedValue({
      folder: "/home/sam/Documents/HoploDex/HoploDex backups",
      available: true,
      backups: [],
    });
    vi.mocked(databasesService.onRestoreProgress)
      .mockReset()
      .mockReturnValue(() => {});
    vi.mocked(databasesService.onBackupsDeleteProgress)
      .mockReset()
      .mockReturnValue(() => {});
  });

  it("is always labelled Database, whichever database is open", async () => {
    renderMenu();

    const button = await screen.findByRole("button", { name: "Database" });
    expect(button).toHaveAttribute("aria-haspopup", "menu");
    expect(screen.queryByText("Main collection")).not.toBeInTheDocument();
  });

  it("opens the database settings (§7)", async () => {
    const user = userEvent.setup();
    renderMenu();

    await user.click(await screen.findByRole("button", { name: "Database" }));
    await user.click(await screen.findByRole("menuitem", { name: "Database settings…" }));

    expect(await screen.findByRole("dialog", { name: "Database settings" })).toBeInTheDocument();
  });

  it("lists its items in the contract's order", async () => {
    const user = userEvent.setup();
    renderMenu();

    await user.click(await screen.findByRole("button", { name: "Database" }));

    const items = await screen.findAllByRole("menuitem");
    expect(items.map((item) => item.textContent)).toEqual([
      "Lock nowCtrl+L",
      "Database settings…",
      "Change passphrase…",
      "Restore from a backup…",
      "About databases and security",
    ]);
  });

  it("opens the guide (§11)", async () => {
    const user = userEvent.setup();
    renderMenu();

    await user.click(await screen.findByRole("button", { name: "Database" }));
    await user.click(await screen.findByRole("menuitem", { name: "About databases and security" }));

    expect(
      await screen.findByRole("dialog", { name: "About databases and security" }),
    ).toBeInTheDocument();
  });

  it("opens the passphrase change (§8)", async () => {
    const user = userEvent.setup();
    renderMenu();

    await user.click(await screen.findByRole("button", { name: "Database" }));
    await user.click(await screen.findByRole("menuitem", { name: "Change passphrase…" }));

    const dialog = await screen.findByRole("dialog", {
      name: "Change passphrase",
    });
    expect(within(dialog).getByLabelText("Current passphrase")).toBeInTheDocument();
  });

  it("opens the restore dialog (§9)", async () => {
    const user = userEvent.setup();
    renderMenu();

    await user.click(await screen.findByRole("button", { name: "Database" }));
    await user.click(await screen.findByRole("menuitem", { name: "Restore from a backup…" }));

    expect(
      await screen.findByRole("dialog", { name: "Restore from a backup" }),
    ).toBeInTheDocument();
    expect(databasesService.listBackups).toHaveBeenCalledWith(undefined);
  });
});

describe("Lock now (FR-033, FR-035)", () => {
  const draft = {
    formVersion: 1,
    kind: "firearm" as const,
    mode: "edit" as const,
    targetId: 3,
    label: "Glock 19 (edit)",
    values: { make: "Glock" },
  };

  beforeEach(() => {
    vi.mocked(sessionService.getDatabaseStatus).mockReset().mockResolvedValue(status);
    vi.mocked(sessionService.lockDatabase).mockReset().mockResolvedValue({ backup: "notDue" });
    vi.mocked(sessionService.closeDatabase).mockReset();
    vi.mocked(databasesService.getChooserState).mockReset().mockResolvedValue(chooser);
    vi.mocked(databasesService.listBackups).mockReset().mockResolvedValue({
      folder: "/home/sam/Documents/HoploDex/HoploDex backups",
      available: true,
      backups: [],
    });
  });

  it("locks from the menu with the form's unsaved input, asking nothing", async () => {
    const user = userEvent.setup();
    renderMenu(true);

    await user.click(await screen.findByRole("button", { name: "Database" }));
    const item = await screen.findByRole("menuitem", { name: /Lock now/ });
    expect(item).toHaveTextContent("Ctrl+L");
    await user.click(item);

    await waitFor(() => expect(sessionService.lockDatabase).toHaveBeenCalledWith(draft));
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(sessionService.closeDatabase).not.toHaveBeenCalled();
  });

  it("locks with one click on the lock button", async () => {
    const user = userEvent.setup();
    renderMenu();

    await user.click(await screen.findByRole("button", { name: "Lock now" }));

    await waitFor(() => expect(sessionService.lockDatabase).toHaveBeenCalledWith(null));
  });

  it("locks with Ctrl+L from anywhere, inside a dialog too", async () => {
    const user = userEvent.setup();
    renderMenu(true);
    await user.click(await screen.findByRole("button", { name: "Database" }));
    await user.click(await screen.findByRole("menuitem", { name: "Database settings…" }));
    const dialog = await screen.findByRole("dialog", { name: "Database settings" });
    await user.click(within(dialog).getByRole("textbox", { name: "Keep the latest" }));

    await user.keyboard("{Control>}l{/Control}");

    await waitFor(() => expect(sessionService.lockDatabase).toHaveBeenCalledWith(draft));
    expect(sessionService.lockDatabase).toHaveBeenCalledTimes(1);
  });

  it("locks from a dialog's lock button, which covers the top bar's", async () => {
    const user = userEvent.setup();
    renderMenu(true);
    await user.click(await screen.findByRole("button", { name: "Database" }));
    await user.click(await screen.findByRole("menuitem", { name: "Database settings…" }));
    const dialog = await screen.findByRole("dialog", { name: "Database settings" });

    const lock = within(dialog).getByRole("button", { name: "Lock now" });
    expect(lock).toHaveAttribute("title", "Lock now (Ctrl+L)");
    await user.click(lock);

    await waitFor(() => expect(sessionService.lockDatabase).toHaveBeenCalledWith(draft));
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });

  it("locks with ⌘L too", async () => {
    const user = userEvent.setup();
    renderMenu();
    await screen.findByRole("button", { name: "Lock now" });

    await user.keyboard("{Meta>}l{/Meta}");

    await waitFor(() => expect(sessionService.lockDatabase).toHaveBeenCalledTimes(1));
  });
});
