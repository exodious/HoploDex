import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CollectionContext } from "../app/collectionStore";
import type { CollectionState } from "../app/collectionStore";
import type { Firearm } from "../firearms/types";
import { CoverageDialog } from "./CoverageDialog";
import type { InsurancePolicy } from "./types";

const policy = {
  id: 7,
  name: "Collectibles rider",
  insuranceCompany: "Acme",
  blanketCoverageLimit: 500000,
  effectiveEndDate: "2099-01-01",
} as InsurancePolicy;

const collection = {
  firearms: [],
  firearmsById: new Map(),
  policies: [policy],
  policiesById: new Map([[policy.id, policy]]),
} as unknown as CollectionState;

const firearm = {
  id: 1,
  make: "Colt",
  model: "Python",
  estimatedValue: 380000,
  insurancePolicyId: null,
  coverageKind: null,
  scheduledCoverageAmount: null,
} as Firearm;

describe("CoverageDialog", () => {
  it("requires a scheduled amount instead of saving $0", async () => {
    // Regression: a blank scheduled amount was saved as $0.00.
    const user = userEvent.setup();
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(
      <CollectionContext.Provider value={collection}>
        <CoverageDialog open onOpenChange={vi.fn()} firearm={firearm} onSave={onSave} />
      </CollectionContext.Provider>,
    );

    await user.click(screen.getByRole("combobox", { name: "Policy" }));
    await user.click(await screen.findByRole("option", { name: /Collectibles rider/ }));
    await user.click(screen.getByRole("radio", { name: /Scheduled individually/ }));
    await user.click(screen.getByRole("button", { name: "Save coverage" }));

    expect(screen.getByText("Enter the amount scheduled on the policy.")).toBeInTheDocument();
    expect(onSave).not.toHaveBeenCalled();

    await user.type(screen.getByLabelText("Scheduled amount"), "3,500");
    await user.click(screen.getByRole("button", { name: "Save coverage" }));
    expect(onSave).toHaveBeenCalledWith({
      policyId: 7,
      coverageKind: "individually_scheduled",
      scheduledCoverageAmount: 350000,
    });
  });
});
