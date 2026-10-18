import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import { ToastProvider, useToast } from "../../components";
import type { Notify } from "../../components/toastContext";
import { CommandFailure } from "../../services/tauriClient";
import { DatabaseMenu } from "../databases/DatabaseMenu";
import { DatabaseNotes } from "../databases/DatabaseNotes";
import * as databasesService from "../databases/databasesService";
import type { ChooserState, DatabaseStatus, Draft, PendingSummary } from "../databases/types";
import { FORM_VERSION, InsurancePolicyForm } from "../insurance/InsurancePolicyForm";
import type { InsurancePolicy } from "../insurance/types";
import { DocumentPreview } from "../media/DocumentPreview";
import type { DocumentSummary } from "../media/types";
import { SessionProvider } from "./SessionProvider";
import { useSessionScope } from "./sessionScope";
import type { SessionScope } from "./sessionScope";
import * as sessionService from "./sessionService";
import type { SessionClosed, SessionClosing } from "./sessionService";
import { useSession } from "./sessionStore";
import type { SessionState } from "./sessionStore";

/** What the session scope of the open (session id 1) looks like to a mocked service. */
const SCOPE = expect.objectContaining({ id: 1 });
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

// Counts every call of `useToast`, and otherwise leaves it as it is: the guard
// of 008 research.md §4 reads the count.
const toastConsumers = vi.hoisted(() => ({ spy: vi.fn() }));
vi.mock("../../components/toastContext", async (importOriginal) => {
  const original = await importOriginal<typeof import("../../components/toastContext")>();
  toastConsumers.spy.mockImplementation(original.useToast);
  return { ...original, useToast: toastConsumers.spy };
});

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
      diskEncryption,
      openedBackup: null,
      restoredWithPassphraseOf: null,
      damagedFileKeptAt: null,
      upgraded: null,
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

describe("SessionProvider (003 US1)", () => {
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

  it("names the open database in the window's title, and only then (003 contracts/ui-databases.md §4)", async () => {
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

  // 003 US1-4
  it("shows the disk-encryption note until it is dismissed (003 FR-008)", async () => {
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

    expect(sessionService.dismissNote).toHaveBeenCalledWith(SCOPE, "diskEncryption");
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

// --- 003 US2 ----------------------------------------------------------------

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

describe("SessionProvider (003 US2, FR-010: close, switch and quit)", () => {
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

    expect(sessionService.closeDatabase).toHaveBeenCalledWith(SCOPE, "closed");
    expect(await screen.findByLabelText("Passphrase for “Main collection”")).toBeInTheDocument();
    expect(screen.queryByText("Collection shell")).not.toBeInTheDocument();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });

  // 003 FR-010
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

  // 003 FR-010
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

    await waitFor(() =>
      expect(sessionService.closeDatabase).toHaveBeenCalledWith(SCOPE, "switched"),
    );
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

    await waitFor(() => expect(sessionService.closeDatabase).toHaveBeenCalledWith(SCOPE, "closed"));
    expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ notes: "Renews in January." }));
    expect(onSubmit.mock.invocationCallOrder[0]).toBeLessThan(
      vi.mocked(sessionService.closeDatabase).mock.invocationCallOrder[0],
    );
  });

  // 003 FR-010
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

    act(() => sessionClosed({ sessionId: 1, reason: "takenOver", databasePath: SHARED }));

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

// --- 007: the document viewer (007 SC-006, contracts/ui-document-preview.md §9) ----

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

describe("SessionProvider (007: the document viewer, 007 SC-006)", () => {
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

      act(() => sessionClosed({ sessionId: 1, reason, databasePath: PATH }));

      expect(await screen.findByLabelText("Passphrase for “Main collection”")).toBeInTheDocument();
      await Promise.resolve();
      watch.disconnect();
      expect(together).toBe(false);
      expect(document.querySelector('[role="dialog"]')).toBeNull();
      expect(screen.queryByText("Serial BXKT482 on the receipt")).not.toBeInTheDocument();
      // Its cleanup ran with the rest of the collection's tree, after the scope ended: the
      // close is refused on this side, and the backend closed the preview with the session.
      expect(viewerBackend.invoke).not.toHaveBeenCalledWith(
        "close_preview",
        expect.anything(),
        expect.anything(),
      );
    },
  );
});

// --- 008: the session scope (008 FR-002, research.md §3) -------------------------

describe("SessionProvider (008: the session scope)", () => {
  let sessionClosed: (closed: SessionClosed) => void;
  let sessionClosing: (closing: SessionClosing) => void;
  /** Every scope the collection's tree rendered with, and whether the one
   * before it had ended by then. */
  let scopes: SessionScope[];
  let previousEndedAtRender: boolean[];

  function ScopeProbe() {
    const scope = useSessionScope();
    if (scopes[scopes.length - 1] !== scope) {
      previousEndedAtRender.push(scopes.length === 0 || scopes[scopes.length - 1].ended);
      scopes.push(scope);
    }
    return null;
  }

  function RestoreButton() {
    const session = useSession();
    return (
      <button type="button" onClick={() => void session.restoreBackup("/backup", "pass")}>
        Restore it
      </button>
    );
  }

  const withId = (sessionId: number): DatabaseStatus => ({ ...status(false), sessionId });
  const current = () => scopes[scopes.length - 1];

  beforeEach(() => {
    scopes = [];
    previousEndedAtRender = [];
    vi.mocked(sessionService.getDatabaseStatus).mockReset().mockResolvedValue(withId(1));
    vi.mocked(sessionService.lockDatabase)
      .mockReset()
      .mockResolvedValue({ backup: "notAttempted" });
    vi.mocked(sessionService.closeDatabase)
      .mockReset()
      .mockResolvedValue({ backup: "notAttempted" });
    vi.mocked(sessionService.onSessionClosing)
      .mockReset()
      .mockImplementation((handler) => {
        sessionClosing = handler;
        return () => {};
      });
    vi.mocked(sessionService.onSessionClosed)
      .mockReset()
      .mockImplementation((handler) => {
        sessionClosed = handler;
        return () => {};
      });
    vi.mocked(sessionService.onQuitRequested)
      .mockReset()
      .mockReturnValue(() => {});
    vi.mocked(databasesService.getChooserState).mockReset().mockResolvedValue(chooser);
    vi.mocked(databasesService.restoreBackup).mockReset();
  });

  async function renderOpen() {
    render(
      <SessionProvider>
        <ScopeProbe />
        <RestoreButton />
        <CloseButtons />
        <p>Collection shell</p>
      </SessionProvider>,
    );
    await screen.findByText("Collection shell");
  }

  it("gives the collection one live scope per open, named by the status's session id", async () => {
    await renderOpen();

    expect(scopes).toHaveLength(1);
    expect(current().id).toBe(1);
    expect(current().ended).toBe(false);
  });

  it("ends the scope on session:closing of its own session", async () => {
    await renderOpen();

    act(() => sessionClosing({ sessionId: 1, reason: "closed" }));

    expect(current().ended).toBe(true);
    expect(
      await screen.findByRole("heading", { name: "Closing “Main collection”…" }),
    ).toBeInTheDocument();
  });

  it("ends the scope on session:closed of its own session", async () => {
    await renderOpen();

    act(() => sessionClosed({ sessionId: 1, reason: "takenOver", databasePath: PATH }));

    expect(current().ended).toBe(true);
    expect(await screen.findByLabelText("Passphrase for “Main collection”")).toBeInTheDocument();
  });

  it("ends the scope as soon as Lock now has sent lock_database", async () => {
    const user = userEvent.setup();
    let finishLock: (outcome: { backup: "notAttempted" }) => void = () => {};
    vi.mocked(sessionService.lockDatabase).mockReturnValue(
      new Promise((resolve) => {
        finishLock = resolve;
      }),
    );
    await renderOpen();
    expect(current().ended).toBe(false);

    await user.keyboard("{Control>}l{/Control}");

    expect(sessionService.lockDatabase).toHaveBeenCalledTimes(1);
    expect(current().ended).toBe(true);
    finishLock({ backup: "notAttempted" });
  });

  it("ends the scope when a lock finds the database closed already", async () => {
    const user = userEvent.setup();
    vi.mocked(sessionService.lockDatabase).mockRejectedValue(closed);
    await renderOpen();

    await user.keyboard("{Control>}l{/Control}");

    await waitFor(() => expect(current().ended).toBe(true));
  });

  it("ends the previous scope before a new open's collection renders", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.restoreBackup).mockResolvedValue(withId(2));
    await renderOpen();
    const first = current();

    await user.click(screen.getByRole("button", { name: "Restore it" }));

    await waitFor(() => expect(scopes).toHaveLength(2));
    expect(first.ended).toBe(true);
    expect(current().id).toBe(2);
    expect(current().ended).toBe(false);
    // When the second scope first rendered, the first was already dead.
    expect(previousEndedAtRender).toEqual([true, true]);
  });

  it("ends the scope when the user closes to the chooser", async () => {
    const user = userEvent.setup();
    await renderOpen();

    await user.click(screen.getByRole("button", { name: "Close it" }));

    expect(await screen.findByLabelText("Passphrase for “Main collection”")).toBeInTheDocument();
    expect(current().ended).toBe(true);
  });

  it("leaves the new scope alone when an old session's events arrive late", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.restoreBackup).mockResolvedValue(withId(2));
    await renderOpen();
    await user.click(screen.getByRole("button", { name: "Restore it" }));
    await waitFor(() => expect(scopes).toHaveLength(2));

    act(() => sessionClosing({ sessionId: 1, reason: "closed" }));
    act(() => sessionClosed({ sessionId: 1, reason: "lockedByUser", databasePath: PATH }));

    expect(current().id).toBe(2);
    expect(current().ended).toBe(false);
    expect(screen.getByText("Collection shell")).toBeInTheDocument();
    expect(screen.queryByLabelText("Passphrase for “Main collection”")).not.toBeInTheDocument();
  });

  it("ignores an event that names no session", async () => {
    await renderOpen();

    act(() => sessionClosing({ reason: "closed" } as SessionClosing));
    act(() => sessionClosed({ reason: "idle", databasePath: PATH } as SessionClosed));

    expect(current().ended).toBe(false);
    expect(screen.getByText("Collection shell")).toBeInTheDocument();
  });
});

// --- 008: toasts go with the session (008 US1-6, FR-005, research.md §4) ----------

describe("SessionProvider (008: notifications go with the session)", () => {
  let sessionClosed: (closed: SessionClosed) => void;
  let sessionClosing: (closing: SessionClosing) => void;
  /** The `notify` the collection's tree got, kept past the tree's end as the
   * closure of work begun in that session would. */
  let notify: Notify;

  function ToastProbe() {
    notify = useToast();
    return <p>Collection shell</p>;
  }

  const idleNotice = "HoploDex locked “Main collection” after 10 minutes without use.";

  beforeEach(() => {
    vi.mocked(sessionService.getDatabaseStatus)
      .mockReset()
      .mockResolvedValue({ ...status(false), sessionId: 1 });
    vi.mocked(sessionService.onSessionClosing)
      .mockReset()
      .mockImplementation((handler) => {
        sessionClosing = handler;
        return () => {};
      });
    vi.mocked(sessionService.onSessionClosed)
      .mockReset()
      .mockImplementation((handler) => {
        sessionClosed = handler;
        return () => {};
      });
    vi.mocked(sessionService.onQuitRequested)
      .mockReset()
      .mockReturnValue(() => {});
    vi.mocked(databasesService.getChooserState)
      .mockReset()
      .mockResolvedValue({
        ...chooser,
        notices: [{ kind: "closed", reason: "idle", databasePath: PATH, idleMinutes: 10 }],
      });
  });

  async function renderOpen() {
    // As App.tsx has it today: a provider above the session. The session's
    // own, inside the tree keyed by the open, is the one that must count.
    render(
      <ToastProvider>
        <SessionProvider>
          <ToastProbe />
        </SessionProvider>
      </ToastProvider>,
    );
    await screen.findByText("Collection shell");
  }

  it("removes every toast of the session when it ends, and keeps the chooser's notice", async () => {
    await renderOpen();
    act(() => notify("Added 1 photo."));
    expect(screen.getByText("Added 1 photo.")).toBeInTheDocument();

    act(() => sessionClosed({ sessionId: 1, reason: "idle", databasePath: PATH }));

    expect(await screen.findByText(idleNotice)).toBeInTheDocument();
    expect(screen.queryByText("Added 1 photo.")).not.toBeInTheDocument();
  });

  it("shows nothing for a notify from the ended session, late", async () => {
    await renderOpen();
    const fromTheEndedSession = notify;
    act(() => sessionClosed({ sessionId: 1, reason: "idle", databasePath: PATH }));
    expect(await screen.findByText(idleNotice)).toBeInTheDocument();

    act(() => fromTheEndedSession("Attached 2 documents."));
    await act(async () => void (await new Promise((r) => setTimeout(r, 30))));

    expect(screen.queryByText("Attached 2 documents.")).not.toBeInTheDocument();
    expect(screen.getByText(idleNotice)).toBeInTheDocument();
  });

  it("removes the toasts as the closing screen replaces the collection", async () => {
    await renderOpen();
    act(() => notify("Photo deleted."));
    expect(screen.getByText("Photo deleted.")).toBeInTheDocument();

    act(() => sessionClosing({ sessionId: 1, reason: "closed" }));

    expect(
      await screen.findByRole("heading", { name: "Closing “Main collection”…" }),
    ).toBeInTheDocument();
    expect(screen.queryByText("Photo deleted.")).not.toBeInTheDocument();
    act(() => notify("Too late."));
    expect(screen.queryByText("Too late.")).not.toBeInTheDocument();
  });

  it("renders no useToast consumer outside the tree keyed by the open (008 research.md §4)", async () => {
    const user = userEvent.setup();
    vi.mocked(sessionService.getDatabaseStatus).mockRejectedValue(closed);
    render(
      <SessionProvider>
        <ToastProbe />
      </SessionProvider>,
    );

    // The chooser, with a dialog of its own open.
    toastConsumers.spy.mockClear();
    await screen.findByLabelText("Passphrase for “Main collection”");
    await user.click(screen.getByRole("button", { name: "Create a new database…" }));
    await screen.findByRole("dialog");
    expect(toastConsumers.spy).not.toHaveBeenCalled();
    await user.keyboard("{Escape}");

    // The pending changes dialog, before the collection can show.
    vi.mocked(databasesService.openDatabase).mockResolvedValue({
      ...status(false),
      sessionId: 2,
      pendingChanges: {
        formVersion: FORM_VERSION,
        kind: "policy",
        mode: "edit",
        targetId: 7,
        label: "Collector Floater (edit)",
        savedAt: "2026-09-25T12:30:05Z",
        resumable: true,
        savedByVersion: "1.3.0",
        appVersion: "1.3.0",
      },
    });
    await user.type(
      screen.getByLabelText("Passphrase for “Main collection”"),
      `${PASSPHRASE}{Enter}`,
    );
    await screen.findByRole("dialog", { name: "Unsaved changes to Collector Floater (edit)" });
    expect(screen.queryByText("Collection shell")).not.toBeInTheDocument();
    expect(toastConsumers.spy).not.toHaveBeenCalled();
  });

  it("renders no useToast consumer on the closing screen", async () => {
    await renderOpen();
    // The collection's tree consumes toasts, so the spy does count.
    expect(toastConsumers.spy).toHaveBeenCalled();
    toastConsumers.spy.mockClear();

    act(() => sessionClosing({ sessionId: 1, reason: "closed" }));
    await screen.findByRole("heading", { name: "Closing “Main collection”…" });

    expect(toastConsumers.spy).not.toHaveBeenCalled();
  });
});

// --- 008: resuming pending changes until their form opens (008 US1-3, US1-4) -----

describe("SessionProvider (008: resuming pending changes)", () => {
  let sessionClosed: (closed: SessionClosed) => void;
  /** Whether the form has opened yet in the collection's tree, for a test to
   * choose; and how it opens. */
  let formAllowed: boolean;
  let openForm: () => void;
  let failWith: string | null;
  /** How many times the collection's tree has mounted. */
  let mounts: number;

  const pending: PendingSummary = {
    formVersion: FORM_VERSION,
    kind: "policy",
    mode: "edit",
    targetId: 7,
    label: "Collector Floater (edit)",
    savedAt: "2026-09-25T12:30:05Z",
    resumable: true,
    savedByVersion: "1.3.0",
    appVersion: "1.3.0",
  };
  const draft: Draft = {
    formVersion: FORM_VERSION,
    kind: "policy",
    mode: "edit",
    targetId: 7,
    label: "Collector Floater (edit)",
    values: {
      name: "Collector Floater",
      policyNumber: "CF-100",
      insuranceCompany: "Acme Mutual",
      companyContact: "",
      agentName: "",
      agentContact: "",
      notes: "Kept at the lock",
      blanketCoverageLimit: "",
      effectiveStartDate: "2026-01-01",
      effectiveEndDate: "2027-01-01",
    },
  };
  const dialogName = "Unsaved changes to Collector Floater (edit)";
  const withPending = (sessionId: number): DatabaseStatus => ({
    ...status(false),
    sessionId,
    pendingChanges: pending,
  });

  /** What `resolve_pending_changes` answers each action with. */
  function answer(action: string) {
    switch (action) {
      case "resume":
        return { draft };
      case "notOpened":
        return { pending };
      default:
        return {};
    }
  }

  /** Every action sent to `resolve_pending_changes`, in order, whether the
   * scope or the session service sends it. */
  function actionsSent(): string[] {
    const viaService = vi
      .mocked(sessionService.resolvePendingChanges)
      .mock.calls.map((call) => call[call.length - 1] as string);
    const viaInvoke = viewerBackend.invoke.mock.calls
      .filter(([command]) => command === "resolve_pending_changes")
      .map(([, args]) => (args as { action: string }).action);
    return [...viaService, ...viaInvoke];
  }

  /** Stands in for the page whose form the resumed changes belong to. It
   * opens the form when the test says so, or reports that it can't. */
  function ResumePage() {
    const scope = useSessionScope();
    const [shown, setShown] = useState(formAllowed);
    openForm = () => setShown(true);
    useEffect(() => {
      mounts += 1;
    }, []);
    useEffect(() => {
      if (failWith && scope.resumed) void scope.resumeFailed(failWith);
    }, [scope]);
    return (
      <>
        <button type="button">Collection button</button>
        {shown && <InsurancePolicyForm initialValues={policy} onSubmit={vi.fn()} />}
      </>
    );
  }

  beforeEach(() => {
    formAllowed = false;
    failWith = null;
    mounts = 0;
    vi.mocked(sessionService.getDatabaseStatus).mockReset().mockResolvedValue(withPending(1));
    vi.mocked(sessionService.resolvePendingChanges)
      .mockReset()
      .mockImplementation(async (...args: unknown[]) => answer(args[args.length - 1] as string));
    viewerBackend.invoke.mockReset().mockImplementation(async (command: string, args?: unknown) => {
      if (command === "resolve_pending_changes") return answer((args as { action: string }).action);
      return undefined;
    });
    vi.mocked(sessionService.lockDatabase)
      .mockReset()
      .mockResolvedValue({ backup: "notAttempted" });
    vi.mocked(sessionService.onSessionClosing)
      .mockReset()
      .mockReturnValue(() => {});
    vi.mocked(sessionService.onSessionClosed)
      .mockReset()
      .mockImplementation((handler) => {
        sessionClosed = handler;
        return () => {};
      });
    vi.mocked(sessionService.onQuitRequested)
      .mockReset()
      .mockReturnValue(() => {});
    vi.mocked(databasesService.getChooserState).mockReset().mockResolvedValue(chooser);
    vi.mocked(databasesService.openDatabase).mockReset();
  });

  function renderPending() {
    render(
      <SessionProvider>
        <ResumePage />
      </SessionProvider>,
    );
  }
  const dialog = () => screen.findByRole("dialog", { name: dialogName });

  it("shows Opening… on Resume editing until the form opens, then closes the dialog on opened", async () => {
    const user = userEvent.setup();
    renderPending();
    await user.click(within(await dialog()).getByRole("button", { name: "Resume editing" }));

    // The collection mounts beneath the dialog, which stays up.
    const opening = await within(screen.getByRole("dialog", { name: dialogName })).findByRole(
      "button",
      { name: "Opening…" },
    );
    expect(opening).toBeDisabled();
    expect(
      screen.getByRole("button", { name: "Collection button", hidden: true }),
    ).toBeInTheDocument();
    expect(actionsSent()).toEqual(["resume"]);

    act(() => openForm());

    await waitFor(() => expect(actionsSent()).toEqual(["resume", "opened"]));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(screen.getByLabelText(/Notes/)).toHaveValue("Kept at the lock");
    expect(screen.getByRole("button", { name: "Collection button" })).toBeInTheDocument();
  });

  it("brings the dialog back with its error when the record fails to load (008 US1-4)", async () => {
    const user = userEvent.setup();
    failWith = "The policy wasn't found.";
    renderPending();
    await user.click(within(await dialog()).getByRole("button", { name: "Resume editing" }));

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(
      "The form for Collector Floater (edit) couldn't be opened. Your changes are still kept. Try again, close the database, or discard them.",
    );
    expect(alert).toHaveClass("hd-field__error");
    expect(actionsSent()).toEqual(["resume", "notOpened"]);
    const back = screen.getByRole("dialog", { name: dialogName });
    expect(within(back).getByRole("button", { name: "Resume editing" })).toBeEnabled();
    expect(within(back).queryByRole("button", { name: "Opening…" })).not.toBeInTheDocument();
    expect(within(back).getByRole("button", { name: "Discard changes" })).toBeInTheDocument();
    // The collection can't be used meanwhile: it stays beneath, out of reach.
    expect(screen.queryByRole("button", { name: "Collection button" })).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Collection button", hidden: true }),
    ).toBeInTheDocument();
  });

  it("offers Close the database with the other two in that dialog (008 US4, T095)", async () => {
    const user = userEvent.setup();
    failWith = "The policy wasn't found.";
    renderPending();
    await user.click(within(await dialog()).getByRole("button", { name: "Resume editing" }));
    await screen.findByRole("alert");

    const back = screen.getByRole("dialog", { name: dialogName });
    expect(
      within(back)
        .getAllByRole("button")
        .map((b) => b.textContent),
    ).toEqual(["Discard changes", "Close the database", "Resume editing"]);
  });

  it("offers Resume editing again after a failure, and sends resume again", async () => {
    const user = userEvent.setup();
    failWith = "The policy wasn't found.";
    renderPending();
    await user.click(within(await dialog()).getByRole("button", { name: "Resume editing" }));
    await screen.findByRole("alert");

    failWith = null;
    formAllowed = true;
    await user.click(
      within(screen.getByRole("dialog", { name: dialogName })).getByRole("button", {
        name: "Resume editing",
      }),
    );

    // A retry mounts the collection afresh, so its pages load again: the page
    // now opens the form as it mounts.
    await waitFor(() => expect(actionsSent()).toEqual(["resume", "notOpened", "resume", "opened"]));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("mounts the collection afresh when the changes are discarded after a failed resume", async () => {
    // The failed resume's collection was mounted while the backend refused
    // its reads (the changes go back to waiting as it mounts), so it may hold
    // nothing and no lists: found by the screenshot walk on Windows, where the
    // Add firearm form offered no types after "Discard changes".
    const user = userEvent.setup();
    failWith = "The policy wasn't found.";
    renderPending();
    await user.click(within(await dialog()).getByRole("button", { name: "Resume editing" }));
    await screen.findByRole("alert");
    expect(mounts).toBe(1);

    failWith = null;
    await user.click(
      within(screen.getByRole("dialog", { name: dialogName })).getByRole("button", {
        name: "Discard changes",
      }),
    );
    await user.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", {
        name: "Discard changes",
      }),
    );

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(actionsSent()).toEqual(["resume", "notOpened", "discard"]);
    expect(mounts).toBe(2);
    expect(screen.getByRole("button", { name: "Collection button" })).toBeInTheDocument();
  });

  it("keeps the changes for the next open when the database locks while opening (008 US1-3)", async () => {
    const user = userEvent.setup();
    renderPending();
    await user.click(within(await dialog()).getByRole("button", { name: "Resume editing" }));
    await screen.findByRole("button", { name: "Opening…" });

    // Lock now: the form never opened.
    await user.keyboard("{Control>}l{/Control}");
    expect(sessionService.lockDatabase).toHaveBeenCalledTimes(1);
    act(() => sessionClosed({ sessionId: 1, reason: "lockedByUser", databasePath: PATH }));

    // Database B opens, and its form for the same record is not filled.
    formAllowed = true;
    vi.mocked(databasesService.openDatabase).mockResolvedValue({
      ...status(false),
      sessionId: 2,
      path: SHARED,
      name: "Shared collection",
    });
    await user.type(
      await screen.findByLabelText("Passphrase for “Main collection”"),
      `${PASSPHRASE}{Enter}`,
    );

    expect(await screen.findByLabelText(/Notes/)).toHaveValue("");
    // Nothing said the form opened, and nothing was discarded or sent back:
    // the backend keeps the row for the next open of A.
    expect(actionsSent()).toEqual(["resume"]);
  });
});

// --- 009 US3: an upgrade installs its status as an open does (009 FR-011) -----------

describe("SessionProvider (009 US3: upgrading)", () => {
  const UPGRADED = {
    version: "0.2.0",
    backupPath: "/backups/Main before 0.2.0",
    atBackupLocation: true,
  };

  function Probe() {
    const session = useSession();
    const scope = useSessionScope();
    return (
      <>
        <p>
          Collection shell, session {scope.id}, upgraded for{" "}
          {session.status?.notes.upgraded?.version ?? "nothing"}
        </p>
        <button type="button" onClick={() => void session.dismissNote("upgraded")}>
          Dismiss the upgraded note
        </button>
        <button
          type="button"
          onClick={() => void session.restoreBackup("/backup", "pass", undefined, true)}
        >
          Restore and upgrade
        </button>
      </>
    );
  }

  const upgradedStatus = (sessionId: number): DatabaseStatus => {
    const opened = status(false);
    return { ...opened, sessionId, notes: { ...opened.notes, upgraded: UPGRADED } };
  };

  beforeEach(() => {
    vi.mocked(sessionService.getDatabaseStatus).mockReset().mockRejectedValue(closed);
    vi.mocked(sessionService.dismissNote).mockReset().mockResolvedValue(undefined);
    vi.mocked(databasesService.getChooserState).mockReset().mockResolvedValue(chooser);
    vi.mocked(databasesService.openDatabase)
      .mockReset()
      .mockRejectedValue(
        new CommandFailure({
          code: "UPGRADE_NEEDED",
          message: "The database needs to be upgraded.",
          details: { awaitingId: 7, version: "0.2.0", backupFolder: "/backups" },
        }),
      );
    vi.mocked(databasesService.upgradeDatabase).mockReset().mockResolvedValue(upgradedStatus(4));
    vi.mocked(databasesService.restoreBackup).mockReset().mockResolvedValue(upgradedStatus(5));
  });

  async function askedToUpgrade(user: ReturnType<typeof userEvent.setup>) {
    render(
      <SessionProvider>
        <Probe />
      </SessionProvider>,
    );
    await user.type(
      await screen.findByLabelText("Passphrase for “Main collection”"),
      `${PASSPHRASE}{Enter}`,
    );
    return screen.findByRole("alertdialog", {
      name: "Upgrade “Main collection” for HoploDex 0.2.0?",
    });
  }

  it("opens the database it upgraded, in a new scope of the status's session", async () => {
    const user = userEvent.setup();
    await askedToUpgrade(user);

    await user.click(screen.getByRole("button", { name: "Upgrade" }));

    expect(
      await screen.findByText(/Collection shell, session 4, upgraded for 0\.2\.0/),
    ).toBeInTheDocument();
    expect(databasesService.upgradeDatabase).toHaveBeenCalledExactlyOnceWith(7);
    expect(screen.queryByLabelText("Passphrase for “Main collection”")).not.toBeInTheDocument();
  });

  it("leaves the chooser showing when the upgrade is refused", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.upgradeDatabase).mockRejectedValue(
      new CommandFailure({ code: "UPGRADE_FAILED", message: "x", details: { backupPath: "/b" } }),
    );
    await askedToUpgrade(user);

    await user.click(screen.getByRole("button", { name: "Upgrade" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "HoploDex couldn't upgrade “Main collection”.",
    );
    expect(screen.queryByText(/Collection shell/)).not.toBeInTheDocument();
  });

  it("dismisses the upgraded note for the session, as the others", async () => {
    const user = userEvent.setup();
    await askedToUpgrade(user);
    await user.click(screen.getByRole("button", { name: "Upgrade" }));
    await screen.findByText(/upgraded for 0\.2\.0/);

    await user.click(screen.getByRole("button", { name: "Dismiss the upgraded note" }));

    expect(sessionService.dismissNote).toHaveBeenCalledExactlyOnceWith(
      expect.objectContaining({ id: 4 }),
      "upgraded",
    );
    expect(await screen.findByText(/upgraded for nothing/)).toBeInTheDocument();
  });

  it("restores with the upgrade, in the new session", async () => {
    const user = userEvent.setup();
    await askedToUpgrade(user);
    await user.click(screen.getByRole("button", { name: "Upgrade" }));
    await screen.findByText(/session 4/);

    await user.click(screen.getByRole("button", { name: "Restore and upgrade" }));

    expect(await screen.findByText(/session 5/)).toBeInTheDocument();
    expect(databasesService.restoreBackup).toHaveBeenCalledExactlyOnceWith(
      expect.objectContaining({ id: 4 }),
      "/backup",
      "pass",
      undefined,
      true,
    );
  });
});

// --- 010: About HoploDex (010 FR-001, contracts/ui.md §1, §4) --------------------

describe("SessionProvider and About HoploDex (010 FR-001)", () => {
  let sessionClosing: (closing: SessionClosing) => void;
  let sessionClosed: (closed: SessionClosed) => void;
  let aboutRequested: () => void;
  let session: SessionState;
  const unsubscribeAbout = vi.fn();
  const fetchNotices = vi.fn();

  /** The open collection's tree: the way into About from a session, and the
   * session itself for a test to call. */
  function Probe() {
    session = useSession();
    return (
      <>
        <button type="button" onClick={() => session.showAbout()}>
          Show About
        </button>
        <button type="button" onClick={() => void session.closeDatabase("closed")}>
          Close it
        </button>
        <button type="button" onClick={() => void session.closeDatabase("switched")}>
          Switch it
        </button>
        <DatabaseMenu />
        <p>Collection shell</p>
      </>
    );
  }

  /** The dialog's program block needs the Vite define of 010 contracts/notices.md
   * ("The Vite plugin"); the fetch never answers, so the dialog stays loading. */
  beforeEach(() => {
    vi.stubGlobal("__HOPLODEX_PROGRAM__", {
      name: "HoploDex",
      version: "0.1.0",
      copyrightYears: "2026",
      copyrightHolder: "exodious and the HoploDex contributors",
      sourceUrl: "https://github.com/exodious/HoploDex",
    });
    fetchNotices.mockReset().mockReturnValue(new Promise(() => {}));
    vi.stubGlobal("fetch", fetchNotices);
    unsubscribeAbout.mockReset();
    vi.mocked(sessionService.getDatabaseStatus).mockReset().mockRejectedValue(closed);
    vi.mocked(sessionService.closeDatabase)
      .mockReset()
      .mockResolvedValue({ backup: "notAttempted" });
    vi.mocked(sessionService.onSessionClosing)
      .mockReset()
      .mockImplementation((handler) => {
        sessionClosing = handler;
        return () => {};
      });
    vi.mocked(sessionService.onSessionClosed)
      .mockReset()
      .mockImplementation((handler) => {
        sessionClosed = handler;
        return () => {};
      });
    vi.mocked(sessionService.onQuitRequested)
      .mockReset()
      .mockReturnValue(() => {});
    vi.mocked(sessionService.onAboutRequested)
      .mockReset()
      .mockImplementation((handler) => {
        aboutRequested = handler;
        return unsubscribeAbout;
      });
    vi.mocked(databasesService.getChooserState).mockReset().mockResolvedValue(chooser);
    vi.mocked(databasesService.openDatabase).mockReset();
    vi.mocked(databasesService.listBackups).mockReset();
    vi.mocked(databasesService.onRestoreProgress)
      .mockReset()
      .mockReturnValue(() => {});
    vi.mocked(databasesService.onBackupsDeleteProgress)
      .mockReset()
      .mockReturnValue(() => {});
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  const aboutDialog = () => screen.queryByRole("dialog", { name: "About HoploDex" });
  const findAbout = () => screen.findByRole("dialog", { name: "About HoploDex" });

  function renderProvider() {
    return render(
      <SessionProvider>
        <Probe />
      </SessionProvider>,
    );
  }

  async function renderOpen() {
    vi.mocked(sessionService.getDatabaseStatus).mockResolvedValue(status(false));
    const rendered = renderProvider();
    await screen.findByText("Collection shell");
    return rendered;
  }

  async function renderChooser() {
    const rendered = renderProvider();
    await screen.findByLabelText("Passphrase for “Main collection”");
    return rendered;
  }

  it("opens the one dialog from the chooser's button, and a second call leaves it as it is", async () => {
    const user = userEvent.setup();
    await renderChooser();

    await user.click(screen.getByRole("button", { name: "About HoploDex" }));
    const dialog = await findAbout();
    act(() => aboutRequested());

    expect(screen.getAllByRole("dialog")).toEqual([dialog]);
    expect(fetchNotices).toHaveBeenCalledTimes(1);
  });

  it("opens the one dialog in the open phase, and a second call leaves it as it is", async () => {
    await renderOpen();

    act(() => session.showAbout());
    const dialog = await findAbout();
    act(() => session.showAbout());
    act(() => aboutRequested());

    expect(screen.getAllByRole("dialog")).toEqual([dialog]);
    expect(fetchNotices).toHaveBeenCalledTimes(1);
  });

  it("opens from the menu bar's event in the starting phase", async () => {
    vi.mocked(sessionService.getDatabaseStatus).mockReturnValue(new Promise(() => {}));
    renderProvider();

    expect(sessionService.onAboutRequested).toHaveBeenCalledTimes(1);
    act(() => aboutRequested());

    expect(await findAbout()).toBeInTheDocument();
  });

  it("stays open when the chooser appears after the starting phase, and closes on the next phase change (010 contracts/ui.md §1)", async () => {
    let finishStart: (error: unknown) => void = () => {};
    vi.mocked(sessionService.getDatabaseStatus).mockReturnValue(
      new Promise((_resolve, reject) => {
        finishStart = reject;
      }),
    );
    let finishOpen: (opened: DatabaseStatus) => void = () => {};
    vi.mocked(databasesService.openDatabase).mockReturnValue(
      new Promise((resolve) => {
        finishOpen = resolve;
      }),
    );
    renderProvider();
    act(() => aboutRequested());
    await findAbout();

    await act(async () => finishStart(closed));
    // The chooser is behind the dialog, which hides it from the accessibility tree.
    const field = await screen.findByLabelText("Passphrase for “Main collection”", {
      selector: "input",
    });
    expect(aboutDialog()).toBeInTheDocument();

    // The next change of phase (an open) closes it. The dialog makes the chooser
    // inert to the pointer, so the form is driven by events.
    fireEvent.change(field, { target: { value: PASSPHRASE } });
    fireEvent.submit(field.closest("form")!);
    await waitFor(() => expect(databasesService.openDatabase).toHaveBeenCalled());
    await act(async () => finishOpen(status(false)));

    expect(await screen.findByText("Collection shell")).toBeInTheDocument();
    expect(aboutDialog()).not.toBeInTheDocument();
  });

  it("opens from the menu bar's event in the chooser phase", async () => {
    await renderChooser();

    act(() => aboutRequested());

    expect(await findAbout()).toBeInTheDocument();
  });

  it("opens from the menu bar's event in the open phase", async () => {
    await renderOpen();

    act(() => aboutRequested());

    expect(await findAbout()).toBeInTheDocument();
  });

  it("listens for the event for the provider's whole life, across every phase", async () => {
    const user = userEvent.setup();
    vi.mocked(databasesService.openDatabase).mockResolvedValue(status(false));
    const { unmount } = await renderChooser();
    expect(sessionService.onAboutRequested).toHaveBeenCalledTimes(1);

    await user.type(
      screen.getByLabelText("Passphrase for “Main collection”"),
      `${PASSPHRASE}{Enter}`,
    );
    await screen.findByText("Collection shell");
    await user.click(screen.getByRole("button", { name: "Close it" }));
    await screen.findByLabelText("Passphrase for “Main collection”");

    expect(sessionService.onAboutRequested).toHaveBeenCalledTimes(1);
    expect(unsubscribeAbout).not.toHaveBeenCalled();
    unmount();
    expect(unsubscribeAbout).toHaveBeenCalledTimes(1);
  });

  it("closes, without asking, when a database opens", async () => {
    const user = userEvent.setup();
    let finishOpen: (opened: DatabaseStatus) => void = () => {};
    vi.mocked(databasesService.openDatabase).mockReturnValue(
      new Promise((resolve) => {
        finishOpen = resolve;
      }),
    );
    await renderChooser();
    await user.type(
      screen.getByLabelText("Passphrase for “Main collection”"),
      `${PASSPHRASE}{Enter}`,
    );
    await waitFor(() => expect(databasesService.openDatabase).toHaveBeenCalled());
    await user.click(screen.getByRole("button", { name: "About HoploDex" }));
    await findAbout();

    await act(async () => finishOpen(status(false)));

    expect(await screen.findByText("Collection shell")).toBeInTheDocument();
    expect(aboutDialog()).not.toBeInTheDocument();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });

  it("closes, without asking, on a lock, which shows the closing screen", async () => {
    await renderOpen();
    act(() => session.showAbout());
    await findAbout();

    act(() => sessionClosing({ sessionId: 1, reason: "lockedByUser" }));

    expect(
      await screen.findByRole("heading", { name: "Closing “Main collection”…" }),
    ).toBeInTheDocument();
    expect(aboutDialog()).not.toBeInTheDocument();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();

    act(() => sessionClosed({ sessionId: 1, reason: "lockedByUser", databasePath: PATH }));
    expect(await screen.findByLabelText("Passphrase for “Main collection”")).toBeInTheDocument();
    expect(aboutDialog()).not.toBeInTheDocument();
  });

  it("stays open for a closing event of another session", async () => {
    await renderOpen();
    act(() => session.showAbout());
    await findAbout();

    act(() => sessionClosing({ sessionId: 2, reason: "closed" }));

    expect(aboutDialog()).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: /^Closing/ })).not.toBeInTheDocument();
  });

  it("closes, without asking, when the database closes under it (a take-over)", async () => {
    await renderOpen();
    act(() => session.showAbout());
    await findAbout();

    act(() => sessionClosed({ sessionId: 1, reason: "takenOver", databasePath: PATH }));

    expect(await screen.findByLabelText("Passphrase for “Main collection”")).toBeInTheDocument();
    expect(aboutDialog()).not.toBeInTheDocument();
  });

  it.each(["closed", "switched"] as const)(
    "closes, without asking, on a database %s",
    async (reason) => {
      await renderOpen();
      act(() => session.showAbout());
      await findAbout();

      await act(async () => void (await session.closeDatabase(reason)));

      expect(sessionService.closeDatabase).toHaveBeenCalledWith(SCOPE, reason);
      expect(await screen.findByLabelText("Passphrase for “Main collection”")).toBeInTheDocument();
      expect(aboutDialog()).not.toBeInTheDocument();
      expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    },
  );

  it("does not stay closed: the next call opens it again, from the chooser", async () => {
    const user = userEvent.setup();
    await renderOpen();
    act(() => session.showAbout());
    await findAbout();
    await act(async () => void (await session.closeDatabase("closed")));
    await screen.findByLabelText("Passphrase for “Main collection”");

    await user.click(screen.getByRole("button", { name: "About HoploDex" }));

    expect(await findAbout()).toBeInTheDocument();
  });

  it("returns focus to the chooser's button when it closes", async () => {
    const user = userEvent.setup();
    await renderChooser();
    const button = screen.getByRole("button", { name: "About HoploDex" });
    await user.click(button);
    await findAbout();

    await user.keyboard("{Escape}");

    await waitFor(() => expect(aboutDialog()).not.toBeInTheDocument());
    expect(button).toHaveFocus();
  });

  it("returns focus to the control that opened it in the open phase", async () => {
    const user = userEvent.setup();
    await renderOpen();
    const button = screen.getByRole("button", { name: "Show About" });
    await user.click(button);
    await findAbout();

    await user.keyboard("{Escape}");

    await waitFor(() => expect(aboutDialog()).not.toBeInTheDocument());
    expect(button).toHaveFocus();
  });

  it("opens over another dialog from the menu bar, and returns focus there", async () => {
    const user = userEvent.setup();
    await renderOpen();
    await user.click(screen.getByRole("button", { name: "Database" }));
    await user.click(await screen.findByRole("menuitem", { name: "About databases and security" }));
    const guide = await screen.findByRole("dialog", { name: "About databases and security" });

    act(() => aboutRequested());
    const about = await findAbout();

    expect(screen.getAllByRole("dialog", { hidden: true })).toHaveLength(2);
    await user.keyboard("{Escape}");
    await waitFor(() => expect(about).not.toBeInTheDocument());
    expect(guide).toBeInTheDocument();
    expect(guide).toContainElement(document.activeElement as HTMLElement);
  });
});
