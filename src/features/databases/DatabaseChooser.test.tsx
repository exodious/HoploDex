import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { formatDateTime } from "../../lib/dates";
import { CommandFailure } from "../../services/tauriClient";
import { SessionContext } from "../session/sessionStore";
import type { SessionState } from "../session/sessionStore";
import { DatabaseChooser } from "./DatabaseChooser";
import * as databasesService from "./databasesService";
import type { ChooserState, RecentDatabase } from "./types";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("./databasesService");

function recent(name: string, folder: string, lastOpenedAt: string): RecentDatabase {
  return {
    path: `${folder}/${name}.hoplodex`,
    name,
    lastOpenedAt,
    available: true,
    passphraseSaved: false,
  };
}

const main = recent("Main collection", "/home/sam/Documents/HoploDex", "2026-09-25T10:00:00Z");
const shared = recent("Shared collection", "/mnt/nas/family", "2026-09-20T10:00:00Z");

function chooserState(overrides: Partial<ChooserState> = {}): ChooserState {
  return {
    recent: [main, shared],
    selectedPath: main.path,
    keyringAvailable: false,
    screenLockSupported: false,
    suggested: { folder: "/home/sam/Documents/HoploDex", name: "My collection" },
    notices: [],
    ...overrides,
  };
}

function renderChooser(session: Partial<SessionState> = {}, selectPath?: string) {
  const value: SessionState = {
    status: null,
    openDatabase: vi.fn().mockResolvedValue(undefined),
    createDatabase: vi.fn().mockResolvedValue(undefined),
    closeDatabase: vi.fn().mockResolvedValue(undefined),
    refreshStatus: vi.fn().mockResolvedValue(undefined),
    dismissNote: vi.fn().mockResolvedValue(undefined),
    restoreBackup: vi.fn().mockResolvedValue(undefined),
    settingsRequested: false,
    requestSettings: vi.fn(),
    clearSettingsRequest: vi.fn(),
    ...session,
  };
  render(
    <SessionContext.Provider value={value}>
      <DatabaseChooser selectPath={selectPath} />
    </SessionContext.Provider>,
  );
  return value;
}

describe("DatabaseChooser (contracts/ui-databases.md §1)", () => {
  beforeEach(() => {
    vi.mocked(databasesService.getChooserState).mockReset();
  });

  it("welcomes a first run with the two page actions as large buttons", async () => {
    vi.mocked(databasesService.getChooserState).mockResolvedValue(
      chooserState({ recent: [], selectedPath: null }),
    );
    renderChooser();

    expect(
      await screen.findByText(
        "HoploDex keeps your collection in an encrypted database file that only your passphrase opens.",
      ),
    ).toBeInTheDocument();
    const create = screen.getByRole("button", { name: "Create a new database…" });
    const openOther = screen.getByRole("button", { name: "Open another database file…" });
    expect(create).toHaveClass("hd-chooser__action--large");
    expect(openOther).toHaveClass("hd-chooser__action--large");
    expect(screen.queryByRole("list", { name: "Recent databases" })).not.toBeInTheDocument();
  });

  it("selects the most recent database and focuses its passphrase field", async () => {
    vi.mocked(databasesService.getChooserState).mockResolvedValue(chooserState());
    renderChooser();

    const field = await screen.findByLabelText("Passphrase for Main collection");
    await waitFor(() => expect(field).toHaveFocus());
    expect(screen.getByRole("button", { name: "Open" })).toBeInTheDocument();

    const list = screen.getByRole("list", { name: "Recent databases" });
    const rows = within(list).getAllByRole("listitem");
    expect(rows).toHaveLength(2);
    expect(rows[0]).toHaveTextContent("Main collection");
    expect(rows[1]).toHaveTextContent("Shared collection");
    // The other row names its full folder for screen readers and in its tooltip.
    const other = within(rows[1]).getByRole("button", {
      name: "Shared collection, /mnt/nas/family",
    });
    expect(within(other).getByTitle("/mnt/nas/family")).toBeInTheDocument();
  });

  it("opens the selected database with the typed passphrase, then clears the field", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.getChooserState).mockResolvedValue(chooserState());
    const session = renderChooser();

    const field = await screen.findByLabelText("Passphrase for Main collection");
    await user.type(field, "correct horse battery staple{Enter}");

    expect(session.openDatabase).toHaveBeenCalledWith(main.path, "correct horse battery staple");
    expect(field).toHaveValue("");
  });

  it("selecting another row asks for that database's passphrase", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.getChooserState).mockResolvedValue(chooserState());
    renderChooser();

    await user.click(
      await screen.findByRole("button", { name: "Shared collection, /mnt/nas/family" }),
    );

    const field = screen.getByLabelText("Passphrase for Shared collection");
    await waitFor(() => expect(field).toHaveFocus());
    expect(screen.queryByLabelText("Passphrase for Main collection")).not.toBeInTheDocument();
  });

  it("says a wrong passphrase didn't open it and keeps the field for another try (FR-006)", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.getChooserState).mockResolvedValue(chooserState());
    const session = renderChooser({
      openDatabase: vi.fn().mockRejectedValue(
        new CommandFailure({
          code: "PASSPHRASE_INCORRECT",
          message:
            "The passphrase is incorrect, or this file is not a HoploDex database or is damaged.",
        }),
      ),
    });

    const field = await screen.findByLabelText("Passphrase for Main collection");
    await user.type(field, "not the right one");
    await user.click(screen.getByRole("button", { name: "Open" }));

    expect(
      await screen.findByText(
        "That passphrase didn't open Main collection. Either the passphrase is wrong, or the file isn't a HoploDex database or is damaged.",
      ),
    ).toBeInTheDocument();
    expect(session.openDatabase).toHaveBeenCalledTimes(1);
    const again = screen.getByLabelText("Passphrase for Main collection");
    expect(again).toBeEnabled();
    expect(again).toHaveValue("");
    expect(again).toHaveAccessibleDescription(/That passphrase didn't open Main collection/);
  });

  it("shows the opening state at once, before the open returns (SC-003)", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.getChooserState).mockResolvedValue(chooserState());
    let finish: () => void = () => {};
    renderChooser({
      openDatabase: vi.fn(() => new Promise<void>((resolve) => (finish = resolve))),
    });

    const field = await screen.findByLabelText("Passphrase for Main collection");
    await user.type(field, "correct horse battery staple");
    await user.click(screen.getByRole("button", { name: "Open" }));

    const opening = screen.getByRole("button", { name: "Opening…" });
    expect(opening).toHaveAttribute("aria-busy", "true");
    expect(field).toBeDisabled();
    expect(
      screen.getByRole("button", { name: "Shared collection, /mnt/nas/family" }),
    ).toBeDisabled();
    expect(screen.getByRole("status")).toHaveTextContent("Opening Main collection…");

    finish();
    await waitFor(() => expect(screen.getByRole("status")).not.toHaveTextContent("Opening"));
  });

  it("shows no collection data", async () => {
    vi.mocked(databasesService.getChooserState).mockResolvedValue(chooserState());
    renderChooser();

    await screen.findByLabelText("Passphrase for Main collection");
    expect(screen.queryByRole("navigation", { name: "Sections" })).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Open a database" })).toBeInTheDocument();
  });

  it("opens the create dialog with the suggested location", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.getChooserState).mockResolvedValue(chooserState());
    renderChooser();

    await user.click(await screen.findByRole("button", { name: "Create a new database…" }));

    const dialog = screen.getByRole("dialog", { name: "Create a database" });
    expect(within(dialog).getByLabelText("Folder")).toHaveValue("/home/sam/Documents/HoploDex");
  });
});

describe("DatabaseChooser (User Story 2)", () => {
  const PASSPHRASE = "correct horse battery staple";

  beforeEach(() => {
    vi.mocked(databasesService.getChooserState).mockReset().mockResolvedValue(chooserState());
    vi.mocked(databasesService.removeRecentDatabase)
      .mockReset()
      .mockResolvedValue({ removed: true });
    vi.mocked(databasesService.locateDatabase).mockReset();
    vi.mocked(openFileDialog).mockReset();
  });

  function failing(code: string, details?: Record<string, unknown>) {
    return vi
      .fn()
      .mockRejectedValue(new CommandFailure({ code, message: `backend ${code}`, details }));
  }

  async function openMain(user: ReturnType<typeof userEvent.setup>) {
    await user.type(
      await screen.findByLabelText("Passphrase for Main collection"),
      `${PASSPHRASE}{Enter}`,
    );
  }

  it("lists the recent databases, most recent first, with their folders", async () => {
    renderChooser();

    const list = await screen.findByRole("list", { name: "Recent databases" });
    const rows = within(list).getAllByRole("listitem");
    expect(rows.map((row) => row.querySelector(".hd-db-row__name")?.textContent)).toEqual([
      "Main collection",
      "Shared collection",
    ]);
    expect(within(rows[0]).getByTitle("/home/sam/Documents/HoploDex")).toBeInTheDocument();
    expect(within(rows[1]).getByTitle("/mnt/nas/family")).toBeInTheDocument();
  });

  it("selects the database just closed", async () => {
    renderChooser({}, shared.path);

    expect(await screen.findByLabelText("Passphrase for Shared collection")).toBeInTheDocument();
    expect(screen.queryByLabelText("Passphrase for Main collection")).not.toBeInTheDocument();
  });

  it("dims a database whose file is missing and offers Locate… and Remove from list", async () => {
    vi.mocked(databasesService.getChooserState).mockResolvedValue(
      chooserState({ recent: [main, { ...shared, available: false }] }),
    );
    renderChooser();

    const list = await screen.findByRole("list", { name: "Recent databases" });
    const row = within(list).getAllByRole("listitem")[1];
    expect(row).toHaveClass("hd-db-row--unavailable");
    expect(row).toHaveTextContent("Not found at this location");
    expect(within(row).getByRole("button", { name: "Locate…" })).toBeInTheDocument();
    expect(within(row).getByRole("button", { name: "Remove from list" })).toBeInTheDocument();
    expect(within(row).queryByLabelText(/Passphrase/)).not.toBeInTheDocument();
  });

  it("locating a missing database points its entry at the file found and selects it", async () => {
    const user = userEvent.setup();
    const found = "/media/usb/Shared collection.hoplodex";
    vi.mocked(databasesService.getChooserState).mockResolvedValue(
      chooserState({ recent: [main, { ...shared, available: false }] }),
    );
    vi.mocked(openFileDialog).mockResolvedValue(found);
    vi.mocked(databasesService.locateDatabase).mockResolvedValue({ ...shared, path: found });
    renderChooser();

    await user.click(await screen.findByRole("button", { name: "Locate…" }));

    expect(databasesService.locateDatabase).toHaveBeenCalledWith(shared.path, found);
    const field = await screen.findByLabelText("Passphrase for Shared collection");
    await waitFor(() => expect(field).toHaveFocus());
    expect(screen.queryByText("Not found at this location")).not.toBeInTheDocument();
  });

  it("removing a missing database takes it off the list", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.getChooserState).mockResolvedValue(
      chooserState({ recent: [main, { ...shared, available: false }] }),
    );
    renderChooser();

    await user.click(await screen.findByRole("button", { name: "Remove from list" }));

    expect(databasesService.removeRecentDatabase).toHaveBeenCalledWith(shared.path);
    await waitFor(() => expect(screen.queryByText("Shared collection")).not.toBeInTheDocument());
  });

  it("every row can be removed from the list, which leaves the file alone", async () => {
    const user = userEvent.setup();
    renderChooser();

    await user.click(
      await screen.findByRole("button", { name: "More actions for Shared collection" }),
    );
    const item = await screen.findByRole("menuitem", { name: /Remove from list/ });
    expect(item).toHaveTextContent("The database file is not deleted.");
    await user.click(item);

    expect(databasesService.removeRecentDatabase).toHaveBeenCalledWith(shared.path);
    await waitFor(() => expect(screen.queryByText("Shared collection")).not.toBeInTheDocument());
    expect(screen.getByLabelText("Passphrase for Main collection")).toBeInTheDocument();
  });

  it("opens another database file chosen with the native dialog (US2-2)", async () => {
    const user = userEvent.setup();
    const other = "/media/usb/Club armory.hoplodex";
    vi.mocked(openFileDialog).mockResolvedValue(other);
    const session = renderChooser();

    await user.click(await screen.findByRole("button", { name: "Open another database file…" }));

    expect(openFileDialog).toHaveBeenCalledWith(
      expect.objectContaining({
        multiple: false,
        directory: false,
        filters: [
          { name: "HoploDex databases", extensions: ["hoplodex"] },
          { name: "All files", extensions: ["*"] },
        ],
      }),
    );
    const field = await screen.findByLabelText("Passphrase for Club armory");
    await waitFor(() => expect(field).toHaveFocus());
    await user.type(field, `${PASSPHRASE}{Enter}`);
    expect(session.openDatabase).toHaveBeenCalledWith(other, PASSPHRASE);
  });

  it("a cancelled file dialog changes nothing", async () => {
    const user = userEvent.setup();
    vi.mocked(openFileDialog).mockResolvedValue(null);
    renderChooser();

    await user.click(await screen.findByRole("button", { name: "Open another database file…" }));

    await waitFor(() => expect(openFileDialog).toHaveBeenCalled());
    expect(screen.getByLabelText("Passphrase for Main collection")).toBeInTheDocument();
    const list = screen.getByRole("list", { name: "Recent databases" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(2);
  });

  it.each([
    [
      "DATABASE_IN_USE",
      "Main collection is open in another copy of HoploDex, on this computer or another one. Close it there first.",
    ],
    [
      "DATABASE_NEWER_VERSION",
      "Main collection was last used by a newer version of HoploDex. Update HoploDex to open it. The file has not been changed.",
    ],
    [
      "DATABASE_UNREADABLE",
      "HoploDex can't read Main collection: it doesn't have permission to open the file, or the drive or network holding it isn't available. The file has not been changed.",
    ],
    ["DATABASE_DAMAGED", "Main collection is damaged and can't be opened."],
  ])("explains %s below the passphrase, which stays for another try", async (code, text) => {
    const user = userEvent.setup();
    renderChooser({ openDatabase: failing(code) });

    await openMain(user);

    expect(await screen.findByText(text)).toBeInTheDocument();
    const field = screen.getByLabelText("Passphrase for Main collection");
    expect(field).toBeEnabled();
    expect(field).toHaveAccessibleDescription(text);
  });

  it("says a database no longer at its location can be located or removed", async () => {
    const user = userEvent.setup();
    renderChooser({ openDatabase: failing("DATABASE_NOT_FOUND", { path: main.path }) });

    await openMain(user);

    expect(
      await screen.findByText("Main collection is no longer at this location."),
    ).toBeInTheDocument();
    const row = within(screen.getByRole("list", { name: "Recent databases" })).getAllByRole(
      "listitem",
    )[0];
    expect(within(row).getByRole("button", { name: "Locate…" })).toBeInTheDocument();
    expect(within(row).getByRole("button", { name: "Remove from list" })).toBeInTheDocument();
  });

  const since = "2026-09-25T14:30:05Z";

  it("names the other computer and when, for a database open elsewhere (FR-032)", async () => {
    const user = userEvent.setup();
    renderChooser({
      openDatabase: failing("DATABASE_OPEN_ELSEWHERE", { machineName: "Workshop PC", since }),
    });

    await openMain(user);

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(
      `Main collection is marked as open on Workshop PC since ${formatDateTime(since)}. It may still be open there, may not have been closed properly, or its latest changes may not have synced to this computer yet.`,
    );
    expect(within(alert).getByText("Workshop PC").tagName).toBe("STRONG");
    expect(within(alert).getByRole("button", { name: "Go back" })).toBeInTheDocument();
    expect(within(alert).getByRole("button", { name: "Take over…" })).toBeInTheDocument();
  });

  it("goes back to the passphrase from the open-elsewhere message", async () => {
    const user = userEvent.setup();
    renderChooser({
      openDatabase: failing("DATABASE_OPEN_ELSEWHERE", { machineName: "Workshop PC", since }),
    });
    await openMain(user);

    await user.click(await screen.findByRole("button", { name: "Go back" }));

    const field = await screen.findByLabelText("Passphrase for Main collection");
    await waitFor(() => expect(field).toHaveFocus());
    expect(screen.queryByRole("button", { name: "Take over…" })).not.toBeInTheDocument();
  });

  it("takes over only once the passphrase is typed again in the destructive confirmation", async () => {
    const user = userEvent.setup();
    let finish: () => void = () => {};
    const openDatabase = vi
      .fn()
      .mockRejectedValueOnce(
        new CommandFailure({
          code: "DATABASE_OPEN_ELSEWHERE",
          message: "open elsewhere",
          details: { machineName: "Workshop PC", since },
        }),
      )
      .mockImplementationOnce(() => new Promise<void>((resolve) => (finish = resolve)));
    renderChooser({ openDatabase });
    await openMain(user);

    await user.click(await screen.findByRole("button", { name: "Take over…" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Take over Main collection?" });
    expect(confirm).toHaveTextContent(
      "Only do this if Workshop PC no longer has Main collection open, or if it crashed. If it still has it open, or its latest changes haven't synced here yet, those changes can be lost.",
    );
    const takeOver = within(confirm).getByRole("button", { name: "Take over" });
    expect(takeOver).toHaveClass("hd-button--danger");
    // Nothing from the refused open was kept (FR-007): it is asked for again.
    const field = within(confirm).getByLabelText("Passphrase for Main collection");
    expect(field).toHaveValue("");
    await waitFor(() => expect(field).toHaveFocus());
    await user.type(field, PASSPHRASE);
    await user.click(takeOver);

    expect(openDatabase).toHaveBeenLastCalledWith(main.path, PASSPHRASE, { takeOver: true });
    expect(field).toHaveValue("");
    const opening = await screen.findByRole("button", { name: "Opening…" });
    expect(opening).toHaveAttribute("aria-busy", "true");
    expect(screen.getByRole("status")).toHaveTextContent("Opening Main collection…");
    finish();
    await waitFor(() => expect(screen.getByRole("status")).not.toHaveTextContent("Opening"));
  });

  it("won't take over without the passphrase", async () => {
    const user = userEvent.setup();
    const openDatabase = failing("DATABASE_OPEN_ELSEWHERE", { machineName: "Workshop PC", since });
    renderChooser({ openDatabase });
    await openMain(user);
    await user.click(await screen.findByRole("button", { name: "Take over…" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Take over Main collection?" });

    await user.click(within(confirm).getByRole("button", { name: "Take over" }));

    expect(
      await within(confirm).findByText("Enter the passphrase to take it over."),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("alertdialog", { name: "Take over Main collection?" }),
    ).toBeInTheDocument();
    expect(openDatabase).toHaveBeenCalledTimes(1);
  });

  it("a wrong passphrase in the take-over is refused like any other", async () => {
    const user = userEvent.setup();
    const openDatabase = vi
      .fn()
      .mockRejectedValueOnce(
        new CommandFailure({
          code: "DATABASE_OPEN_ELSEWHERE",
          message: "open elsewhere",
          details: { machineName: "Workshop PC", since },
        }),
      )
      .mockRejectedValueOnce(new CommandFailure({ code: "PASSPHRASE_INCORRECT", message: "no" }));
    renderChooser({ openDatabase });
    await openMain(user);
    await user.click(await screen.findByRole("button", { name: "Take over…" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Take over Main collection?" });

    await user.type(
      within(confirm).getByLabelText("Passphrase for Main collection"),
      "typo typo typo",
    );
    await user.click(within(confirm).getByRole("button", { name: "Take over" }));

    expect(
      await screen.findByText(
        "That passphrase didn't open Main collection. Either the passphrase is wrong, or the file isn't a HoploDex database or is damaged.",
      ),
    ).toBeInTheDocument();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(screen.getByLabelText("Passphrase for Main collection")).toBeEnabled();
  });

  it("cancelling the take-over leaves the choice open", async () => {
    const user = userEvent.setup();
    const openDatabase = failing("DATABASE_OPEN_ELSEWHERE", { machineName: "Workshop PC", since });
    renderChooser({ openDatabase });
    await openMain(user);

    await user.click(await screen.findByRole("button", { name: "Take over…" }));
    await user.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Cancel" }),
    );

    expect(openDatabase).toHaveBeenCalledTimes(1);
    expect(await screen.findByRole("button", { name: "Take over…" })).toBeInTheDocument();
  });

  it.each([
    [
      { kind: "takenOver", databasePath: main.path } as const,
      "Main collection was taken over on another computer, so HoploDex stopped saving to it here and closed it.",
    ],
    [
      { kind: "backupFailed", databasePath: main.path, reason: "databaseUnreachable" } as const,
      "Main collection was not backed up: its file could not be reached. Its changes will be backed up at the next close.",
    ],
    [
      { kind: "closed", databasePath: main.path, reason: "lockedByUser" } as const,
      "HoploDex locked Main collection.",
    ],
  ])("shows the %o notice until dismissed", async (notice, text) => {
    const user = userEvent.setup();
    vi.mocked(databasesService.getChooserState).mockResolvedValue(
      chooserState({ notices: [notice] }),
    );
    renderChooser();

    expect(await screen.findByText(text)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByText(text)).not.toBeInTheDocument();
  });

  it("says nothing about an ordinary close", async () => {
    vi.mocked(databasesService.getChooserState).mockResolvedValue(
      chooserState({ notices: [{ kind: "closed", databasePath: main.path, reason: "closed" }] }),
    );
    renderChooser();

    await screen.findByLabelText("Passphrase for Main collection");
    expect(screen.queryByRole("button", { name: "Dismiss" })).not.toBeInTheDocument();
  });
});

describe("DatabaseChooser (User Story 3)", () => {
  beforeEach(() => {
    vi.mocked(databasesService.getChooserState).mockReset().mockResolvedValue(chooserState());
    vi.mocked(databasesService.listBackups).mockReset().mockResolvedValue({
      folder: "/home/sam/Documents/HoploDex/HoploDex backups",
      available: true,
      backups: [],
    });
    vi.mocked(databasesService.onRestoreProgress)
      .mockReset()
      .mockReturnValue(() => {});
  });

  async function openFailing(failure: CommandFailure) {
    const user = userEvent.setup();
    renderChooser({ openDatabase: vi.fn().mockRejectedValue(failure) });
    await user.type(await screen.findByLabelText("Passphrase for Main collection"), "a guess");
    await user.click(screen.getByRole("button", { name: "Open" }));
    return user;
  }

  it("offers restoring a damaged database when it has backups (US3-6)", async () => {
    const user = await openFailing(
      new CommandFailure({
        code: "DATABASE_DAMAGED",
        message: "This database is damaged and can't be opened.",
        details: { backupsAvailable: true },
      }),
    );

    expect(
      await screen.findByText("Main collection is damaged and can't be opened."),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Restore from a backup…" }));

    expect(
      await screen.findByRole("dialog", { name: "Restore Main collection from a backup" }),
    ).toBeInTheDocument();
    expect(databasesService.listBackups).toHaveBeenCalledWith(main.path);
  });

  it("offers no restore for a damaged database without backups", async () => {
    await openFailing(
      new CommandFailure({
        code: "DATABASE_DAMAGED",
        message: "This database is damaged and can't be opened.",
        details: { backupsAvailable: false },
      }),
    );

    expect(
      await screen.findByText("Main collection is damaged and can't be opened."),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Restore from a backup…" }),
    ).not.toBeInTheDocument();
  });

  it("offers restoring when the passphrase didn't open a database that has backups", async () => {
    await openFailing(
      new CommandFailure({
        code: "PASSPHRASE_INCORRECT",
        message: "The passphrase is incorrect.",
        details: { backupsAvailable: true },
      }),
    );

    expect(
      await screen.findByText(/That passphrase didn't open Main collection\./),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Restore from a backup…" })).toBeInTheDocument();
  });

  it("opens the backup settings after the next open from a failed-backup notice", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.getChooserState).mockResolvedValue(
      chooserState({
        notices: [{ kind: "backupFailed", databasePath: main.path, reason: "locationUnavailable" }],
      }),
    );
    const session = renderChooser();

    expect(
      await screen.findByText(
        "Main collection was not backed up: the backup location is not available. Its changes will be backed up at the next close.",
      ),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Change backup location…" }));

    expect(session.requestSettings).toHaveBeenCalledTimes(1);
    expect(
      screen.getByText("Its backup settings will open when you open Main collection."),
    ).toBeInTheDocument();
  });

  it("says when the last backup did not finish", async () => {
    vi.mocked(databasesService.getChooserState).mockResolvedValue(
      chooserState({
        notices: [{ kind: "backupFailed", databasePath: main.path, reason: "interrupted" }],
      }),
    );
    renderChooser();

    expect(
      await screen.findByText(
        "Main collection was not backed up: the backup was interrupted. Its changes will be backed up at the next close.",
      ),
    ).toBeInTheDocument();
  });
});

describe("DatabaseChooser (User Story 5)", () => {
  const saved: RecentDatabase = { ...main, passphraseSaved: true };

  beforeEach(() => {
    vi.mocked(databasesService.getChooserState).mockReset();
  });

  it("offers remembering the passphrase, off by default", async () => {
    vi.mocked(databasesService.getChooserState).mockResolvedValue(
      chooserState({ keyringAvailable: true }),
    );
    renderChooser();

    const remember = await screen.findByRole("checkbox", { name: "Remember on this computer" });
    expect(remember).not.toBeChecked();
    expect(remember).toBeEnabled();
  });

  it("remembers only once the FR-017 confirmation is accepted", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.getChooserState).mockResolvedValue(
      chooserState({ keyringAvailable: true }),
    );
    const session = renderChooser();

    const remember = await screen.findByRole("checkbox", { name: "Remember on this computer" });
    await user.click(remember);
    const confirm = await screen.findByRole("alertdialog", {
      name: "Remember the passphrase of Main collection?",
    });
    expect(confirm).toHaveTextContent(
      "Anyone who can use this computer account, or its keyring while it's unlocked, will be able to open Main collection without knowing the passphrase. On a shared account this defeats the passphrase.",
    );
    // Not ticked while it asks, nor after a cancel.
    expect(remember).not.toBeChecked();
    await user.click(within(confirm).getByRole("button", { name: "Cancel" }));
    expect(remember).not.toBeChecked();

    await user.click(remember);
    await user.click(await screen.findByRole("button", { name: "Remember passphrase" }));
    expect(remember).toBeChecked();

    await user.type(
      screen.getByLabelText("Passphrase for Main collection"),
      "correct horse battery staple{Enter}",
    );
    expect(session.openDatabase).toHaveBeenCalledWith(main.path, "correct horse battery staple", {
      rememberPassphrase: true,
    });
  });

  it("opens a database whose passphrase is saved with Open alone", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.getChooserState).mockResolvedValue(
      chooserState({ recent: [saved, shared], keyringAvailable: true }),
    );
    const session = renderChooser();

    const open = await screen.findByRole("button", { name: "Open" });
    await waitFor(() => expect(open).toHaveFocus());
    const rows = within(screen.getByRole("list", { name: "Recent databases" })).getAllByRole(
      "listitem",
    );
    expect(rows[0]).toHaveTextContent("Opens without a passphrase on this computer");
    expect(rows[1]).not.toHaveTextContent("Opens without a passphrase on this computer");
    expect(screen.queryByLabelText("Passphrase for Main collection")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("checkbox", { name: "Remember on this computer" }),
    ).not.toBeInTheDocument();

    await user.click(open);
    expect(session.openDatabase).toHaveBeenCalledWith(saved.path, null);
  });

  it("asks for the passphrase when the saved one no longer opens it (US5-5)", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.getChooserState).mockResolvedValue(
      chooserState({ recent: [saved, shared], keyringAvailable: true }),
    );
    const session = renderChooser({
      openDatabase: vi
        .fn()
        .mockRejectedValueOnce(
          new CommandFailure({
            code: "PASSPHRASE_INCORRECT",
            message: "The passphrase is incorrect.",
            details: { savedPassphraseFailed: true },
          }),
        )
        .mockResolvedValue(undefined),
    });

    await user.click(await screen.findByRole("button", { name: "Open" }));

    const field = await screen.findByLabelText("Passphrase for Main collection");
    expect(field).toHaveAccessibleDescription(
      expect.stringContaining(
        "The saved passphrase no longer opens Main collection. Enter its passphrase; the saved copy will be updated.",
      ),
    );
    await waitFor(() => expect(field).toHaveFocus());
    await user.type(field, "the new passphrase here{Enter}");
    expect(session.openDatabase).toHaveBeenLastCalledWith(saved.path, "the new passphrase here");
  });

  it("disables remembering on a computer with no keyring (FR-019)", async () => {
    vi.mocked(databasesService.getChooserState).mockResolvedValue(
      chooserState({ keyringAvailable: false }),
    );
    renderChooser();

    const remember = await screen.findByRole("checkbox", { name: "Remember on this computer" });
    expect(remember).toBeDisabled();
    expect(
      screen.getByText("Not available: this computer has no keyring service."),
    ).toBeInTheDocument();
  });
});
