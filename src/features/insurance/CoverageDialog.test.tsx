import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ACCESSORY_KINDS } from "../../test/collectionFixtures";
import type { Accessory } from "../accessories/types";
import { CollectionContext } from "../app/collectionStore";
import type { CollectionState } from "../app/collectionStore";
import type { Firearm } from "../firearms/types";
import { currentDraft, getDirtyForm } from "../session/usePendingDraft";
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
  accessoryCount: 0,
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

// specs/006-accessory-links FR-009, FR-027, contracts/ui-accessories.md §10:
// the dialog serves an accessory as it serves a firearm. Given one (in place
// of the firearm) it names it by `RecordName`, hands the same input to
// `onSave`, which the accessory's page sends as `assign_accessory_coverage`,
// and keeps its draft as `kind: "accessory"`.
describe("CoverageDialog for an accessory (FR-009)", () => {
  const accessory = {
    id: 3,
    accessoryKindId: 1,
    make: "Leupold",
    model: "VX-5HD 3-15x44",
    estimatedValue: 1000,
    insurancePolicyId: null,
    scheduledCoverageAmount: null,
  } as Accessory;
  const NAME = "Leupold VX-5HD 3-15x44 · Optic";

  function renderAccessoryDialog(
    onSave = vi.fn().mockResolvedValue(undefined),
    blanketInForce: BlanketSummary | null = blanket,
    subject: Accessory = accessory,
  ) {
    const state = {
      ...collection(blanketInForce),
      accessories: [],
      accessoriesById: new Map(),
      accessoryKinds: { kinds: ACCESSORY_KINDS },
      accessoryKindsFailed: false,
    } as unknown as CollectionState;
    render(
      <CollectionContext.Provider value={state}>
        <CoverageDialog open onOpenChange={vi.fn()} accessory={subject} onSave={onSave} />
      </CollectionContext.Provider>,
    );
    return onSave;
  }

  it("names the accessory, and speaks of it, not of a firearm", () => {
    renderAccessoryDialog();

    const dialog = screen.getByRole("dialog");
    expect(dialog).toHaveTextContent(NAME);
    expect(dialog).toHaveTextContent(/covered by Homeowner's blanket/i);
    expect(dialog).toHaveTextContent(/estimated replacement value is \$1,000/i);
    expect(dialog).not.toHaveTextContent(/firearm/i);
  });

  it("schedules the accessory on a policy with its own amount", async () => {
    const user = userEvent.setup();
    const onSave = renderAccessoryDialog();

    await user.click(screen.getByRole("combobox", { name: "Policy" }));
    await user.click(await screen.findByRole("option", { name: /Collectibles rider/ }));
    await user.click(screen.getByRole("button", { name: "Save coverage" }));
    expect(screen.getByText("Enter the amount scheduled on the policy.")).toBeInTheDocument();
    expect(onSave).not.toHaveBeenCalled();

    await user.type(screen.getByLabelText("Scheduled amount"), "900");
    await user.click(screen.getByRole("button", { name: "Save coverage" }));
    expect(onSave).toHaveBeenCalledWith({ policyId: 7, scheduledCoverageAmount: 900 });
  });

  it("unschedules a scheduled accessory with a null policy", async () => {
    const user = userEvent.setup();
    const onSave = renderAccessoryDialog(undefined, blanket, {
      ...accessory,
      insurancePolicyId: 7,
      scheduledCoverageAmount: 800,
    });

    await user.click(screen.getByRole("combobox", { name: "Policy" }));
    await user.click(await screen.findByRole("option", { name: /Not scheduled/ }));
    await user.click(screen.getByRole("button", { name: "Save coverage" }));

    expect(onSave).toHaveBeenCalledWith({ policyId: null });
  });

  it("warns that an unscheduled accessory is uninsured when no blanket policy is in force", () => {
    renderAccessoryDialog(undefined, null);

    expect(screen.getByText(/no blanket policy is in force/i)).toBeInTheDocument();
    expect(screen.getByText(/uninsured/i)).toBeInTheDocument();
  });

  it("keeps its unsaved input as a draft of the accessory (FR-027)", async () => {
    const user = userEvent.setup();
    renderAccessoryDialog();
    expect(getDirtyForm()).toBeNull();

    await user.click(screen.getByRole("combobox", { name: "Policy" }));
    await user.click(await screen.findByRole("option", { name: /Collectibles rider/ }));

    expect(getDirtyForm()?.label).toBe(`${NAME} (coverage)`);
    expect(currentDraft()).toMatchObject({
      formVersion: 1,
      kind: "accessory",
      mode: "coverage",
      targetId: 3,
      label: `${NAME} (coverage)`,
      values: { policyId: "7" },
    });
  });
});
