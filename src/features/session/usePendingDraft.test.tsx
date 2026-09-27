import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { Draft } from "../databases/types";
import { FORM_VERSION, InsurancePolicyForm } from "../insurance/InsurancePolicyForm";
import type { InsurancePolicy } from "../insurance/types";
import * as sessionService from "./sessionService";
import {
  STAGE_DELAY_MS,
  currentDraft,
  getDirtyForm,
  peekResumedDraft,
  setResumedDraft,
} from "./usePendingDraft";

vi.mock("./sessionService");

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

/** The policy form's state for `policy`, with `changes`. */
function valuesOf(changes: Record<string, string>) {
  return {
    name: "Collector Floater",
    policyNumber: "CF-100",
    insuranceCompany: "Acme Mutual",
    companyContact: "",
    agentName: "",
    agentContact: "",
    notes: "",
    blanketCoverageLimit: "",
    effectiveStartDate: "2026-01-01",
    effectiveEndDate: "2027-01-01",
    ...changes,
  };
}

function editDraft(changes: Record<string, string>): Draft {
  return {
    formVersion: FORM_VERSION,
    kind: "policy",
    mode: "edit",
    targetId: 7,
    label: "Collector Floater (edit)",
    values: valuesOf(changes),
  };
}

const staged = () => vi.mocked(sessionService.stagePendingChanges).mock.calls.map(([d]) => d);

describe("usePendingDraft: staging (research.md §16)", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.mocked(sessionService.stagePendingChanges).mockReset().mockResolvedValue(undefined);
  });
  afterEach(() => vi.useRealTimers());

  function setup() {
    return userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
  }

  it("stages the draft 250 ms after the last edit", async () => {
    const user = setup();
    render(<InsurancePolicyForm initialValues={policy} onSubmit={vi.fn()} />);

    await user.type(screen.getByLabelText(/Notes/), "Rider");
    vi.advanceTimersByTime(STAGE_DELAY_MS - 50);
    expect(staged()).toEqual([]);
    vi.advanceTimersByTime(50);

    expect(staged()).toEqual([editDraft({ notes: "Rider" })]);
  });

  it("stages at once when a field loses focus", async () => {
    const user = setup();
    render(<InsurancePolicyForm initialValues={policy} onSubmit={vi.fn()} />);

    await user.type(screen.getByLabelText(/Notes/), "Rider");
    await user.tab();

    expect(staged()).toEqual([editDraft({ notes: "Rider" })]);
  });

  it("stages nothing for a form that stays clean", async () => {
    render(<InsurancePolicyForm initialValues={policy} onSubmit={vi.fn()} />);

    vi.advanceTimersByTime(1000);

    expect(staged()).toEqual([]);
    expect(currentDraft()).toBeNull();
  });

  it("stages null once the input is back as it was", async () => {
    const user = setup();
    render(<InsurancePolicyForm initialValues={policy} onSubmit={vi.fn()} />);
    await user.type(screen.getByLabelText(/Notes/), "R");
    vi.advanceTimersByTime(STAGE_DELAY_MS);

    await user.clear(screen.getByLabelText(/Notes/));
    vi.advanceTimersByTime(STAGE_DELAY_MS);

    expect(staged()).toEqual([editDraft({ notes: "R" }), null]);
  });

  it("stages null once a form with a staged draft is saved and closed", async () => {
    const user = setup();
    const { unmount } = render(<InsurancePolicyForm initialValues={policy} onSubmit={vi.fn()} />);
    await user.type(screen.getByLabelText(/Notes/), "Rider");
    vi.advanceTimersByTime(STAGE_DELAY_MS);

    unmount();

    expect(staged()).toEqual([editDraft({ notes: "Rider" }), null]);
  });

  it("gives the input exactly as it is now for lock now", async () => {
    const user = setup();
    render(<InsurancePolicyForm initialValues={policy} onSubmit={vi.fn()} />);

    await user.type(screen.getByLabelText(/Notes/), "Ri");

    expect(currentDraft()).toEqual(editDraft({ notes: "Ri" }));
  });
});

describe("usePendingDraft: resuming (FR-039)", () => {
  beforeEach(() => {
    vi.mocked(sessionService.stagePendingChanges).mockReset().mockResolvedValue(undefined);
  });
  afterEach(() => setResumedDraft(null));

  it("opens the form with the resumed changes as unsaved input", () => {
    setResumedDraft(editDraft({ notes: "Kept at a lock", agentName: "Dana" }));

    render(<InsurancePolicyForm initialValues={policy} onSubmit={vi.fn()} />);

    expect(screen.getByLabelText(/Notes/)).toHaveValue("Kept at a lock");
    expect(screen.getByLabelText("Agent name")).toHaveValue("Dana");
    expect(getDirtyForm()?.label).toBe("Collector Floater (edit)");
    expect(peekResumedDraft()).toBeNull();
  });

  it("leaves changes for another record, or another form version, alone", () => {
    const other = { ...editDraft({ notes: "Elsewhere" }), targetId: 8 };
    setResumedDraft(other);
    render(<InsurancePolicyForm initialValues={policy} onSubmit={vi.fn()} />);
    expect(screen.getByLabelText(/Notes/)).toHaveValue("");
    expect(peekResumedDraft()).toEqual(other);

    const older = { ...editDraft({ notes: "Older" }), formVersion: FORM_VERSION + 1 };
    setResumedDraft(older);
    render(<InsurancePolicyForm initialValues={policy} onSubmit={vi.fn()} />);
    expect(screen.getAllByLabelText(/Notes/)[1]).toHaveValue("");
  });
});
