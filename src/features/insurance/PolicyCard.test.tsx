import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { NavigationContext } from "../app/navigation";
import type { Navigation } from "../app/navigation";
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
  underInsured: false,
};

function firearmSummary(id: number, overrides: Partial<FirearmSummary> = {}): FirearmSummary {
  return {
    id,
    make: "Colt",
    model: `Model ${id}`,
    nickname: null,
    serialNumber: `SN${id}`,
    caliber: ".357",
    firearmTypeName: "Handgun",
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
  } = {},
) {
  render(
    <NavigationContext.Provider value={navigation}>
      <PolicyCard
        policy={policy}
        blanket={options.blanket ?? null}
        summary={options.summary}
        firearms={options.firearms ?? []}
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
    expect(screen.getByText(/every firearm not scheduled/i)).toBeInTheDocument();
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
            { firearmId: 5, estimatedValue: 2_000, scheduledAmount: 1_500, underInsured: true },
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
