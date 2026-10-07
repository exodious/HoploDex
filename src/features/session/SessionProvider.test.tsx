import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { CommandFailure } from "../../services/tauriClient";
import { DatabaseNotes } from "../databases/DatabaseNotes";
import * as databasesService from "../databases/databasesService";
import type { ChooserState, DatabaseStatus } from "../databases/types";
import { InsurancePolicyForm } from "../insurance/InsurancePolicyForm";
import type { InsurancePolicy } from "../insurance/types";
import { DocumentPreview } from "../media/DocumentPreview";
import type { DocumentSummary } from "../media/types";
import { SessionProvider } from "./SessionProvider";
import * as sessionService from "./sessionService";
import type { SessionClosed } from "./sessionService";
import { useSession } from "./sessionStore";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("../databases/databasesService");
vi.mock("./sessionService");

// The document viewer's own commands (specs/007-document-preview): everything
// else the viewer calls is `invoke`, and every other service here is mocked.
const viewerBackend = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("../../services/tauriClient", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../services/tauriClient")>()),
  invoke: viewerBackend.invoke,
}));

const setTitle = vi.hoisted(() => vi.fn(() => Promise.resolve()));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ setTitle }) }));

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

function status(diskEncryption: boolean): DatabaseStatus {
  return {
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

    expect(await screen.findByLabelText("Passphrase for “Main collection”")).toBeInTheDocument();
    expect(screen.queryByText("Collection shell")).not.toBeInTheDocument();
  });

  it("shows the collection once a database opens", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.openDatabase).mockResolvedValue(status(false));
    renderSession();

    await user.type(
      await screen.findByLabelText("Passphrase for “Main collection”"),
      `${PASSPHRASE}{Enter}`,
    );

    expect(await screen.findByText("Collection shell")).toBeInTheDocument();
    expect(databasesService.openDatabase).toHaveBeenCalledWith(PATH, PASSPHRASE);
    expect(screen.queryByLabelText("Passphrase for “Main collection”")).not.toBeInTheDocument();
  });

  it("names the open database in the window's title, and only then (§4)", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.openDatabase).mockResolvedValue(status(false));
    setTitle.mockClear();
    renderSession();

    const field = await screen.findByLabelText("Passphrase for “Main collection”");
    expect(setTitle).toHaveBeenLastCalledWith("HoploDex");
    await user.type(field, `${PASSPHRASE}{Enter}`);

    await screen.findByText("Collection shell");
    expect(setTitle).toHaveBeenLastCalledWith("Main collection — HoploDex");
  });

  it("shows the collection once a database is created", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.createDatabase).mockResolvedValue(status(true));
    renderSession();

    await user.click(await screen.findByRole("button", { name: "Create a new database…" }));
    // Nothing is open to lock, so the dialog has only its close button.
    expect(screen.queryByRole("button", { name: "Lock now" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Close" })).toBeInTheDocument();
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
    await user.click(screen.getByRole("button", { name: "Why?" }));
    expect(
      await screen.findByRole("dialog", { name: "About databases and security" }),
    ).toBeInTheDocument();
    await user.keyboard("{Escape}");

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

// --- User Story 2 -----------------------------------------------------------

const SHARED = "/mnt/nas/family/Shared collection.hoplodex";

const policy: InsurancePolicy = {
  id: 7,
  name: "Collector Floater",
  policyNumber: "CF-100",
  insuranceCompany: "Acme Mutual",
  companyContact: null,
  agentName: null,
  agentContact: null,
  notes: null,
  blanketCoverageLimit: null,
  effectiveStartDate: "2026-01-01",
  effectiveEndDate: "2027-01-01",
  createdAt: "2026-01-01 00:00:00",
  updatedAt: "2026-01-01 00:00:00",
  isInForce: true,
  isExpired: false,
  isExpiringSoon: false,
  expiringWarning: false,
  expiredWarning: false,
};

/** Stands in for the database menu. */
function CloseButtons() {
  const session = useSession();
  return (
    <>
      <button type="button" onClick={() => void session.closeDatabase("closed")}>
        Close it
      </button>
      <button type="button" onClick={() => void session.closeDatabase("switched")}>
        Switch it
      </button>
    </>
  );
}

describe("SessionProvider (User Story 2: close, switch and quit, FR-010)", () => {
  let sessionClosed: (closed: SessionClosed) => void;
  let quitRequested: () => void;

  beforeEach(() => {
    vi.mocked(sessionService.getDatabaseStatus).mockReset().mockResolvedValue(status(false));
    vi.mocked(sessionService.closeDatabase).mockReset().mockResolvedValue({
      backup: "notAttempted",
    });
    vi.mocked(sessionService.quitApplication).mockReset().mockResolvedValue(undefined);
    vi.mocked(sessionService.onSessionClosed)
      .mockReset()
      .mockImplementation((handler) => {
        sessionClosed = handler;
        return () => {};
      });
    vi.mocked(sessionService.onQuitRequested)
      .mockReset()
      .mockImplementation((handler) => {
        quitRequested = handler;
        return () => {};
      });
    vi.mocked(databasesService.getChooserState)
      .mockReset()
      .mockResolvedValue({
        ...chooser,
        recent: [
          ...chooser.recent,
          { ...chooser.recent[0], path: SHARED, name: "Shared collection" },
        ],
      });
  });

  async function renderOpen(form: ReactNode) {
    render(
      <SessionProvider>
        <CloseButtons />
        {form}
        <p>Collection shell</p>
      </SessionProvider>,
    );
    await screen.findByText("Collection shell");
  }

  const prompt = () =>
    screen.findByRole("alertdialog", { name: "Save changes to New insurance policy?" });

  it("closes a clean form's database without asking", async () => {
    const user = userEvent.setup();
    await renderOpen(<InsurancePolicyForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("button", { name: "Close it" }));

    expect(sessionService.closeDatabase).toHaveBeenCalledWith("closed");
    expect(await screen.findByLabelText("Passphrase for “Main collection”")).toBeInTheDocument();
    expect(screen.queryByText("Collection shell")).not.toBeInTheDocument();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });

  it("asks save, discard or cancel about a form with unsaved input", async () => {
    const user = userEvent.setup();
    await renderOpen(<InsurancePolicyForm onSubmit={vi.fn()} />);
    await user.type(screen.getByLabelText(/Policy name/), "Home");

    await user.click(screen.getByRole("button", { name: "Switch it" }));

    const dialog = await prompt();
    expect(
      within(dialog)
        .getAllByRole("button")
        .map((b) => b.textContent),
    ).toEqual(["Cancel", "Discard changes", "Save changes"]);
    expect(sessionService.closeDatabase).not.toHaveBeenCalled();
  });

  it("keeps everything open on cancel", async () => {
    const user = userEvent.setup();
    await renderOpen(<InsurancePolicyForm onSubmit={vi.fn()} />);
    await user.type(screen.getByLabelText(/Policy name/), "Home");
    await user.click(screen.getByRole("button", { name: "Close it" }));

    await user.click(within(await prompt()).getByRole("button", { name: "Cancel" }));

    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(sessionService.closeDatabase).not.toHaveBeenCalled();
    expect(screen.getByLabelText(/Policy name/)).toHaveValue("Home");
    expect(screen.getByText("Collection shell")).toBeInTheDocument();
  });

  it("closes without saving on discard", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    await renderOpen(<InsurancePolicyForm onSubmit={onSubmit} />);
    await user.type(screen.getByLabelText(/Policy name/), "Home");
    await user.click(screen.getByRole("button", { name: "Switch it" }));

    await user.click(within(await prompt()).getByRole("button", { name: "Discard changes" }));

    await waitFor(() => expect(sessionService.closeDatabase).toHaveBeenCalledWith("switched"));
    expect(onSubmit).not.toHaveBeenCalled();
    expect(await screen.findByLabelText("Passphrase for “Main collection”")).toBeInTheDocument();
  });

  it("saves through the form's own submit, then closes", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    await renderOpen(<InsurancePolicyForm initialValues={policy} onSubmit={onSubmit} />);
    await user.type(screen.getByLabelText("Notes"), "Renews in January.");
    await user.click(screen.getByRole("button", { name: "Close it" }));

    const dialog = await screen.findByRole("alertdialog", {
      name: "Save changes to Collector Floater (edit)?",
    });
    await user.click(within(dialog).getByRole("button", { name: "Save changes" }));

    await waitFor(() => expect(sessionService.closeDatabase).toHaveBeenCalledWith("closed"));
    expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ notes: "Renews in January." }));
    expect(onSubmit.mock.invocationCallOrder[0]).toBeLessThan(
      vi.mocked(sessionService.closeDatabase).mock.invocationCallOrder[0],
    );
  });

  it("closes nothing when the save fails validation, and the form shows why", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    await renderOpen(<InsurancePolicyForm onSubmit={onSubmit} />);
    await user.type(screen.getByLabelText(/Policy name/), "Home");
    await user.click(screen.getByRole("button", { name: "Close it" }));

    await user.click(within(await prompt()).getByRole("button", { name: "Save changes" }));

    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(onSubmit).not.toHaveBeenCalled();
    expect(sessionService.closeDatabase).not.toHaveBeenCalled();
    expect(screen.getByText("Enter the policy number.")).toBeInTheDocument();
    expect(screen.getByLabelText(/Policy name/)).toHaveValue("Home");
  });

  it("asks the same question when the window is closed, then quits", async () => {
    const user = userEvent.setup();
    await renderOpen(<InsurancePolicyForm onSubmit={vi.fn()} />);
    await user.type(screen.getByLabelText(/Policy name/), "Home");

    act(() => quitRequested());

    await user.click(within(await prompt()).getByRole("button", { name: "Discard changes" }));
    await waitFor(() => expect(sessionService.quitApplication).toHaveBeenCalledTimes(1));
    expect(sessionService.closeDatabase).not.toHaveBeenCalled();
  });

  it("quits at once when nothing is unsaved", async () => {
    await renderOpen(<InsurancePolicyForm onSubmit={vi.fn()} />);

    act(() => quitRequested());

    await waitFor(() => expect(sessionService.quitApplication).toHaveBeenCalledTimes(1));
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });

  it("drops the collection when the backend closes the database, and selects it", async () => {
    await renderOpen(<InsurancePolicyForm onSubmit={vi.fn()} />);

    act(() => sessionClosed({ reason: "takenOver", databasePath: SHARED }));

    expect(await screen.findByLabelText("Passphrase for “Shared collection”")).toBeInTheDocument();
    expect(screen.queryByText("Collection shell")).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/Policy name/)).not.toBeInTheDocument();
  });

  it("keeps the form and the collection when a save can't reach the file", async () => {
    const user = userEvent.setup();
    const unreachable =
      "HoploDex can't reach /media/usb/Main collection.hoplodex. Nothing already saved was lost. Close the database and open it again once the drive or network is back.";
    const onSubmit = vi.fn().mockRejectedValue(
      new CommandFailure({
        code: "DATABASE_UNAVAILABLE",
        message: unreachable,
        details: { path: "/media/usb/Main collection.hoplodex" },
      }),
    );
    await renderOpen(<InsurancePolicyForm initialValues={policy} onSubmit={onSubmit} />);
    await user.type(screen.getByLabelText("Notes"), "Renews in January.");

    await user.click(screen.getByRole("button", { name: "Save changes" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(unreachable);
    expect(screen.getByLabelText("Notes")).toHaveValue("Renews in January.");
    expect(screen.getByText("Collection shell")).toBeInTheDocument();
    expect(sessionService.closeDatabase).not.toHaveBeenCalled();
  });
});

// --- 007: the document viewer (SC-006, contracts/ui-document-preview.md §9) ----

const notesDocument: DocumentSummary = {
  id: 3,
  owner: { kind: "firearm", id: 1 },
  originalFilename: "Notes.txt",
  mimeType: "text/plain",
  createdAt: "2026-03-14 09:30:00",
  previewKind: "text",
  previewAvailable: true,
  openable: true,
};

describe("SessionProvider (007: the document viewer, SC-006)", () => {
  let sessionClosed: (closed: SessionClosed) => void;

  beforeEach(() => {
    vi.mocked(sessionService.getDatabaseStatus).mockReset().mockResolvedValue(status(false));
    vi.mocked(sessionService.onSessionClosed)
      .mockReset()
      .mockImplementation((handler) => {
        sessionClosed = handler;
        return () => {};
      });
    vi.mocked(databasesService.getChooserState).mockReset().mockResolvedValue(chooser);
    viewerBackend.invoke.mockReset().mockImplementation(async (command: string) => {
      if (command === "open_preview") {
        return {
          previewId: 31,
          documentId: 3,
          kind: "text",
          text: "Serial BXKT482 on the receipt",
        };
      }
      if (command === "close_preview") return null;
      throw new Error(`unexpected command ${command}`);
    });
  });

  it.each(["lockedByUser", "idle", "screenLocked"] as const)(
    "a lock (%s) unmounts the viewer before the chooser is rendered",
    async (reason) => {
      render(
        <SessionProvider>
          <DocumentPreview
            documents={[notesDocument]}
            index={0}
            documentTypes={[
              {
                label: "Plain text",
                extensions: ["txt"],
                mimeType: "text/plain",
                previewKind: "text",
              },
            ]}
            onIndexChange={() => {}}
            onClose={() => {}}
            onDelete={() => {}}
          />
        </SessionProvider>,
      );
      await screen.findByRole("dialog", { name: "Notes.txt" });
      await screen.findByText("Serial BXKT482 on the receipt");

      // At no point may the viewer and the chooser be on the screen together.
      let together = false;
      const watch = new MutationObserver(() => {
        if (
          document.querySelector('[role="dialog"]') &&
          screen.queryByLabelText("Passphrase for “Main collection”")
        ) {
          together = true;
        }
      });
      watch.observe(document.body, { childList: true, subtree: true });

      act(() => sessionClosed({ reason, databasePath: PATH }));

      expect(await screen.findByLabelText("Passphrase for “Main collection”")).toBeInTheDocument();
      await Promise.resolve();
      watch.disconnect();
      expect(together).toBe(false);
      expect(document.querySelector('[role="dialog"]')).toBeNull();
      expect(screen.queryByText("Serial BXKT482 on the receipt")).not.toBeInTheDocument();
      // Its cleanup ran with the rest of the collection's tree.
      expect(viewerBackend.invoke).toHaveBeenCalledWith("close_preview", { previewId: 31 });
    },
  );
});
