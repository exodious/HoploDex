import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen, waitFor } from "@testing-library/react";
import { render } from "../../test/sessionScope";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import * as tauriClient from "../../services/tauriClient";
import type { Draft } from "../databases/types";
import { FORM_VERSION, InsurancePolicyForm } from "../insurance/InsurancePolicyForm";
import type { InsurancePolicy } from "../insurance/types";
import { SessionScopeContext, createSessionScope } from "./sessionScope";
import type { SessionScope } from "./sessionScope";
import { STAGE_DELAY_MS, currentDraft, flushPendingDraft, getDirtyForm } from "./usePendingDraft";

// Staging and resuming are the session scope's (specs/008-hardening-batch
// research.md §4, §6; tasks.md T030, T037): each form stages through the
// scope it registered under, a draft is matched only through the scope that
// resumed it, and the staging state resets when a scope ends. The backend is
// pinned at the IPC boundary (`invoke`), with the real scope and session
// service, so "sends nothing" means no command left the frontend.

vi.mock("../../services/tauriClient", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../services/tauriClient")>()),
  invoke: vi.fn(),
}));

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

function inScope(scope: SessionScope, ui: ReactNode) {
  return <SessionScopeContext.Provider value={scope}>{ui}</SessionScopeContext.Provider>;
}

function policyForm(scope: SessionScope) {
  return inScope(scope, <InsurancePolicyForm initialValues={policy} onSubmit={vi.fn()} />);
}

/** Every `stage_pending_changes` that left the frontend: the draft, and the
 * session it named. */
function stagedCalls() {
  return vi
    .mocked(tauriClient.invoke)
    .mock.calls.filter(([command]) => command === "stage_pending_changes")
    .map(([, args, options]) => ({
      draft: (args as { draft: Draft | null }).draft,
      session: options?.session,
    }));
}
const staged = () => stagedCalls().map((call) => call.draft);
const resolved = () =>
  vi
    .mocked(tauriClient.invoke)
    .mock.calls.filter(([command]) => command === "resolve_pending_changes")
    .map(([, args]) => (args as { action: string }).action);

beforeEach(() => {
  vi.mocked(tauriClient.invoke).mockReset().mockResolvedValue(undefined);
});

describe("usePendingDraft: staging (research.md §16)", () => {
  let scope: SessionScope;
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    scope = createSessionScope(1);
  });
  afterEach(() => vi.useRealTimers());

  function setup() {
    return userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
  }

  it("stages the draft 250 ms after the last edit", async () => {
    const user = setup();
    render(policyForm(scope));

    await user.type(screen.getByLabelText(/Notes/), "Rider");
    vi.advanceTimersByTime(STAGE_DELAY_MS - 50);
    expect(staged()).toEqual([]);
    vi.advanceTimersByTime(50);

    expect(staged()).toEqual([editDraft({ notes: "Rider" })]);
  });

  it("stages through the scope the form registered under", async () => {
    const user = setup();
    render(policyForm(createSessionScope(5)));

    await user.type(screen.getByLabelText(/Notes/), "Rider");
    vi.advanceTimersByTime(STAGE_DELAY_MS);

    expect(stagedCalls().map((call) => call.session)).toEqual([5]);
  });

  it("stages at once when a field loses focus", async () => {
    const user = setup();
    render(policyForm(scope));

    await user.type(screen.getByLabelText(/Notes/), "Rider");
    await user.tab();

    expect(staged()).toEqual([editDraft({ notes: "Rider" })]);
  });

  it("stages nothing for a form that stays clean", async () => {
    render(policyForm(scope));

    vi.advanceTimersByTime(1000);

    expect(staged()).toEqual([]);
    expect(currentDraft()).toBeNull();
  });

  it("stages null once the input is back as it was", async () => {
    const user = setup();
    render(policyForm(scope));
    await user.type(screen.getByLabelText(/Notes/), "R");
    vi.advanceTimersByTime(STAGE_DELAY_MS);

    await user.clear(screen.getByLabelText(/Notes/));
    vi.advanceTimersByTime(STAGE_DELAY_MS);

    expect(staged()).toEqual([editDraft({ notes: "R" }), null]);
  });

  it("stages null once a form with a staged draft is saved and closed", async () => {
    const user = setup();
    const { unmount } = render(policyForm(scope));
    await user.type(screen.getByLabelText(/Notes/), "Rider");
    vi.advanceTimersByTime(STAGE_DELAY_MS);

    unmount();

    expect(staged()).toEqual([editDraft({ notes: "Rider" }), null]);
  });

  it("gives the input exactly as it is now for lock now", async () => {
    const user = setup();
    render(policyForm(scope));

    await user.type(screen.getByLabelText(/Notes/), "Ri");

    expect(currentDraft()).toEqual(editDraft({ notes: "Ri" }));
  });
});

describe("usePendingDraft: the scope's end (US1, research.md §4)", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });
  afterEach(() => vi.useRealTimers());

  function setup() {
    return userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
  }

  it("sends nothing from a debounced flush that fires after its scope ended", async () => {
    const user = setup();
    const scope = createSessionScope(1);
    render(policyForm(scope));
    await user.type(screen.getByLabelText(/Notes/), "Rider");
    expect(staged()).toEqual([]);

    scope.end();
    vi.advanceTimersByTime(STAGE_DELAY_MS * 2);

    expect(staged()).toEqual([]);
  });

  it("sends nothing from a flush on blur after its scope ended", async () => {
    const user = setup();
    const scope = createSessionScope(1);
    render(policyForm(scope));
    await user.type(screen.getByLabelText(/Notes/), "Rider");

    scope.end();
    flushPendingDraft();
    await user.tab();

    expect(staged()).toEqual([]);
  });

  it("sends nothing when a form of the ended scope unmounts", async () => {
    const user = setup();
    const scope = createSessionScope(1);
    const { unmount } = render(policyForm(scope));
    await user.type(screen.getByLabelText(/Notes/), "Rider");
    vi.advanceTimersByTime(STAGE_DELAY_MS);
    expect(staged()).toHaveLength(1);

    scope.end();
    unmount();

    expect(staged()).toHaveLength(1);
  });

  it("resets what was staged when a scope ends: the next session's first draft is staged", async () => {
    const user = setup();
    const first = createSessionScope(1);
    render(policyForm(first));
    await user.type(screen.getByLabelText(/Notes/), "Rider");
    vi.advanceTimersByTime(STAGE_DELAY_MS);
    expect(stagedCalls()).toEqual([{ draft: editDraft({ notes: "Rider" }), session: 1 }]);

    // The first session ends with its form still mounted, as a draft the
    // backend held for it is gone with the session.
    first.end();
    const second = createSessionScope(2);
    render(policyForm(second));
    await user.type(screen.getAllByLabelText(/Notes/)[1], "Rider");
    vi.advanceTimersByTime(STAGE_DELAY_MS);

    expect(stagedCalls().at(-1)).toEqual({ draft: editDraft({ notes: "Rider" }), session: 2 });
  });
});

describe("usePendingDraft: resuming (FR-039, research.md §6)", () => {
  it("opens the form with the resumed changes as unsaved input, and reports it opened", async () => {
    const scope = createSessionScope(1);
    scope.resumed = {
      draft: editDraft({ notes: "Kept at a lock", agentName: "Dana" }),
      state: "opening",
    };

    render(policyForm(scope));

    expect(screen.getByLabelText(/Notes/)).toHaveValue("Kept at a lock");
    expect(screen.getByLabelText("Agent name")).toHaveValue("Dana");
    expect(getDirtyForm()?.label).toBe("Collector Floater (edit)");
    await waitFor(() => expect(resolved()).toEqual(["opened"]));
    expect(scope.resumed).toBeNull();
  });

  it("leaves changes for another record, or another form version, alone", async () => {
    const scope = createSessionScope(1);
    const other = { ...editDraft({ notes: "Elsewhere" }), targetId: 8 };
    scope.resumed = { draft: other, state: "opening" };
    render(policyForm(scope));
    expect(screen.getByLabelText(/Notes/)).toHaveValue("");
    expect(scope.resumed).toEqual({ draft: other, state: "opening" });

    const older = { ...editDraft({ notes: "Older" }), formVersion: FORM_VERSION + 1 };
    scope.resumed = { draft: older, state: "opening" };
    render(policyForm(scope));
    expect(screen.getAllByLabelText(/Notes/)[1]).toHaveValue("");

    await act(async () => {});
    expect(resolved()).toEqual([]);
  });

  it("doesn't match a draft resumed in one scope to a form of the same kind, mode, version and target in another", async () => {
    const resumedIn = createSessionScope(1);
    const draft = editDraft({ notes: "Kept in the first session" });
    resumedIn.resumed = { draft, state: "opening" };
    const other = createSessionScope(2);

    render(policyForm(other));

    expect(screen.getByLabelText(/Notes/)).toHaveValue("");
    expect(getDirtyForm()).toBeNull();
    await act(async () => {});
    expect(resolved()).toEqual([]);
    expect(resumedIn.resumed).toEqual({ draft, state: "opening" });
    expect(other.resumed).toBeNull();
  });

  it("opens only in the scope that resumed it when both forms are mounted", async () => {
    const resumedIn = createSessionScope(1);
    resumedIn.resumed = { draft: editDraft({ notes: "Mine" }), state: "opening" };
    const other = createSessionScope(2);

    render(
      <>
        {policyForm(other)}
        {policyForm(resumedIn)}
      </>,
    );

    const [notOurs, ours] = screen.getAllByLabelText(/Notes/);
    expect(notOurs).toHaveValue("");
    expect(ours).toHaveValue("Mine");
  });
});
