import { describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { NavigationContext } from "../app/navigation";
import type { Navigation } from "../app/navigation";
import type { AccessorySummary } from "../accessories/types";
import type { FirearmSummary } from "../browse/types";
import { PolicyCard } from "./PolicyCard";
import type { BlanketSummary, InsurancePolicy, PolicySummary } from "./types";

const navigation = { open: vi.fn(), navigate: vi.fn() } as unknown as Navigation;

const base: InsurancePolicy = {
  id: 1,
  name: "Homeowner's blanket",
  policyNumber: "HB-1",
  insuranceCompany: "Acme",
  companyContact: null,
  agentName: null,
  agentContact: null,
  notes: null,
  blanketCoverageLimit: 10_000,
  effectiveStartDate: "2025-07-01",
  effectiveEndDate: "2099-06-30",
  createdAt: "",
  updatedAt: "",
  isInForce: true,
  isExpired: false,
  isExpiringSoon: false,
  expiringWarning: false,
  expiredWarning: false,
};

const blanketInForce: BlanketSummary = {
  policyId: 1,
  policyName: "Homeowner's blanket",
  limit: 10_000,
  total: 4_000,
  firearmCount: 3,
  accessoryCount: 2,
  underInsured: false,
};

// specs/006-accessory-links FR-009: an accessory is scheduled on a policy as
// a firearm is, and is listed with the scheduled firearms by `RecordName`.
function accessorySummary(id: number, overrides: Partial<AccessorySummary> = {}): AccessorySummary {
  return {
    id,
    accessoryKindId: 1,
    kindName: "Optic",
    genericThumbnailKey: "optic",
    make: "Leupold",
    model: "VX-5HD 3-15x44",
    serialNumber: null,
    caliber: null,
    cartridge: null,
    status: "active",
    thumbnailPhotoId: null,
    estimatedValue: 1_000,
    insuranceWarning: "none",
    insurancePolicyId: null,
    scheduledCoverageAmount: null,
    mountedOn: null,
    ...overrides,
  };
}

function firearmSummary(id: number, overrides: Partial<FirearmSummary> = {}): FirearmSummary {
  return {
    id,
    make: "Colt",
    model: `Model ${id}`,
    nickname: null,
    serialNumber: `SN${id}`,
    caliber: ".357",
    cartridge: null,
    firearmTypeName: "Handgun",
    actionTypeName: null,
    registeredAs: null,
    status: "active",
    thumbnailPhotoId: null,
    genericThumbnailKey: "handgun",
    estimatedValue: 2_000,
    insuranceWarning: "none",
    insurancePolicyId: null,
    scheduledCoverageAmount: null,
    ...overrides,
  };
}

function renderCard(
  policy: InsurancePolicy,
  options: {
    blanket?: BlanketSummary | null;
    summary?: PolicySummary;
    firearms?: FirearmSummary[];
    accessories?: AccessorySummary[];
  } = {},
) {
  render(
    <NavigationContext.Provider value={navigation}>
      <PolicyCard
        policy={policy}
        blanket={options.blanket ?? null}
        summary={options.summary}
        firearms={options.firearms ?? []}
        accessories={options.accessories ?? []}
        onEdit={() => {}}
        onDelete={() => {}}
      />
    </NavigationContext.Provider>,
  );
}

describe("PolicyCard (FR-015, FR-027, FR-028, FR-036)", () => {
  it("shows the blanket policy in force with its total against its limit", () => {
    renderCard(base, { blanket: blanketInForce });

    expect(screen.getByText("In force")).toBeInTheDocument();
    expect(screen.getByText("Blanket coverage")).toBeInTheDocument();
    expect(screen.getByText("$4,000")).toBeInTheDocument();
    expect(screen.getByText(/\$10,000/)).toBeInTheDocument();
    expect(screen.getByText(/3 firearms/)).toBeInTheDocument();
    expect(screen.getByText(/every firearm and accessory not scheduled individually/i)).toBeInTheDocument();
  });

  it("counts the accessories the blanket policy covers beside its firearms (FR-009)", () => {
    renderCard(base, { blanket: blanketInForce });

    expect(screen.getByText("3 firearms and 2 accessories")).toBeInTheDocument();
  });

  it("says 1 accessory, not 1 accessories", () => {
    renderCard(base, { blanket: { ...blanketInForce, firearmCount: 1, accessoryCount: 1 } });

    expect(screen.getByText("1 firearm and 1 accessory")).toBeInTheDocument();
  });

  it("lists scheduled accessories with the scheduled firearms, each by its name", async () => {
    const user = userEvent.setup();
    const firearm = firearmSummary(5, { insurancePolicyId: 1, scheduledCoverageAmount: 1_500 });
    const optic = accessorySummary(8, {
      insurancePolicyId: 1,
      scheduledCoverageAmount: 900,
      insuranceWarning: "under_insured",
    });
    const sling = accessorySummary(9, {
      accessoryKindId: 10,
      kindName: "Sling",
      genericThumbnailKey: "sling",
      make: null,
      model: null,
      estimatedValue: 100,
      insurancePolicyId: 1,
      scheduledCoverageAmount: 100,
    });
    renderCard(
      { ...base, blanketCoverageLimit: null },
      {
        firearms: [firearm],
        accessories: [optic, sling],
        summary: {
          policyId: 1,
          policyName: base.name,
          isExpired: false,
          isExpiringSoon: false,
          individuallyScheduled: [
            {
              record: { kind: "firearm", id: 5 },
              estimatedValue: 2_000,
              scheduledAmount: 1_500,
              underInsured: false,
            },
            {
              record: { kind: "accessory", id: 8 },
              estimatedValue: 1_000,
              scheduledAmount: 900,
              underInsured: true,
            },
            {
              record: { kind: "accessory", id: 9 },
              estimatedValue: 100,
              scheduledAmount: 100,
              underInsured: false,
            },
          ],
        },
      },
    );

    const table = screen.getByRole("table");
    const rows = within(table).getAllByRole("row").slice(1);
    expect(rows).toHaveLength(3);
    expect(rows[0]).toHaveTextContent("Colt Model 5");
    expect(rows[1]).toHaveTextContent("Leupold VX-5HD 3-15x44 · Optic");
    // The scheduled amount and the shortfall are keyed by the record, not
    // by an id that a firearm and an accessory might share.
    expect(rows[1]).toHaveTextContent("$900");
    expect(rows[1]).toHaveTextContent("$100 short");
    expect(rows[2]).toHaveTextContent(/^Sling/);

    await user.click(screen.getByRole("button", { name: "Leupold VX-5HD 3-15x44 · Optic" }));
    expect(navigation.open).toHaveBeenCalledWith({
      page: "accessory",
      id: 8,
      from: "insurance",
    });
  });

  it("does not mix up a firearm and an accessory that share an id", () => {
    const firearm = firearmSummary(5, { insurancePolicyId: 1, scheduledCoverageAmount: 1_500 });
    const optic = accessorySummary(5, { insurancePolicyId: 1, scheduledCoverageAmount: 900 });
    renderCard(
      { ...base, blanketCoverageLimit: null },
      {
        firearms: [firearm],
        accessories: [optic],
        summary: {
          policyId: 1,
          policyName: base.name,
          isExpired: false,
          isExpiringSoon: false,
          individuallyScheduled: [
            {
              record: { kind: "firearm", id: 5 },
              estimatedValue: 2_000,
              scheduledAmount: 1_500,
              underInsured: false,
            },
            {
              record: { kind: "accessory", id: 5 },
              estimatedValue: 1_000,
              scheduledAmount: 900,
              underInsured: false,
            },
          ],
        },
      },
    );

    const rows = within(screen.getByRole("table")).getAllByRole("row").slice(1);
    expect(rows[0]).toHaveTextContent("$1,500");
    expect(rows[0]).not.toHaveTextContent("$900");
    expect(rows[1]).toHaveTextContent("$900");
  });

  it("flags a blanket total over its limit", () => {
    renderCard(base, {
      blanket: { ...blanketInForce, total: 12_500, underInsured: true },
    });

    expect(screen.getByText("Over by $2,500")).toBeInTheDocument();
  });

  it("notes a blanket limit that is not in force rather than showing a meter", () => {
    renderCard({
      ...base,
      isInForce: false,
      effectiveStartDate: "2099-07-01",
      effectiveEndDate: "2100-06-30",
    });

    expect(screen.queryByText("Blanket coverage")).not.toBeInTheDocument();
    expect(screen.getByText(/not in force/i)).toBeInTheDocument();
  });

  it("has no blanket section for a schedule-only policy", () => {
    renderCard({ ...base, blanketCoverageLimit: null });

    expect(screen.queryByText("Blanket coverage")).not.toBeInTheDocument();
    expect(screen.queryByText(/not in force/i)).not.toBeInTheDocument();
  });

  it("lists the firearms scheduled under the policy with their amounts", () => {
    const scheduled = firearmSummary(5, {
      insurancePolicyId: 1,
      scheduledCoverageAmount: 1_500,
      insuranceWarning: "under_insured",
    });
    renderCard(
      { ...base, blanketCoverageLimit: null },
      {
        firearms: [scheduled],
        summary: {
          policyId: 1,
          policyName: base.name,
          isExpired: false,
          isExpiringSoon: false,
          individuallyScheduled: [
            {
              record: { kind: "firearm", id: 5 },
              estimatedValue: 2_000,
              scheduledAmount: 1_500,
              underInsured: true,
            },
          ],
        },
      },
    );

    expect(screen.getByText("Scheduled individually")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Colt Model 5" })).toBeInTheDocument();
    expect(screen.getByText("$500 short")).toBeInTheDocument();
  });

  it("warns when the policy is about to expire", () => {
    renderCard({
      ...base,
      isExpiringSoon: true,
      expiringWarning: true,
      effectiveEndDate: "2099-01-01",
    });

    expect(screen.getByText(/Expires in/)).toBeInTheDocument();
  });

  it("stays quiet about an expiry that FR-028 suppresses, since a successor takes over", () => {
    renderCard({ ...base, isExpiringSoon: true, expiringWarning: false });

    expect(screen.queryByText(/Expires in/)).not.toBeInTheDocument();
    expect(screen.queryByText(/expired/i)).not.toBeInTheDocument();
  });

  it("warns about an expired policy, and says its scheduled firearms are uninsured", () => {
    const scheduled = firearmSummary(5, { insurancePolicyId: 1, insuranceWarning: "uninsured" });
    renderCard(
      {
        ...base,
        blanketCoverageLimit: null,
        isInForce: false,
        isExpired: true,
        expiredWarning: true,
        effectiveEndDate: "2026-03-01",
      },
      { firearms: [scheduled] },
    );

    expect(screen.getByText("Expired Mar 1, 2026")).toBeInTheDocument();
    expect(
      screen.getByText(/scheduled on an expired policy counts as uninsured/i),
    ).toBeInTheDocument();
  });

  it("shows an old blanket policy as ended, without a warning, once it has been replaced", () => {
    renderCard({
      ...base,
      isInForce: false,
      isExpired: true,
      expiredWarning: false,
      effectiveEndDate: "2026-03-01",
    });

    expect(screen.getByText("Ended Mar 1, 2026")).toBeInTheDocument();
    expect(screen.queryByText(/Expired/)).not.toBeInTheDocument();
  });

  it("shows the notes, keeping line breaks, when the policy has any", () => {
    renderCard({ ...base, notes: "Renews in January.\nAsk about the rider." });

    expect(screen.getByText("Notes")).toBeInTheDocument();
    const notes = screen.getByText(/Renews in January\./);
    // textContent, not toHaveTextContent, which collapses the line break.
    expect(notes.textContent).toBe("Renews in January.\nAsk about the rider.");
  });

  it("shows no notes row when the policy has none", () => {
    renderCard(base);

    expect(screen.queryByText("Notes")).not.toBeInTheDocument();
  });
});
