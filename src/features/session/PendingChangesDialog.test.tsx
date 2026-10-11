import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import { useState } from "react";
import userEvent from "@testing-library/user-event";
import { formatDateTime } from "../../lib/dates";
import * as databasesService from "../databases/databasesService";
import type { DatabaseStatus, Draft, PendingSummary } from "../databases/types";
import { FORM_VERSION as ACCESSORY_FORM_VERSION } from "../accessories/AccessoryForm";
import { FORM_VERSION as COVERAGE_FORM_VERSION } from "../insurance/CoverageDialog";
import { FORM_VERSION as DISPOSE_FORM_VERSION } from "../firearms/DisposeDialog";
import { FORM_VERSION as RESTORE_FORM_VERSION } from "../firearms/RestoreDialog";
import { FORM_VERSION, InsurancePolicyForm } from "../insurance/InsurancePolicyForm";
import type { InsurancePolicy } from "../insurance/types";
import { SessionProvider } from "./SessionProvider";
import { useSessionScope } from "./sessionScope";
import type { SessionScope } from "./sessionScope";
import * as sessionService from "./sessionService";

/** What the session scope of the open (session id 1) looks like to a mocked service. */
const SCOPE = expect.objectContaining({ id: 1 });
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("../databases/databasesService");
vi.mock("./sessionService");

const SAVED_AT = "2026-09-25T12:30:05Z";

const policy = {
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
} satisfies InsurancePolicy;

const pending: PendingSummary = {
  formVersion: FORM_VERSION,
  kind: "policy",
  mode: "edit",
  targetId: 7,
  label: "Collector Floater (edit)",
  savedAt: SAVED_AT,
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

function status(pendingChanges: PendingSummary | null): DatabaseStatus {
  return {
    sessionId: 1,
    path: "/home/sam/Documents/HoploDex/Main collection.hoplodex",
    name: "Main collection",
    passphraseSaved: false,
    keyringAvailable: false,
    screenLockSupported: false,
    settings: {
      backups: {
        enabled: true,
        keepCount: 5,
        location: { kind: "default", path: "/home/sam/Documents/HoploDex", available: true },
      },
      lock: { idleEnabled: true, idleMinutes: 10, onScreenLock: false },
    },
    pendingChanges,
    notes: {
      diskEncryption: false,
      openedBackup: null,
      restoredWithPassphraseOf: null,
      damagedFileKeptAt: null,
    },
  };
}

/** The scope of the open, as the collection's tree sees it. */
let openScope: SessionScope | null;

/** Stands in for the page the resumed form belongs to: it opens the form when
 * there are resumed changes to open it with. */
function PolicyPageStandIn() {
  const scope = useSessionScope();
  openScope = scope;
  const [resumed] = useState(() => scope.resumed);
  return resumed ? <InsurancePolicyForm initialValues={policy} onSubmit={vi.fn()} /> : null;
}

function renderOpen(pendingChanges: PendingSummary | null) {
  vi.mocked(sessionService.getDatabaseStatus).mockResolvedValue(status(pendingChanges));
  render(
    <SessionProvider>
      <p>Collection shell</p>
      <PolicyPageStandIn />
    </SessionProvider>,
  );
}

const prompt = () =>
  screen.findByRole("dialog", { name: "Unsaved changes to Collector Floater (edit)" });

describe("PendingChangesDialog (FR-039, contracts/ui-databases.md §13)", () => {
  beforeEach(() => {
    vi.mocked(sessionService.getDatabaseStatus).mockReset();
    vi.mocked(sessionService.resolvePendingChanges).mockReset().mockResolvedValue({ draft });
    vi.mocked(databasesService.getChooserState).mockReset();
    openScope = null;
  });

  it("asks before the collection can be used, saying what was kept", async () => {
    renderOpen(pending);

    const dialog = await prompt();
    expect(dialog).toHaveTextContent(
      `The database locked on ${formatDateTime(SAVED_AT)} while you were editing Collector Floater (edit). Your changes were kept.`,
    );
    expect(within(dialog).getByRole("button", { name: "Resume editing" })).toHaveFocus();
    expect(screen.queryByText("Collection shell")).not.toBeInTheDocument();
  });

  it("can't be dismissed", async () => {
    const user = userEvent.setup();
    renderOpen(pending);
    const dialog = await prompt();

    await user.keyboard("{Escape}");

    expect(dialog).toBeInTheDocument();
    expect(within(dialog).queryByRole("button", { name: "Close" })).not.toBeInTheDocument();
  });

  it("resumes: the form opens with the kept changes as unsaved input, then the dialog goes", async () => {
    const user = userEvent.setup();
    renderOpen(pending);

    await user.click(within(await prompt()).getByRole("button", { name: "Resume editing" }));

    expect(sessionService.resolvePendingChanges).toHaveBeenCalledWith(SCOPE, "resume");
    expect(await screen.findByText("Collection shell")).toBeInTheDocument();
    await waitFor(() =>
      expect(sessionService.resolvePendingChanges).toHaveBeenLastCalledWith(SCOPE, "opened"),
    );
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(screen.getByLabelText(/Notes/)).toHaveValue("Kept at the lock");
    expect(openScope?.resumed).toBeNull();
  });

  it("asks before discarding, then discards", async () => {
    const user = userEvent.setup();
    vi.mocked(sessionService.resolvePendingChanges).mockResolvedValue({});
    renderOpen(pending);

    await user.click(within(await prompt()).getByRole("button", { name: "Discard changes" }));
    const confirm = await screen.findByRole("alertdialog", {
      name: "Discard the changes to Collector Floater (edit)?",
    });
    expect(sessionService.resolvePendingChanges).not.toHaveBeenCalled();
    await user.click(within(confirm).getByRole("button", { name: "Discard changes" }));

    await waitFor(() =>
      expect(sessionService.resolvePendingChanges).toHaveBeenCalledWith(SCOPE, "discard"),
    );
    expect(await screen.findByText("Collection shell")).toBeInTheDocument();
    expect(screen.queryByLabelText(/Notes/)).not.toBeInTheDocument();
  });

  it("goes back to the question when discarding is cancelled", async () => {
    const user = userEvent.setup();
    renderOpen(pending);

    await user.click(within(await prompt()).getByRole("button", { name: "Discard changes" }));
    await user.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Cancel" }),
    );

    expect(await prompt()).toBeInTheDocument();
    expect(sessionService.resolvePendingChanges).not.toHaveBeenCalled();
  });

  it("offers only discarding for a record that no longer exists", async () => {
    renderOpen({ ...pending, resumable: false });

    const dialog = await prompt();
    expect(dialog).toHaveTextContent(
      "Collector Floater (edit) no longer exists, so these changes can only be discarded.",
    );
    expect(
      within(dialog).queryByRole("button", { name: "Resume editing" }),
    ).not.toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Discard changes" })).toBeInTheDocument();
  });

  describe("changes of a form this build doesn't know (US4, contracts/ui.md §1)", () => {
    const unknownForm = { ...pending, formVersion: FORM_VERSION + 1 };
    const resumeButton = (dialog: HTMLElement) =>
      within(dialog).queryByRole("button", { name: "Resume editing" });

    it("names the version that kept them when it is recorded and isn't this build's", async () => {
      renderOpen({ ...unknownForm, savedByVersion: "1.2.0", appVersion: "1.3.0" });

      const dialog = await prompt();
      expect(dialog).toHaveTextContent(
        "They were kept by HoploDex 1.2.0. Open the database with that version to resume them, or discard them here.",
      );
      expect(resumeButton(dialog)).not.toBeInTheDocument();
    });

    it("says a different build when the version number is this build's", async () => {
      renderOpen({ ...unknownForm, savedByVersion: "1.3.0", appVersion: "1.3.0" });

      expect(await prompt()).toHaveTextContent(
        "They were kept by a different build of HoploDex 1.3.0. Open the database with that build to resume them, or discard them here.",
      );
    });

    it("says another version when none is recorded", async () => {
      renderOpen({ ...unknownForm, savedByVersion: null });

      const dialog = await prompt();
      expect(dialog).toHaveTextContent(
        "They were kept by another version of HoploDex. Open the database with that version to resume them, or discard them here.",
      );
      expect(dialog).not.toHaveTextContent("so they can only be discarded");
    });

    it("keeps the record-gone sentence, and no version sentence, when the record is gone", async () => {
      renderOpen({ ...unknownForm, resumable: false, savedByVersion: "1.2.0" });

      const dialog = await prompt();
      expect(dialog).toHaveTextContent(
        "Collector Floater (edit) no longer exists, so these changes can only be discarded.",
      );
      expect(dialog).not.toHaveTextContent("They were kept by");
    });

    it("still offers Resume editing for a form this build knows, whichever version kept it", async () => {
      renderOpen({ ...pending, savedByVersion: "1.2.0", appVersion: "1.3.0" });

      const dialog = await prompt();
      expect(resumeButton(dialog)).toBeInTheDocument();
      expect(dialog).not.toHaveTextContent("They were kept by");
    });
  });

  describe("Close the database (US4, contracts/ui.md §1)", () => {
    const cases: [string, PendingSummary, string[]][] = [
      ["a resumable change", pending, ["Discard changes", "Close the database", "Resume editing"]],
      [
        "changes of another version",
        { ...pending, formVersion: FORM_VERSION + 1, savedByVersion: "1.2.0" },
        ["Discard changes", "Close the database"],
      ],
      [
        "a record that is gone",
        { ...pending, resumable: false },
        ["Discard changes", "Close the database"],
      ],
    ];

    it.each(cases)(
      "is offered between Discard and Resume for %s",
      async (_name, summary, names) => {
        renderOpen(summary);

        const dialog = await prompt();
        expect(
          within(dialog)
            .getAllByRole("button")
            .map((button) => button.textContent),
        ).toEqual(names);
      },
    );

    it.each(cases)("closes the database, keeping the changes, for %s", async (_name, summary) => {
      const user = userEvent.setup();
      vi.mocked(sessionService.closeDatabase).mockReset().mockResolvedValue({ backup: "notDue" });
      vi.mocked(databasesService.getChooserState).mockResolvedValue({
        recent: [
          {
            path: status(null).path,
            name: "Main collection",
            lastOpenedAt: SAVED_AT,
            available: true,
            passphraseSaved: false,
            backups: null,
            changedSinceLeftAt: null,
          },
        ],
        selectedPath: status(null).path,
        keyringAvailable: false,
        screenLockSupported: false,
        suggested: { folder: "/home/sam/Documents/HoploDex", name: "My collection" },
        notices: [],
      });
      renderOpen(summary);

      await user.click(within(await prompt()).getByRole("button", { name: "Close the database" }));

      await waitFor(() =>
        expect(sessionService.closeDatabase).toHaveBeenCalledWith(SCOPE, "closed"),
      );
      expect(sessionService.resolvePendingChanges).not.toHaveBeenCalled();
      // The chooser, with this database selected, and the changes kept.
      expect(await screen.findByLabelText("Passphrase for “Main collection”")).toBeInTheDocument();
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("focuses Resume editing when it is offered", async () => {
      renderOpen(pending);

      const dialog = await prompt();
      expect(within(dialog).getByRole("button", { name: "Resume editing" })).toHaveFocus();
    });

    it.each(cases.slice(1))(
      "focuses Close the database, not Discard, for %s",
      async (_name, summary) => {
        renderOpen(summary);

        const dialog = await prompt();
        expect(within(dialog).getByRole("button", { name: "Close the database" })).toHaveFocus();
        expect(within(dialog).getByRole("button", { name: "Discard changes" })).not.toHaveFocus();
      },
    );

    it("stays available while Resume editing shows Opening…", async () => {
      const user = userEvent.setup();
      renderOpen(pending);

      const dialog = await prompt();
      await user.click(within(dialog).getByRole("button", { name: "Resume editing" }));
      await within(dialog).findByRole("button", { name: "Opening…" });

      expect(within(dialog).getByRole("button", { name: "Close the database" })).toBeEnabled();
    });
  });

  it("shows the collection at once when nothing is pending", async () => {
    renderOpen(null);

    expect(await screen.findByText("Collection shell")).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});

// specs/006-accessory-links FR-027: an accessory draft kept at a lock can be
// resumed in each of its five modes. The accessory's dispose, restore and
// coverage forms are the firearm's dialogs, so they carry those versions.
describe("PendingChangesDialog for an accessory draft (FR-027)", () => {
  const modes: [Draft["mode"], number, number | null][] = [
    ["add", ACCESSORY_FORM_VERSION, null],
    ["edit", ACCESSORY_FORM_VERSION, 4],
    ["dispose", DISPOSE_FORM_VERSION, 4],
    ["restore", RESTORE_FORM_VERSION, 4],
    ["coverage", COVERAGE_FORM_VERSION, 4],
  ];

  function accessoryPending(
    mode: Draft["mode"],
    formVersion: number,
    targetId: number | null,
  ): { summary: PendingSummary; kept: Draft } {
    const label = `Leupold VX (${mode})`;
    return {
      summary: {
        formVersion,
        kind: "accessory",
        mode,
        targetId,
        label,
        savedAt: SAVED_AT,
        resumable: true,
        savedByVersion: "1.3.0",
        appVersion: "1.3.0",
      },
      kept: { formVersion, kind: "accessory", mode, targetId, label, values: {} },
    };
  }

  beforeEach(() => {
    vi.mocked(sessionService.getDatabaseStatus).mockReset();
    vi.mocked(sessionService.resolvePendingChanges).mockReset();
    openScope = null;
  });

  it.each(modes)("resumes an accessory %s draft", async (mode, formVersion, targetId) => {
    const user = userEvent.setup();
    const { summary, kept } = accessoryPending(mode, formVersion, targetId);
    vi.mocked(sessionService.resolvePendingChanges).mockResolvedValue({ draft: kept });
    vi.mocked(sessionService.getDatabaseStatus).mockResolvedValue(status(summary));
    render(
      <SessionProvider>
        <p>Collection shell</p>
        <PolicyPageStandIn />
      </SessionProvider>,
    );

    const dialog = await screen.findByRole("dialog", {
      name: `Unsaved changes to ${summary.label}`,
    });
    await user.click(within(dialog).getByRole("button", { name: "Resume editing" }));

    expect(sessionService.resolvePendingChanges).toHaveBeenCalledWith(SCOPE, "resume");
    expect(await screen.findByText("Collection shell")).toBeInTheDocument();
    // No page of this test opens the form, so the dialog stays with the
    // changes waiting in the scope.
    expect(openScope?.resumed).toEqual({ draft: kept, state: "opening" });
    expect(within(dialog).getByRole("button", { name: "Opening…" })).toBeDisabled();
  });

  it.each(modes)(
    "offers only discarding for an accessory %s draft of another version",
    async (mode, formVersion, targetId) => {
      const { summary } = accessoryPending(mode, formVersion + 1, targetId);
      vi.mocked(sessionService.getDatabaseStatus).mockResolvedValue(status(summary));
      render(
        <SessionProvider>
          <p>Collection shell</p>
        </SessionProvider>,
      );

      const dialog = await screen.findByRole("dialog", {
        name: `Unsaved changes to ${summary.label}`,
      });
      expect(
        within(dialog).queryByRole("button", { name: "Resume editing" }),
      ).not.toBeInTheDocument();
    },
  );
});
