import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { formatDateTime } from "../../lib/dates";
import * as databasesService from "../databases/databasesService";
import type { DatabaseStatus, Draft, PendingSummary } from "../databases/types";
import { FORM_VERSION, InsurancePolicyForm } from "../insurance/InsurancePolicyForm";
import type { InsurancePolicy } from "../insurance/types";
import { SessionProvider } from "./SessionProvider";
import * as sessionService from "./sessionService";
import { peekResumedDraft, setResumedDraft } from "./usePendingDraft";

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

/** Stands in for the page the resumed form belongs to. */
function PolicyPageStandIn() {
  return peekResumedDraft() ? (
    <InsurancePolicyForm initialValues={policy} onSubmit={vi.fn()} />
  ) : null;
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
  });
  afterEach(() => setResumedDraft(null));

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

  it("resumes: the form opens with the kept changes as unsaved input", async () => {
    const user = userEvent.setup();
    renderOpen(pending);

    await user.click(within(await prompt()).getByRole("button", { name: "Resume editing" }));

    expect(sessionService.resolvePendingChanges).toHaveBeenCalledWith("resume");
    expect(await screen.findByText("Collection shell")).toBeInTheDocument();
    expect(screen.getByLabelText(/Notes/)).toHaveValue("Kept at the lock");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
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
      expect(sessionService.resolvePendingChanges).toHaveBeenCalledWith("discard"),
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

  it("offers only discarding for changes kept by another version of the form", async () => {
    renderOpen({ ...pending, formVersion: FORM_VERSION + 1 });

    const dialog = await prompt();
    expect(dialog).toHaveTextContent("They were kept by another version of HoploDex");
    expect(
      within(dialog).queryByRole("button", { name: "Resume editing" }),
    ).not.toBeInTheDocument();
  });

  it("shows the collection at once when nothing is pending", async () => {
    renderOpen(null);

    expect(await screen.findByText("Collection shell")).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});
