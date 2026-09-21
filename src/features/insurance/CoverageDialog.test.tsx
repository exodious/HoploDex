import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CollectionContext } from "../app/collectionStore";
import type { CollectionState } from "../app/collectionStore";
import type { Firearm } from "../firearms/types";
import { CoverageDialog } from "./CoverageDialog";
import type { BlanketSummary, InsurancePolicy } from "./types";

const policy = {
  id: 7,
  name: "Collectibles rider",
  insuranceCompany: "Acme",
  blanketCoverageLimit: null,
  effectiveEndDate: "2099-01-01",
  isExpired: false,
  isExpiringSoon: false,
  isInForce: true,
} as InsurancePolicy;

const blanket: BlanketSummary = {
  policyId: 9,
  policyName: "Homeowner's blanket",
  limit: 1_000_000,
  total: 400_000,
  firearmCount: 3,
  underInsured: false,
};

function collection(blanketInForce: BlanketSummary | null): CollectionState {
  return {
    firearms: [],
    firearmsById: new Map(),
    policies: [policy],
    policiesById: new Map([[policy.id, policy]]),
    summary: { collectionTotal: 0, blanket: blanketInForce, byPolicy: [], uninsured: [] },
  } as unknown as CollectionState;
}

const firearm = {
  id: 1,
  make: "Colt",
  model: "Python",
  estimatedValue: 3800,
  insurancePolicyId: null,
  scheduledCoverageAmount: null,
} as Firearm;

function renderDialog(
  onSave = vi.fn().mockResolvedValue(undefined),
  blanketInForce: BlanketSummary | null = blanket,
  subject: Firearm = firearm,
) {
  render(
    <CollectionContext.Provider value={collection(blanketInForce)}>
      <CoverageDialog open onOpenChange={vi.fn()} firearm={subject} onSave={onSave} />
    </CollectionContext.Provider>,
  );
  return onSave;
}

describe("CoverageDialog (FR-014, FR-036)", () => {
  it("requires a scheduled amount instead of saving $0", async () => {
    // Regression: a blank scheduled amount was saved as $0.
    const user = userEvent.setup();
    const onSave = renderDialog();

    await user.click(screen.getByRole("combobox", { name: "Policy" }));
    await user.click(await screen.findByRole("option", { name: /Collectibles rider/ }));
    await user.click(screen.getByRole("button", { name: "Save coverage" }));

    expect(screen.getByText("Enter the amount scheduled on the policy.")).toBeInTheDocument();
    expect(onSave).not.toHaveBeenCalled();

    await user.type(screen.getByLabelText("Scheduled amount"), "3,500");
    await user.click(screen.getByRole("button", { name: "Save coverage" }));
    expect(onSave).toHaveBeenCalledWith({ policyId: 7, scheduledCoverageAmount: 3500 });
  });

  it("has no per-firearm blanket option: choosing not to schedule is the blanket", async () => {
    const user = userEvent.setup();
    renderDialog();

    expect(screen.queryByRole("radio", { name: /Blanket/ })).not.toBeInTheDocument();
    // Unscheduled says where the firearm's coverage comes from.
    expect(screen.getByText(/covered by Homeowner's blanket/i)).toBeInTheDocument();
    expect(screen.queryByLabelText("Scheduled amount")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Save coverage" }));
  });

  it("unschedules a scheduled firearm with a null policy", async () => {
    const user = userEvent.setup();
    const scheduled = {
      ...firearm,
      insurancePolicyId: 7,
      scheduledCoverageAmount: 3000,
    } as Firearm;
    const onSave = renderDialog(undefined, blanket, scheduled);

    await user.click(screen.getByRole("combobox", { name: "Policy" }));
    await user.click(await screen.findByRole("option", { name: /Not scheduled/ }));
    await user.click(screen.getByRole("button", { name: "Save coverage" }));

    expect(onSave).toHaveBeenCalledWith({ policyId: null });
  });

  it("warns that an unscheduled firearm is uninsured when no blanket policy is in force", () => {
    renderDialog(undefined, null);

    expect(screen.getByText(/no blanket policy is in force/i)).toBeInTheDocument();
    expect(screen.getByText(/uninsured/i)).toBeInTheDocument();
  });
});
