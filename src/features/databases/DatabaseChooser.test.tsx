import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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

function renderChooser(session: Partial<SessionState> = {}) {
  const value: SessionState = {
    status: null,
    openDatabase: vi.fn().mockResolvedValue(undefined),
    createDatabase: vi.fn().mockResolvedValue(undefined),
    refreshStatus: vi.fn().mockResolvedValue(undefined),
    dismissNote: vi.fn().mockResolvedValue(undefined),
    ...session,
  };
  render(
    <SessionContext.Provider value={value}>
      <DatabaseChooser />
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
