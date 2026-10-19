import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { CLEAR_PASSPHRASE_FIELDS } from "../../components/PassphraseField";
import { formatBytes } from "../../lib/bytes";
import { formatDateTime, formatRecentDay, formatRecentMoment } from "../../lib/dates";
import { CommandFailure } from "../../services/tauriClient";
import { SessionContext } from "../session/sessionStore";
import type { SessionState } from "../session/sessionStore";
import { DatabaseChooser } from "./DatabaseChooser";
import * as databasesService from "./databasesService";
import type { ChooserState, RecentDatabase, UpgradeProgress } from "./types";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
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

function recent(name: string, folder: string, lastOpenedAt: string): RecentDatabase {
  return {
    path: `${folder}/${name}.hoplodex`,
    name,
    lastOpenedAt,
    available: true,
    passphraseSaved: false,
    backups: null,
    changedSinceLeftAt: null,
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
    lockDatabase: vi.fn().mockResolvedValue(undefined),
    refreshStatus: vi.fn().mockResolvedValue(undefined),
    dismissNote: vi.fn().mockResolvedValue(undefined),
    restoreBackup: vi.fn().mockResolvedValue(undefined),
    upgradeDatabase: vi.fn().mockResolvedValue(undefined),
    settingsRequested: false,
    requestSettings: vi.fn(),
    clearSettingsRequest: vi.fn(),
    showAbout: vi.fn(),
    ...session,
  };
  render(
    <SessionContext.Provider value={value}>
      <DatabaseChooser selectPath={selectPath} />
    </SessionContext.Provider>,
  );
  return value;
}

describe("DatabaseChooser (003 contracts/ui-databases.md §1)", () => {
  beforeEach(() => {
    vi.mocked(databasesService.getChooserState).mockReset();
  });

  // 003 US1-1
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

    const field = await screen.findByLabelText("Passphrase for “Main collection”");
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

    const field = await screen.findByLabelText("Passphrase for “Main collection”");
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

    const field = screen.getByLabelText("Passphrase for “Shared collection”");
    await waitFor(() => expect(field).toHaveFocus());
    expect(screen.queryByLabelText("Passphrase for “Main collection”")).not.toBeInTheDocument();
  });

  // 003 US1-6
  it("says a wrong passphrase didn't open it and keeps the field for another try (003 FR-006)", async () => {
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

    const field = await screen.findByLabelText("Passphrase for “Main collection”");
    await user.type(field, "not the right one");
    await user.click(screen.getByRole("button", { name: "Open" }));

    expect(
      await screen.findByText(
        "That passphrase didn't open “Main collection”. Either the passphrase is wrong, or the file isn't a HoploDex database or is damaged.",
      ),
    ).toBeInTheDocument();
    expect(session.openDatabase).toHaveBeenCalledTimes(1);
    const again = screen.getByLabelText("Passphrase for “Main collection”");
    expect(again).toBeEnabled();
    expect(again).toHaveValue("");
    expect(again).toHaveAccessibleDescription(/That passphrase didn't open “Main collection”/);
  });

  it("shows the opening state at once, before the open returns (003 SC-003)", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.getChooserState).mockResolvedValue(chooserState());
    let finish: () => void = () => {};
    renderChooser({
      openDatabase: vi.fn(() => new Promise<void>((resolve) => (finish = resolve))),
    });

    const field = await screen.findByLabelText("Passphrase for “Main collection”");
    await user.type(field, "correct horse battery staple");
    await user.click(screen.getByRole("button", { name: "Open" }));

    const opening = screen.getByRole("button", { name: "Opening…" });
    expect(opening).toHaveAttribute("aria-busy", "true");
    expect(field).toBeDisabled();
    expect(
      screen.getByRole("button", { name: "Shared collection, /mnt/nas/family" }),
    ).toBeDisabled();
    expect(screen.getByRole("status")).toHaveTextContent("Opening “Main collection”…");

    finish();
    await waitFor(() => expect(screen.getByRole("status")).not.toHaveTextContent("Opening"));
  });

  it("shows no collection data", async () => {
    vi.mocked(databasesService.getChooserState).mockResolvedValue(chooserState());
    renderChooser();

    await screen.findByLabelText("Passphrase for “Main collection”");
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

describe("DatabaseChooser (003 US2)", () => {
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
      await screen.findByLabelText("Passphrase for “Main collection”"),
      `${PASSPHRASE}{Enter}`,
    );
  }

  // 003 US2-3
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

  // 003 US2-4, US6-1
  it("selects the database just closed", async () => {
    renderChooser({}, shared.path);

    expect(await screen.findByLabelText("Passphrase for “Shared collection”")).toBeInTheDocument();
    expect(screen.queryByLabelText("Passphrase for “Main collection”")).not.toBeInTheDocument();
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

  // 011 US1-8, 011 FR-004: inside a Flatpak, a folder out of reach isn't "not found".
  it("says this copy of HoploDex can't reach a database's folder, with the fix, instead of 'not found'", async () => {
    vi.mocked(databasesService.getChooserState).mockResolvedValue(
      chooserState({
        recent: [main, { ...shared, available: false, unavailableReason: "outOfReach" }],
      }),
    );
    renderChooser();

    const list = await screen.findByRole("list", { name: "Recent databases" });
    const row = within(list).getAllByRole("listitem")[1];
    expect(row).toHaveClass("hd-db-row--unavailable");
    expect(row).toHaveTextContent("This copy of HoploDex can't reach this database's folder.");
    expect(row).toHaveTextContent(
      "give HoploDex access to /mnt/nas/family by running: flatpak override --user --filesystem=/mnt/nas/family io.github.exodious.HoploDex and then restarting HoploDex.",
    );
    expect(row).not.toHaveTextContent("Not found at this location");
    expect(within(row).getByRole("button", { name: "Locate…" })).toBeInTheDocument();
    expect(within(row).getByRole("button", { name: "Remove from list" })).toBeInTheDocument();
    expect(within(row).queryByLabelText(/Passphrase/)).not.toBeInTheDocument();
  });

  // 011 US1-8, 011 FR-004, contracts/tauri-commands.md "Commands that check reach": the
  // open-failure line shows the backend's message (its `default` case).
  it("shows an open refused as out of reach in the open-failure line, and the passphrase stays", async () => {
    const user = userEvent.setup();
    const message =
      "This copy of HoploDex is a Flatpak, and it can only reach your home folder and removable drives, so it can't use /home/sam/Documents/HoploDex/Main collection.hoplodex. Move the file into your home folder or onto a removable drive, or give HoploDex access to /home/sam/Documents/HoploDex by running: flatpak override --user --filesystem=/home/sam/Documents/HoploDex io.github.exodious.HoploDex and then restarting HoploDex.";
    renderChooser({
      openDatabase: vi.fn().mockRejectedValue(
        new CommandFailure({
          code: "LOCATION_OUT_OF_REACH",
          message,
          details: {
            path: main.path,
            folder: "/home/sam/Documents/HoploDex",
            grants: ["your home folder", "removable drives"],
          },
        }),
      ),
    });

    await openMain(user);

    expect(await screen.findByText(message)).toBeInTheDocument();
    const field = screen.getByLabelText("Passphrase for “Main collection”");
    expect(field).toBeEnabled();
    expect(field).toHaveAccessibleDescription(message);
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
    const field = await screen.findByLabelText("Passphrase for “Shared collection”");
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

  // 003 US2-5
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
    expect(screen.getByLabelText("Passphrase for “Main collection”")).toBeInTheDocument();
  });

  it("opens another database file chosen with the native dialog (003 US2-2)", async () => {
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
    const field = await screen.findByLabelText("Passphrase for “Club armory”");
    await waitFor(() => expect(field).toHaveFocus());
    // Nothing is known about a file this computer hasn't opened (003 FR-040).
    expect(screen.queryByText("Last opened here")).not.toBeInTheDocument();
    await user.type(field, `${PASSPHRASE}{Enter}`);
    expect(session.openDatabase).toHaveBeenCalledWith(other, PASSPHRASE);
  });

  it("a cancelled file dialog changes nothing", async () => {
    const user = userEvent.setup();
    vi.mocked(openFileDialog).mockResolvedValue(null);
    renderChooser();

    await user.click(await screen.findByRole("button", { name: "Open another database file…" }));

    await waitFor(() => expect(openFileDialog).toHaveBeenCalled());
    expect(screen.getByLabelText("Passphrase for “Main collection”")).toBeInTheDocument();
    const list = screen.getByRole("list", { name: "Recent databases" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(2);
  });

  it.each([
    [
      "DATABASE_IN_USE",
      "“Main collection” is open in another copy of HoploDex, on this computer or another one. Close it there first.",
    ],
    [
      "DATABASE_NEWER_VERSION",
      "“Main collection” was last used by a newer version of HoploDex, which upgraded it. Update HoploDex to open it. The file has not been changed. A backup saved before that upgrade opens in this version: look for one whose name ends in “before” and a version number, in the “HoploDex backups” folder next to the database, or the backup location you chose for it.",
    ],
    [
      "DATABASE_UNREADABLE",
      "HoploDex can't read “Main collection”: it doesn't have permission to open the file, or the drive or network holding it isn't available. The file has not been changed.",
    ],
    ["DATABASE_DAMAGED", "“Main collection” is damaged and can't be opened."],
    [
      "DISK_FULL",
      "The disk is full, so HoploDex couldn't open “Main collection”. Free some space on the drive that holds it, then try again. The file has not been changed.",
    ],
  ])("explains %s below the passphrase, which stays for another try", async (code, text) => {
    const user = userEvent.setup();
    renderChooser({ openDatabase: failing(code) });

    await openMain(user);

    expect(await screen.findByText(text)).toBeInTheDocument();
    const field = screen.getByLabelText("Passphrase for “Main collection”");
    expect(field).toBeEnabled();
    expect(field).toHaveAccessibleDescription(text);
  });

  it("says a database no longer at its location can be located or removed", async () => {
    const user = userEvent.setup();
    renderChooser({ openDatabase: failing("DATABASE_NOT_FOUND", { path: main.path }) });

    await openMain(user);

    expect(
      await screen.findByText("“Main collection” is no longer at this location."),
    ).toBeInTheDocument();
    const row = within(screen.getByRole("list", { name: "Recent databases" })).getAllByRole(
      "listitem",
    )[0];
    expect(within(row).getByRole("button", { name: "Locate…" })).toBeInTheDocument();
    expect(within(row).getByRole("button", { name: "Remove from list" })).toBeInTheDocument();
  });

  const since = "2026-09-25T14:30:05Z";

  it("names the other computer and when, for a database open elsewhere (003 FR-032)", async () => {
    const user = userEvent.setup();
    renderChooser({
      openDatabase: failing("DATABASE_OPEN_ELSEWHERE", { machineName: "Workshop PC", since }),
    });

    await openMain(user);

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(
      `“Main collection” is marked as open on Workshop PC since ${formatDateTime(since)}. It may still be open there, may not have been closed properly, or its latest changes may not have synced to this computer yet.`,
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

    const field = await screen.findByLabelText("Passphrase for “Main collection”");
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
    const confirm = await screen.findByRole("alertdialog", {
      name: "Take over “Main collection”?",
    });
    expect(confirm).toHaveTextContent(
      "Only do this if Workshop PC no longer has “Main collection” open, or if it crashed. If it still has it open, or its latest changes haven't synced here yet, those changes can be lost.",
    );
    const takeOver = within(confirm).getByRole("button", { name: "Take over" });
    expect(takeOver).toHaveClass("hd-button--danger");
    // Nothing from the refused open was kept (003 FR-007): it is asked for again.
    const field = within(confirm).getByLabelText("Passphrase for “Main collection”");
    expect(field).toHaveValue("");
    await waitFor(() => expect(field).toHaveFocus());
    await user.type(field, PASSPHRASE);
    await user.click(takeOver);

    expect(openDatabase).toHaveBeenLastCalledWith(main.path, PASSPHRASE, { takeOver: true });
    expect(field).toHaveValue("");
    const opening = await screen.findByRole("button", { name: "Opening…" });
    expect(opening).toHaveAttribute("aria-busy", "true");
    expect(screen.getByRole("status")).toHaveTextContent("Opening “Main collection”…");
    finish();
    await waitFor(() => expect(screen.getByRole("status")).not.toHaveTextContent("Opening"));
  });

  // Issue #44: Enter in the passphrase field confirms, like the button.
  it("takes over when Enter is pressed in the passphrase field", async () => {
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
      .mockResolvedValue(undefined);
    renderChooser({ openDatabase });
    await openMain(user);
    await user.click(await screen.findByRole("button", { name: "Take over…" }));
    const confirm = await screen.findByRole("alertdialog", {
      name: "Take over “Main collection”?",
    });
    const field = within(confirm).getByLabelText("Passphrase for “Main collection”");
    await waitFor(() => expect(field).toHaveFocus());

    await user.type(field, `${PASSPHRASE}{Enter}`);

    expect(openDatabase).toHaveBeenLastCalledWith(main.path, PASSPHRASE, { takeOver: true });
  });

  it("won't take over without the passphrase", async () => {
    const user = userEvent.setup();
    const openDatabase = failing("DATABASE_OPEN_ELSEWHERE", { machineName: "Workshop PC", since });
    renderChooser({ openDatabase });
    await openMain(user);
    await user.click(await screen.findByRole("button", { name: "Take over…" }));
    const confirm = await screen.findByRole("alertdialog", {
      name: "Take over “Main collection”?",
    });

    await user.click(within(confirm).getByRole("button", { name: "Take over" }));

    expect(
      await within(confirm).findByText("Enter the passphrase to take it over."),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("alertdialog", { name: "Take over “Main collection”?" }),
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
    const confirm = await screen.findByRole("alertdialog", {
      name: "Take over “Main collection”?",
    });

    await user.type(
      within(confirm).getByLabelText("Passphrase for “Main collection”"),
      "typo typo typo",
    );
    await user.click(within(confirm).getByRole("button", { name: "Take over" }));

    expect(
      await screen.findByText(
        "That passphrase didn't open “Main collection”. Either the passphrase is wrong, or the file isn't a HoploDex database or is damaged.",
      ),
    ).toBeInTheDocument();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(screen.getByLabelText("Passphrase for “Main collection”")).toBeEnabled();
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
      "“Main collection” was taken over on another computer, so HoploDex stopped saving to it here and closed it.",
    ],
    [
      { kind: "backupFailed", databasePath: main.path, reason: "databaseUnreachable" } as const,
      "“Main collection” was not backed up: its file could not be reached. Its changes will be backed up at the next close.",
    ],
    [
      { kind: "backupFailed", databasePath: main.path, reason: "insufficientSpace" } as const,
      "“Main collection” was not backed up: there isn't enough free space on the drive that holds the backups. Free some space on it, or change the backup location. Its changes will be backed up at the next close.",
    ],
    [
      { kind: "closed", databasePath: main.path, reason: "lockedByUser" } as const,
      "HoploDex locked “Main collection”.",
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

    await screen.findByLabelText("Passphrase for “Main collection”");
    expect(screen.queryByRole("button", { name: "Dismiss" })).not.toBeInTheDocument();
  });
});

describe("DatabaseChooser (003 US3)", () => {
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
    await user.type(await screen.findByLabelText("Passphrase for “Main collection”"), "a guess");
    await user.click(screen.getByRole("button", { name: "Open" }));
    return user;
  }

  it("offers restoring a damaged database when it has backups (003 US3-6)", async () => {
    const user = await openFailing(
      new CommandFailure({
        code: "DATABASE_DAMAGED",
        message: "This database is damaged and can't be opened.",
        details: { backupsAvailable: true },
      }),
    );

    expect(
      await screen.findByText("“Main collection” is damaged and can't be opened."),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Restore from a backup…" }));

    expect(
      await screen.findByRole("dialog", { name: "Restore “Main collection” from a backup" }),
    ).toBeInTheDocument();
    expect(databasesService.listBackups).toHaveBeenCalledWith(null, main.path);
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
      await screen.findByText("“Main collection” is damaged and can't be opened."),
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
      await screen.findByText(/That passphrase didn't open “Main collection”\./),
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
        "“Main collection” was not backed up: the backup location is not available. Its changes will be backed up at the next close.",
      ),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Change backup location…" }));

    expect(session.requestSettings).toHaveBeenCalledTimes(1);
    expect(
      screen.getByText("Its backup settings will open when you open “Main collection”."),
    ).toBeInTheDocument();
  });

  // 011 FR-004, data-model.md "Backup location availability"
  it("says when this copy of HoploDex can't reach the backup location, and offers a new one", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.getChooserState).mockResolvedValue(
      chooserState({
        notices: [{ kind: "backupFailed", databasePath: main.path, reason: "outOfReach" }],
      }),
    );
    const session = renderChooser();

    expect(
      await screen.findByText(
        "“Main collection” was not backed up: this copy of HoploDex (a Flatpak) can't reach the backup location. Its changes will be backed up at the next close.",
      ),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Change backup location…" }));
    expect(session.requestSettings).toHaveBeenCalledTimes(1);
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
        "“Main collection” was not backed up: the backup was interrupted. Its changes will be backed up at the next close.",
      ),
    ).toBeInTheDocument();
  });
});

describe("DatabaseChooser (003 US5)", () => {
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

  it("remembers only once the 003 FR-017 confirmation is accepted", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.getChooserState).mockResolvedValue(
      chooserState({ keyringAvailable: true }),
    );
    const session = renderChooser();

    const remember = await screen.findByRole("checkbox", { name: "Remember on this computer" });
    await user.click(remember);
    const confirm = await screen.findByRole("alertdialog", {
      name: "Remember the passphrase of “Main collection”?",
    });
    expect(confirm).toHaveTextContent(
      "Anyone who can use this computer account, or its keyring while it's unlocked, will be able to open “Main collection” without knowing the passphrase. On a shared account this defeats the passphrase.",
    );
    // Not ticked while it asks, nor after a cancel.
    expect(remember).not.toBeChecked();
    await user.click(within(confirm).getByRole("button", { name: "Cancel" }));
    expect(remember).not.toBeChecked();

    await user.click(remember);
    await user.click(await screen.findByRole("button", { name: "Remember passphrase" }));
    expect(remember).toBeChecked();

    await user.type(
      screen.getByLabelText("Passphrase for “Main collection”"),
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
    expect(screen.queryByLabelText("Passphrase for “Main collection”")).not.toBeInTheDocument();
    expect(
      screen.queryByRole("checkbox", { name: "Remember on this computer" }),
    ).not.toBeInTheDocument();

    await user.click(open);
    expect(session.openDatabase).toHaveBeenCalledWith(saved.path, null);
  });

  it("asks for the passphrase when the saved one no longer opens it (003 US5-5)", async () => {
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

    const field = await screen.findByLabelText("Passphrase for “Main collection”");
    expect(field).toHaveAccessibleDescription(
      expect.stringContaining(
        "The saved passphrase no longer opens “Main collection”. Enter its passphrase; the saved copy will be updated.",
      ),
    );
    await waitFor(() => expect(field).toHaveFocus());
    await user.type(field, "the new passphrase here{Enter}");
    expect(session.openDatabase).toHaveBeenLastCalledWith(saved.path, "the new passphrase here");
  });

  // 003 US5-4
  it("disables remembering on a computer with no keyring (003 FR-019)", async () => {
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

// 003 US6-1, US6-4a, US6-5 and US6-8: the notice a lock leaves in the chooser (the "lockedByUser" notice, for Ctrl+L, is checked above).
describe("DatabaseChooser after a lock (003 US6, contracts/ui-databases.md §1, §3, §14)", () => {
  beforeEach(() => {
    vi.mocked(databasesService.getChooserState).mockReset().mockResolvedValue(chooserState());
    vi.mocked(databasesService.onChooserNotices)
      .mockReset()
      .mockReturnValue(() => {});
  });

  it.each([
    [
      { kind: "closed", databasePath: main.path, reason: "sleep" } as const,
      "HoploDex locked “Main collection”.",
    ],
    [
      { kind: "closed", databasePath: main.path, reason: "screenLocked" } as const,
      "HoploDex locked “Main collection”.",
    ],
    [
      { kind: "closed", databasePath: main.path, reason: "idle", idleMinutes: 10 } as const,
      "HoploDex locked “Main collection” after 10 minutes without use.",
    ],
    [
      { kind: "operationStopped", databasePath: main.path, operation: "passphraseChange" } as const,
      "The computer went to sleep while the passphrase change was running, so it was stopped. “Main collection” is as it was before it started.",
    ],
    [
      { kind: "operationStopped", databasePath: main.path, operation: "backup" } as const,
      "The computer went to sleep while a backup was running, so it was stopped. “Main collection” is as it was before it started.",
    ],
    [
      {
        kind: "operationStopped",
        databasePath: main.path,
        operation: "import",
        importedCount: 12,
      } as const,
      "The computer went to sleep while an import was running, so it was stopped. 12 rows were imported before it stopped. You can import the file again; rows already imported will be found as matches.",
    ],
    [
      {
        kind: "operationStopped",
        databasePath: main.path,
        operation: "deleteBackups",
        deletedCount: 2,
      } as const,
      "The computer went to sleep while the backups of “Main collection” were being deleted, so it was stopped. 2 were deleted; the rest are still there. You can delete them in its Database settings.",
    ],
    [
      {
        kind: "operationStopped",
        databasePath: main.path,
        operation: "moveBackups",
        leftBehindCount: 2,
        folder: "/home/sam/Documents/HoploDex/HoploDex backups",
      } as const,
      "The computer went to sleep while the backups of “Main collection” were being moved, so it was stopped. 2 are still in /home/sam/Documents/HoploDex/HoploDex backups, and HoploDex no longer manages them there. The new backup location is kept.",
    ],
    [
      {
        kind: "backupsLeftBehind",
        databasePath: main.path,
        folder: "/home/sam/Documents/HoploDex/HoploDex backups",
        count: 1,
      } as const,
      "The backups of “Main collection” were not all moved. 1 is still in /home/sam/Documents/HoploDex/HoploDex backups, and HoploDex no longer manages it there.",
    ],
    [
      { kind: "pendingChangesLost", databasePath: main.path } as const,
      "Unsaved changes could not be kept when “Main collection” locked.",
    ],
  ])("says %o", async (notice, text) => {
    vi.mocked(databasesService.getChooserState).mockResolvedValue(
      chooserState({ notices: [notice] }),
    );
    renderChooser();

    expect(await screen.findByText(text)).toBeInTheDocument();
  });

  it("adds notices that come after it shows, such as what a sleep stopped", async () => {
    let noticesCame = () => {};
    vi.mocked(databasesService.onChooserNotices).mockImplementation((handler) => {
      noticesCame = handler;
      return () => {};
    });
    vi.mocked(databasesService.getChooserState)
      .mockResolvedValueOnce(
        chooserState({ notices: [{ kind: "closed", databasePath: main.path, reason: "sleep" }] }),
      )
      .mockResolvedValueOnce(
        chooserState({
          notices: [{ kind: "operationStopped", databasePath: main.path, operation: "export" }],
        }),
      );
    renderChooser(undefined, main.path);
    await screen.findByText("HoploDex locked “Main collection”.");

    noticesCame();

    expect(
      await screen.findByText(
        "The computer went to sleep while an export was running, so it was stopped. “Main collection” is as it was before it started.",
      ),
    ).toBeInTheDocument();
    expect(screen.getByText("HoploDex locked “Main collection”.")).toBeInTheDocument();
  });

  // 003 US6-1: the chooser shows the locked database selected, its passphrase prompt open.
  it("selects the locked database with its passphrase field focused", async () => {
    vi.mocked(databasesService.getChooserState).mockResolvedValue(
      chooserState({ notices: [{ kind: "closed", databasePath: shared.path, reason: "idle" }] }),
    );
    renderChooser(undefined, shared.path);

    const field = await screen.findByLabelText("Passphrase for “Shared collection”");
    await waitFor(() => expect(field).toHaveFocus());
  });

  it("focuses Open after a lock when the passphrase is saved", async () => {
    vi.mocked(databasesService.getChooserState).mockResolvedValue(
      chooserState({ recent: [main, { ...shared, passphraseSaved: true }] }),
    );
    renderChooser(undefined, shared.path);

    const open = await screen.findByRole("button", { name: "Open" });
    await waitFor(() => expect(open).toHaveFocus());
    expect(screen.queryByLabelText("Passphrase for “Shared collection”")).not.toBeInTheDocument();
  });
});

describe("What the chooser knows about the selected database (003 FR-040)", () => {
  /** A moment `days` ago, at 09:12 local time. */
  function daysAgo(days: number, hours = 9, minutes = 12): Date {
    const now = new Date();
    return new Date(now.getFullYear(), now.getMonth(), now.getDate() - days, hours, minutes);
  }

  /** As the backend names backups: local time, without a zone. */
  function local(date: Date): string {
    const pad = (n: number) => String(n).padStart(2, "0");
    return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(
      date.getHours(),
    )}:${pad(date.getMinutes())}:00`;
  }

  function fact(label: string): HTMLElement {
    const term = screen.getByText(label, { selector: "dt" });
    return term.nextElementSibling as HTMLElement;
  }

  beforeEach(() => {
    vi.mocked(databasesService.getChooserState).mockReset();
  });

  // 003 FR-040
  it("shows the selected database's last open here and its backups, and no other row's", async () => {
    const opened = daysAgo(0);
    const latest = daysAgo(1, 17, 40);
    const oldest = daysAgo(15, 20, 45);
    vi.mocked(databasesService.getChooserState).mockResolvedValue(
      chooserState({
        recent: [
          {
            ...main,
            lastOpenedAt: opened.toISOString(),
            backups: { count: 5, latestMadeAt: local(latest), oldestMadeAt: local(oldest) },
          },
          { ...shared, backups: { count: 2, latestMadeAt: local(latest), oldestMadeAt: null } },
        ],
      }),
    );
    renderChooser();

    await screen.findByLabelText("Passphrase for “Main collection”");
    expect(fact("Last opened here")).toHaveTextContent(formatRecentMoment(opened.toISOString()));
    expect(fact("Last opened here")).toHaveTextContent(/^Today at /);
    const backup = fact("Last backup");
    expect(backup).toHaveTextContent(/^Yesterday at /);
    expect(backup).toHaveTextContent(
      new RegExp(`5 kept, the oldest from ${formatRecentDay(local(oldest))}$`),
    );
    expect(screen.getAllByText("Last opened here")).toHaveLength(1);
    expect(screen.queryByText(/after it was last closed here/)).not.toBeInTheDocument();
  });

  it("says when the file changed after it was last closed here", async () => {
    const changed = daysAgo(2, 16, 20);
    vi.mocked(databasesService.getChooserState).mockResolvedValue(
      chooserState({ recent: [{ ...main, changedSinceLeftAt: changed.toISOString() }] }),
    );
    renderChooser();

    await screen.findByLabelText("Passphrase for “Main collection”");
    expect(fact("Last opened here")).toHaveTextContent(
      `Changed on ${formatRecentDay(changed.toISOString())}, after it was last closed here.`,
    );
  });

  it("says a change made today without a date", async () => {
    vi.mocked(databasesService.getChooserState).mockResolvedValue(
      chooserState({ recent: [{ ...main, changedSinceLeftAt: daysAgo(0).toISOString() }] }),
    );
    renderChooser();

    expect(
      await screen.findByText("Changed today, after it was last closed here."),
    ).toBeInTheDocument();
  });

  // 003 FR-040
  it("says there are no backups, or one, without an oldest", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.getChooserState).mockResolvedValue(
      chooserState({
        recent: [
          { ...main, backups: { count: 0, latestMadeAt: null, oldestMadeAt: null } },
          {
            ...shared,
            backups: { count: 1, latestMadeAt: local(daysAgo(3)), oldestMadeAt: local(daysAgo(3)) },
          },
        ],
      }),
    );
    renderChooser();

    await screen.findByLabelText("Passphrase for “Main collection”");
    expect(fact("Last backup")).toHaveTextContent(/^None$/);

    await user.click(screen.getByRole("button", { name: "Shared collection, /mnt/nas/family" }));

    await screen.findByLabelText("Passphrase for “Shared collection”");
    expect(fact("Last backup")).toHaveTextContent(/1 kept$/);
    expect(fact("Last backup")).not.toHaveTextContent("oldest");
  });

  it("leaves out the backups when their folder can't be read", async () => {
    vi.mocked(databasesService.getChooserState).mockResolvedValue(
      chooserState({ recent: [{ ...main, backups: null }] }),
    );
    renderChooser();

    await screen.findByLabelText("Passphrase for “Main collection”");
    expect(screen.getByText("Last opened here")).toBeInTheDocument();
    expect(screen.queryByText("Last backup")).not.toBeInTheDocument();
  });

  it("shows the details above Open when the passphrase is saved", async () => {
    vi.mocked(databasesService.getChooserState).mockResolvedValue(
      chooserState({ recent: [{ ...main, passphraseSaved: true }], keyringAvailable: true }),
    );
    renderChooser();

    const open = await screen.findByRole("button", { name: "Open" });
    const details = screen.getByText("Last opened here").closest("dl") as HTMLElement;
    expect(details.compareDocumentPosition(open) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });
});

// 009 contracts/ui.md §1-§4 and §8. The chooser asks the question and shows
// the progress; the session (`SessionState.upgradeDatabase`, like
// `openDatabase`) sends `upgrade_database` and, once it answers, opens the
// database. Cancelling is `databasesService.cancelUpgrade`, which needs no
// session.
describe("DatabaseChooser and an upgrade (009 US3)", () => {
  const PASSPHRASE = "correct horse battery staple";
  const BACKUPS = "/home/sam/Documents/HoploDex/HoploDex backups";
  const WAITING = { awaitingId: 7, version: "0.2.0", backupFolder: BACKUPS };
  const QUESTION = "Upgrade “Main collection” for HoploDex 0.2.0?";

  let sendProgress: (progress: UpgradeProgress) => void = () => {};

  function upgradeNeeded() {
    return new CommandFailure({
      code: "UPGRADE_NEEDED",
      message: "The database needs to be upgraded.",
      details: WAITING,
    });
  }

  function refusal(code: string, details: Record<string, unknown>) {
    return new CommandFailure({ code, message: `backend ${code}`, details });
  }

  beforeEach(() => {
    vi.mocked(databasesService.getChooserState).mockReset().mockResolvedValue(chooserState());
    vi.mocked(databasesService.cancelUpgrade).mockReset().mockResolvedValue(null);
    vi.mocked(databasesService.onUpgradeProgress)
      .mockReset()
      .mockImplementation((handler) => {
        sendProgress = handler;
        return () => {};
      });
    vi.mocked(openFileDialog).mockReset();
  });

  /** Opens "Main collection" with a database that waits for its upgrade. */
  async function openUpgrading(session: Partial<SessionState> = {}) {
    const user = userEvent.setup();
    const value = renderChooser({
      openDatabase: vi.fn().mockRejectedValue(upgradeNeeded()),
      ...session,
    });
    await user.type(
      await screen.findByLabelText("Passphrase for “Main collection”"),
      `${PASSPHRASE}{Enter}`,
    );
    return { user, session: value };
  }

  async function question() {
    return screen.findByRole("alertdialog", { name: QUESTION });
  }

  async function pressUpgrade(user: ReturnType<typeof userEvent.setup>) {
    await user.click(within(await question()).getByRole("button", { name: "Upgrade" }));
  }

  /** Opens it and chooses Upgrade, whose request `upgradeDatabase` answers. */
  async function upgradeRefusedWith(failure: CommandFailure) {
    const upgradeDatabase = vi.fn().mockRejectedValueOnce(failure);
    const opened = await openUpgrading({ upgradeDatabase });
    await pressUpgrade(opened.user);
    return { ...opened, upgradeDatabase };
  }

  // 009 FR-010
  it("asks before upgrading, in the words of the contract, with Upgrade focused", async () => {
    const { session } = await openUpgrading();

    const asked = await question();
    expect(asked).toHaveTextContent(
      "“Main collection” was made by an earlier version of HoploDex. To open it, this version needs to upgrade it.",
    );
    expect(asked).toHaveTextContent(
      `First, HoploDex saves a backup of it as it is now, in ${BACKUPS}.`,
    );
    expect(asked).toHaveTextContent(
      "After the upgrade, earlier versions of HoploDex can open that backup, but not “Main collection” itself.",
    );
    const upgrade = within(asked).getByRole("button", { name: "Upgrade" });
    expect(upgrade).toHaveClass("hd-button--primary");
    await waitFor(() => expect(upgrade).toHaveFocus());
    expect(within(asked).getByRole("button", { name: "Cancel" })).toBeInTheDocument();
    expect(session.openDatabase).toHaveBeenCalledTimes(1);
    expect(databasesService.cancelUpgrade).not.toHaveBeenCalled();
  });

  it("keeps the row busy while the question is up", async () => {
    await openUpgrading();
    await question();

    // The question is modal, so the page behind it is hidden from the tree.
    const busy = screen.getByRole("button", { name: "Opening…", hidden: true });
    expect(busy).toHaveAttribute("aria-busy", "true");
    expect(screen.getByLabelText("Passphrase for “Main collection”")).toBeDisabled();
  });

  // Issue #44: Enter on the focused Upgrade presses it.
  it("upgrades when Enter is pressed", async () => {
    const { user, session } = await openUpgrading();
    const upgrade = within(await question()).getByRole("button", { name: "Upgrade" });
    await waitFor(() => expect(upgrade).toHaveFocus());

    await user.keyboard("{Enter}");

    expect(session.upgradeDatabase).toHaveBeenCalledExactlyOnceWith(7);
    expect(databasesService.cancelUpgrade).not.toHaveBeenCalled();
    await waitFor(() =>
      expect(screen.queryByRole("alertdialog", { name: QUESTION })).not.toBeInTheDocument(),
    );
  });

  it("resumes the upgrade question after a take-over of a database open elsewhere", async () => {
    const user = userEvent.setup();
    const openDatabase = vi
      .fn()
      .mockRejectedValueOnce(
        refusal("DATABASE_OPEN_ELSEWHERE", {
          machineName: "Workshop PC",
          since: "2026-09-25T14:30:05Z",
        }),
      )
      .mockRejectedValueOnce(upgradeNeeded());
    const session = renderChooser({ openDatabase });
    await user.type(
      await screen.findByLabelText("Passphrase for “Main collection”"),
      `${PASSPHRASE}{Enter}`,
    );
    await user.click(await screen.findByRole("button", { name: "Take over…" }));
    const confirm = await screen.findByRole("alertdialog", {
      name: "Take over “Main collection”?",
    });
    await user.type(within(confirm).getByLabelText("Passphrase for “Main collection”"), PASSPHRASE);
    await user.click(within(confirm).getByRole("button", { name: "Take over" }));

    expect(openDatabase).toHaveBeenLastCalledWith(main.path, PASSPHRASE, { takeOver: true });
    await pressUpgrade(user);
    expect(session.upgradeDatabase).toHaveBeenCalledExactlyOnceWith(7);
  });

  // 009 FR-010: nothing was written, so cancelling leaves no trace.
  it.each([
    [
      "Cancel",
      async (user: ReturnType<typeof userEvent.setup>) =>
        user.click(within(await question()).getByRole("button", { name: "Cancel" })),
    ],
    ["Escape", async (user: ReturnType<typeof userEvent.setup>) => user.keyboard("{Escape}")],
    [
      "a sleep or screen lock",
      async () => {
        // Subscribed once the effects have run.
        await act(async () => {});
        await act(async () => events.send(CLEAR_PASSPHRASE_FIELDS));
      },
    ],
  ])("closes the question as Cancel on %s, with no message", async (_how, close) => {
    const { user, session } = await openUpgrading();
    await question();

    await close(user);

    await waitFor(() =>
      expect(screen.queryByRole("alertdialog", { name: QUESTION })).not.toBeInTheDocument(),
    );
    expect(databasesService.cancelUpgrade).toHaveBeenCalledExactlyOnceWith(7);
    expect(session.upgradeDatabase).not.toHaveBeenCalled();
    // Back to the chooser with the row selected, ready for another try.
    const field = screen.getByLabelText("Passphrase for “Main collection”");
    expect(field).toBeEnabled();
    expect(field).toHaveValue("");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open" })).toBeInTheDocument();
  });

  // 009 FR-016
  it("shows the progress dialog once the backup's first progress says showNow", async () => {
    const upgradeDatabase = vi.fn(() => new Promise<void>(() => {}));
    const { user } = await openUpgrading({ upgradeDatabase });
    await pressUpgrade(user);
    expect(screen.queryByRole("dialog", { name: "Upgrading “Main collection”" })).toBeNull();

    act(() => sendProgress({ phase: "backingUp", processed: 0, total: 212, showNow: true }));

    const progress = await screen.findByRole("dialog", { name: "Upgrading “Main collection”" });
    expect(progress).toHaveTextContent("Saving a backup of “Main collection”…");
    expect(upgradeDatabase).toHaveBeenCalledExactlyOnceWith(7);
  });

  it("Cancel in the progress dialog stops the backup and returns to the chooser without a message", async () => {
    let stopped: (failure: CommandFailure) => void = () => {};
    const upgradeDatabase = vi.fn(
      () => new Promise<void>((_resolve, reject) => (stopped = reject)),
    );
    const { user } = await openUpgrading({ upgradeDatabase });
    await pressUpgrade(user);
    act(() => sendProgress({ phase: "backingUp", processed: 10, total: 100, showNow: true }));
    const progress = await screen.findByRole("dialog", { name: "Upgrading “Main collection”" });

    await user.click(within(progress).getByRole("button", { name: "Cancel" }));

    expect(databasesService.cancelUpgrade).toHaveBeenCalledExactlyOnceWith(7);
    expect(progress).toHaveTextContent("Stopping…");

    await act(async () => stopped(refusal("OPERATION_STOPPED", { operation: "upgrade" })));

    await waitFor(() =>
      expect(
        screen.queryByRole("dialog", { name: "Upgrading “Main collection”" }),
      ).not.toBeInTheDocument(),
    );
    expect(screen.getByLabelText("Passphrase for “Main collection”")).toBeEnabled();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.queryByText(/stopped/i)).not.toBeInTheDocument();
  });

  // 009 FR-015
  it("says in the row that the upgrade failed, that nothing changed, and where the backup is", async () => {
    const backupPath = `${BACKUPS}/Main collection 2026-10-17 101500 3fa2c9d1 before 0.2.0.hoplodex`;
    await upgradeRefusedWith(refusal("UPGRADE_FAILED", { backupPath, atBackupLocation: true }));

    const text = `HoploDex couldn't upgrade “Main collection”. It is as it was before, and hasn't been opened. The backup saved before the upgrade is ${backupPath}.`;
    expect(await screen.findByText(text)).toBeInTheDocument();
    const field = screen.getByLabelText("Passphrase for “Main collection”");
    expect(field).toBeEnabled();
    expect(field).toHaveAccessibleDescription(text);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });

  // 009 FR-015
  it("adds where the backup is to the disk-full text when the upgrade ran out of room", async () => {
    const backupPath = `${BACKUPS}/Main collection 2026-10-17 101500 3fa2c9d1 before 0.2.0.hoplodex`;
    await upgradeRefusedWith(
      refusal("DISK_FULL", { path: main.path, backupPath, atBackupLocation: true }),
    );

    const text = `The disk is full, so HoploDex couldn't open “Main collection”. Free some space on the drive that holds it, then try again. The file has not been changed. The upgrade didn't happen; the backup made before it is at ${backupPath}.`;
    expect(await screen.findByText(text)).toBeInTheDocument();
    expect(screen.getByLabelText("Passphrase for “Main collection”")).toHaveAccessibleDescription(
      text,
    );
  });

  it("uses the plain disk-full text when the full disk is not the upgrade's", async () => {
    await upgradeRefusedWith(refusal("DISK_FULL", { path: main.path }));

    expect(
      await screen.findByText(
        "The disk is full, so HoploDex couldn't open “Main collection”. Free some space on the drive that holds it, then try again. The file has not been changed.",
      ),
    ).toBeInTheDocument();
  });

  // 009 FR-018
  it("says where to look for the backup from before a newer version's upgrade", async () => {
    const user = userEvent.setup();
    renderChooser({
      openDatabase: vi
        .fn()
        .mockRejectedValue(refusal("DATABASE_NEWER_VERSION", { backupFolder: BACKUPS })),
    });

    await user.type(
      await screen.findByLabelText("Passphrase for “Main collection”"),
      `${PASSPHRASE}{Enter}`,
    );

    expect(
      await screen.findByText(
        `“Main collection” was last used by a newer version of HoploDex, which upgraded it. Update HoploDex to open it. The file has not been changed. A backup saved before that upgrade opens in this version: look for one whose name ends in “before” and a version number, in ${BACKUPS}.`,
      ),
    ).toBeInTheDocument();
  });

  // 009 FR-012
  describe("when the backup can't be saved at its location", () => {
    const needed = 420_000_000;
    const free = 100_000_000;

    it.each([
      [
        "missing",
        refusal("BACKUP_LOCATION_UNAVAILABLE", { path: BACKUPS, reason: "missing" }),
        BACKUPS,
        "the folder isn't there; it may be on a drive that isn't connected",
      ],
      [
        "notWritable",
        refusal("BACKUP_LOCATION_UNAVAILABLE", { path: BACKUPS, reason: "notWritable" }),
        BACKUPS,
        "HoploDex isn't allowed to save files there",
      ],
      [
        "INSUFFICIENT_SPACE",
        refusal("INSUFFICIENT_SPACE", {
          bytesNeeded: needed,
          bytesAvailable: free,
          path: BACKUPS,
        }),
        BACKUPS,
        `there isn't enough free space (it needs ${formatBytes(needed)}; ${formatBytes(free)} is free)`,
      ],
      [
        "outOfReach",
        refusal("BACKUP_LOCATION_UNAVAILABLE", { path: BACKUPS, reason: "outOfReach" }),
        BACKUPS,
        "this copy of HoploDex (a Flatpak) can't reach that folder",
      ],
      [
        "UPGRADE_BACKUP_FAILED",
        refusal("UPGRADE_BACKUP_FAILED", { folder: BACKUPS }),
        BACKUPS,
        "saving it failed part way",
      ],
    ])("%s: offers a folder of the user's choosing", async (_name, failure, folder, reason) => {
      await upgradeRefusedWith(failure);

      const asked = await screen.findByRole("dialog", { name: "Save the backup somewhere else?" });
      expect(asked).toHaveTextContent(
        `HoploDex couldn't save the backup it makes before upgrading “Main collection” in ${folder}: ${reason}. You can save this one backup in a folder you choose instead, or cancel. “Main collection” isn't upgraded without a backup.`,
      );
      const choose = within(asked).getByRole("button", { name: "Choose a folder…" });
      expect(choose).toHaveClass("hd-button--primary");
      await waitFor(() => expect(choose).toHaveFocus());
      expect(within(asked).getByRole("button", { name: "Cancel" })).toBeInTheDocument();
    });

    async function askedForAFolder() {
      const upgradeDatabase = vi
        .fn()
        .mockRejectedValueOnce(
          refusal("BACKUP_LOCATION_UNAVAILABLE", { path: BACKUPS, reason: "missing" }),
        )
        .mockResolvedValue(undefined);
      const opened = await openUpgrading({ upgradeDatabase });
      await pressUpgrade(opened.user);
      const asked = await screen.findByRole("dialog", { name: "Save the backup somewhere else?" });
      return { ...opened, upgradeDatabase, asked };
    }

    it("upgrades with the folder the OS picker returns", async () => {
      vi.mocked(openFileDialog).mockResolvedValue("/mnt/usb/Backups");
      const { user, upgradeDatabase, asked } = await askedForAFolder();

      await user.click(within(asked).getByRole("button", { name: "Choose a folder…" }));

      expect(openFileDialog).toHaveBeenCalledWith(
        expect.objectContaining({ directory: true, multiple: false }),
      );
      await waitFor(() => expect(upgradeDatabase).toHaveBeenCalledTimes(2));
      expect(upgradeDatabase).toHaveBeenLastCalledWith(7, "/mnt/usb/Backups");
      expect(databasesService.cancelUpgrade).not.toHaveBeenCalled();
    });

    it("asks again, naming the chosen folder, when that one is refused too", async () => {
      vi.mocked(openFileDialog).mockResolvedValue("/mnt/usb/Backups");
      const upgradeDatabase = vi
        .fn()
        .mockRejectedValueOnce(
          refusal("BACKUP_LOCATION_UNAVAILABLE", { path: BACKUPS, reason: "missing" }),
        )
        .mockRejectedValueOnce(
          refusal("BACKUP_LOCATION_UNAVAILABLE", {
            path: "/mnt/usb/Backups",
            reason: "notWritable",
          }),
        );
      const { user } = await openUpgrading({ upgradeDatabase });
      await pressUpgrade(user);
      const first = await screen.findByRole("dialog", { name: "Save the backup somewhere else?" });

      await user.click(within(first).getByRole("button", { name: "Choose a folder…" }));

      await waitFor(() =>
        expect(
          screen.getByRole("dialog", { name: "Save the backup somewhere else?" }),
        ).toHaveTextContent("in /mnt/usb/Backups: HoploDex isn't allowed to save files there."),
      );
    });

    it("stays up when the picker is dismissed", async () => {
      vi.mocked(openFileDialog).mockResolvedValue(null);
      const { user, upgradeDatabase, asked } = await askedForAFolder();

      await user.click(within(asked).getByRole("button", { name: "Choose a folder…" }));

      await waitFor(() => expect(openFileDialog).toHaveBeenCalled());
      expect(
        screen.getByRole("dialog", { name: "Save the backup somewhere else?" }),
      ).toBeInTheDocument();
      expect(upgradeDatabase).toHaveBeenCalledTimes(1);
      expect(databasesService.cancelUpgrade).not.toHaveBeenCalled();
    });

    it("Cancel gives up on the upgrade and returns to the chooser with no message", async () => {
      const { user, upgradeDatabase, asked } = await askedForAFolder();

      await user.click(within(asked).getByRole("button", { name: "Cancel" }));

      expect(databasesService.cancelUpgrade).toHaveBeenCalledExactlyOnceWith(7);
      await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
      expect(upgradeDatabase).toHaveBeenCalledTimes(1);
      expect(screen.getByLabelText("Passphrase for “Main collection”")).toBeEnabled();
      expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    });
  });
});

describe("DatabaseChooser's About HoploDex button (010 US1-1, 010 FR-001)", () => {
  beforeEach(() => {
    vi.mocked(databasesService.getChooserState).mockReset().mockResolvedValue(chooserState());
  });

  it("is a ghost button in the top bar, before the theme toggle", async () => {
    renderChooser();

    const button = await screen.findByRole("button", { name: "About HoploDex" });
    expect(button).toHaveClass("hd-button--ghost");
    const topBar = screen.getByRole("banner");
    expect(topBar).toContainElement(button);
    const colorMode = within(topBar).getByRole("radiogroup", { name: "Color mode" });
    expect(
      button.compareDocumentPosition(colorMode) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it("asks the session to show About, and nothing else", async () => {
    const user = userEvent.setup();
    const session = renderChooser();

    await user.click(await screen.findByRole("button", { name: "About HoploDex" }));

    expect(session.showAbout).toHaveBeenCalledTimes(1);
    expect(session.openDatabase).not.toHaveBeenCalled();
    expect(databasesService.openDatabase).not.toHaveBeenCalled();
  });

  it("is there on a first run too, with no database to choose", async () => {
    vi.mocked(databasesService.getChooserState).mockResolvedValue(
      chooserState({ recent: [], selectedPath: null }),
    );
    renderChooser();

    expect(await screen.findByRole("button", { name: "About HoploDex" })).toBeInTheDocument();
  });
});
