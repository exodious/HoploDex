import { describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import { CollectionContext } from "../app/collectionStore";
import type { CollectionState } from "../app/collectionStore";
import { SessionScopeContext, createSessionScope } from "../session/sessionScope";
import type { SessionScope } from "../session/sessionScope";
import { FORM_VERSION } from "./InsurancePolicyForm";
import { usePolicyEditors } from "./PolicyEditors";
import type { InsurancePolicy } from "./types";

// specs/008-hardening-batch tasks.md T032 (US1-4), research.md §6: when a
// resumed draft for a policy is waiting for its form and the policy isn't in
// the collection, the editors tell the scope (`scope.resumeFailed`), so the
// pending changes dialog comes back. They tell it for nothing else.

vi.mock("./insuranceService");

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

function collectionWith(policies: InsurancePolicy[], loaded = true): CollectionState {
  return {
    firearms: [],
    firearmsById: new Map(),
    accessories: [],
    accessoriesById: new Map(),
    summary: null,
    policies,
    policiesById: new Map(policies.map((p) => [p.id, p])),
    actionTypes: { actions: [], allowedByFirearmType: {} },
    actionTypesFailed: false,
    firearmTypes: { types: [] },
    firearmTypesFailed: false,
    accessoryKinds: { kinds: [] },
    accessoryKindsFailed: false,
    registrationClasses: { classes: [] },
    registrationClassesFailed: false,
    loaded,
    error: null,
    revision: 1,
    refresh: async () => {},
  };
}

const draft = (targetId: number | null, mode: "add" | "edit" = "edit") => ({
  formVersion: FORM_VERSION,
  kind: "policy" as const,
  mode,
  targetId,
  label: "Collector Floater (edit)",
  values: {},
});

/** A scope with `resumed` set (or not), and its two reports spied on. */
function scopeResuming(resumedDraft: ReturnType<typeof draft> | null) {
  const scope = createSessionScope(1);
  scope.resumed = resumedDraft ? { draft: resumedDraft, state: "opening" } : null;
  scope.resumeFailed = vi.fn().mockResolvedValue(undefined);
  scope.resumeOpened = vi.fn().mockResolvedValue(undefined);
  return scope;
}

function Editors() {
  const { dialogs } = usePolicyEditors();
  return <>{dialogs}</>;
}

function renderIn(scope: SessionScope, collection: CollectionState) {
  return render(
    <SessionScopeContext.Provider value={scope}>
      <CollectionContext.Provider value={collection}>
        <Editors />
      </CollectionContext.Provider>
    </SessionScopeContext.Provider>,
  );
}

describe("usePolicyEditors: a resumed draft whose form can't open (US1-4)", () => {
  it("reports notOpened when the policy isn't in the collection while its draft is opening", async () => {
    const scope = scopeResuming(draft(7));

    renderIn(scope, collectionWith([]));

    await act(async () => {});
    expect(scope.resumeFailed).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("reports nothing when no draft is resuming", async () => {
    const scope = scopeResuming(null);

    renderIn(scope, collectionWith([]));

    await act(async () => {});
    expect(scope.resumeFailed).not.toHaveBeenCalled();
  });

  it("reports nothing when the draft being resumed is for another kind of record", async () => {
    const scope = scopeResuming({ ...draft(7), kind: "firearm" } as never);

    renderIn(scope, collectionWith([]));

    await act(async () => {});
    expect(scope.resumeFailed).not.toHaveBeenCalled();
  });

  it("reports nothing while the collection is still loading, and opens the form once the policy arrives", async () => {
    const scope = scopeResuming(draft(7));
    const view = renderIn(scope, collectionWith([], false));
    await act(async () => {});
    expect(scope.resumeFailed).not.toHaveBeenCalled();

    view.rerender(
      <SessionScopeContext.Provider value={scope}>
        <CollectionContext.Provider value={collectionWith([policy])}>
          <Editors />
        </CollectionContext.Provider>
      </SessionScopeContext.Provider>,
    );

    expect(
      await screen.findByRole("dialog", { name: "Edit Collector Floater" }),
    ).toBeInTheDocument();
    expect(scope.resumeFailed).not.toHaveBeenCalled();
  });

  it("opens the form and reports nothing when the policy is in the collection", async () => {
    const scope = scopeResuming(draft(7));

    renderIn(scope, collectionWith([policy]));

    expect(
      await screen.findByRole("dialog", { name: "Edit Collector Floater" }),
    ).toBeInTheDocument();
    expect(scope.resumeFailed).not.toHaveBeenCalled();
  });

  it("opens the Add policy form for a draft of a new policy, which has no record to miss", async () => {
    const scope = scopeResuming(draft(null, "add"));

    renderIn(scope, collectionWith([]));

    expect(await screen.findByRole("dialog", { name: "Add policy" })).toBeInTheDocument();
    expect(scope.resumeFailed).not.toHaveBeenCalled();
  });
});
