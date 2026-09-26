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
  useDirtyForm({ label: "Glock 19 (edit)", isDirty: true, submit: async () => true });
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
  });

  it("shows the open database's name on a menu button", async () => {
    renderMenu();

    const button = await screen.findByRole("button", { name: "Main collection" });
    expect(button).toHaveAttribute("aria-haspopup", "menu");
  });

  it("switches through a normal close, then shows the chooser", async () => {
    const user = userEvent.setup();
    renderMenu();

    await user.click(await screen.findByRole("button", { name: "Main collection" }));
    await user.click(await screen.findByRole("menuitem", { name: "Switch database…" }));

    await waitFor(() => expect(sessionService.closeDatabase).toHaveBeenCalledWith("switched"));
    expect(await screen.findByLabelText("Passphrase for Main collection")).toBeInTheDocument();
  });

  it("closes through a normal close", async () => {
    const user = userEvent.setup();
    renderMenu();

    await user.click(await screen.findByRole("button", { name: "Main collection" }));
    await user.click(await screen.findByRole("menuitem", { name: "Close database" }));

    await waitFor(() => expect(sessionService.closeDatabase).toHaveBeenCalledWith("closed"));
  });

  it("asks about unsaved changes before closing", async () => {
    const user = userEvent.setup();
    renderMenu(true);

    await user.click(await screen.findByRole("button", { name: "Main collection" }));
    await user.click(await screen.findByRole("menuitem", { name: "Close database" }));

    const prompt = await screen.findByRole("alertdialog", {
      name: "Save changes to Glock 19 (edit)?",
    });
    expect(sessionService.closeDatabase).not.toHaveBeenCalled();
    await user.click(within(prompt).getByRole("button", { name: "Discard changes" }));
    await waitFor(() => expect(sessionService.closeDatabase).toHaveBeenCalledWith("closed"));
  });
});
