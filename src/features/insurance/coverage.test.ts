import { describe, expect, it } from "vitest";
import type { Firearm } from "../firearms/types";
import { coverageShortfall, coverageStatus, expiryLabel } from "./coverage";
import type { BlanketSummary, InsurancePolicy, PolicySummary, ValueSummary } from "./types";

const policy: InsurancePolicy = {
  id: 7,
  name: "Collectibles rider",
  policyNumber: "P-7",
  insuranceCompany: "Acme",
  companyContact: null,
  agentName: null,
  agentContact: null,
  blanketCoverageLimit: null,
  effectiveStartDate: "2025-01-01",
  effectiveEndDate: "2026-12-31",
  createdAt: "",
  updatedAt: "",
  isInForce: true,
  isExpired: false,
  isExpiringSoon: false,
  expiringWarning: false,
  expiredWarning: false,
};

const blanket: BlanketSummary = {
  policyId: 9,
  policyName: "Homeowner's blanket",
  limit: 1_000_000,
  total: 400_000,
  firearmCount: 3,
  underInsured: false,
};

function firearm(overrides: Partial<Firearm> = {}): Firearm {
  return {
    id: 1,
    make: "Colt",
    model: "Python",
    nickname: null,
    serialNumber: "V1",
    noSerialAttested: false,
    caliber: ".357 Magnum",
    firearmTypeId: 1,
    notes: null,
    accessories: null,
    status: "active",
    estimatedValue: 400000,
    acquisitionSource: null,
    acquisitionDate: null,
    acquisitionPrice: null,
    dispositionType: null,
    dispositionRecipient: null,
    dispositionDate: null,
    dispositionPrice: null,
    thumbnailPhotoId: null,
    insurancePolicyId: null,
    scheduledCoverageAmount: null,
    createdAt: "",
    updatedAt: "",
    ...overrides,
  };
}

describe("expiryLabel", () => {
  const label = (overrides: Partial<InsurancePolicy>, today = "2026-06-15") =>
    expiryLabel({ ...policy, ...overrides }, today);

  it("says when an expired policy ended", () => {
    expect(label({ isExpired: true, effectiveEndDate: "2026-03-01" })).toBe("Expired Mar 1, 2026");
  });

  it("counts down an expiring policy, using the backend's flag rather than its own window", () => {
    expect(label({ isExpiringSoon: true, effectiveEndDate: "2026-06-27" })).toBe(
      "Expires in 12 days",
    );
    expect(label({ isExpiringSoon: true, effectiveEndDate: "2026-06-16" })).toBe(
      "Expires in 1 day",
    );
    expect(label({ isExpiringSoon: true, effectiveEndDate: "2026-06-15" })).toBe("Expires today");
  });

  it("names the end date of a policy in force", () => {
    expect(label({ effectiveEndDate: "2026-12-31" })).toBe("In force until Dec 31, 2026");
  });

  it("says when a policy has yet to start", () => {
    expect(label({ isInForce: false, effectiveStartDate: "2027-01-01" })).toBe(
      "Starts Jan 1, 2027",
    );
  });
});

describe("coverageStatus", () => {
  it("explains an unscheduled firearm with no blanket policy in force as uninsured", () => {
    const status = coverageStatus(firearm(), "uninsured", undefined, null);
    expect(status).toMatchObject({ tone: "warn", label: "Uninsured" });
    expect(status.detail).toMatch(/no blanket policy/i);
  });

  it("blames an expired policy when the firearm is scheduled on one", () => {
    const expired = { ...policy, isExpired: true, effectiveEndDate: "2025-12-01" };
    const status = coverageStatus(
      firearm({ insurancePolicyId: 7, scheduledCoverageAmount: 500000 }),
      "uninsured",
      expired,
      blanket,
    );
    expect(status.label).toBe("Uninsured");
    expect(status.detail).toBe("Collectibles rider expired Dec 1, 2025.");
  });

  it("shows the shortfall for an under-scheduled firearm", () => {
    const status = coverageStatus(
      firearm({ insurancePolicyId: 7, scheduledCoverageAmount: 300000 }),
      "under_insured",
      policy,
      null,
    );
    expect(status).toMatchObject({ tone: "warn", label: "Under-insured" });
    expect(status.detail).toBe("Scheduled for $3,000 — $1,000 short of its value.");
  });

  it("blames the blanket limit when an unscheduled firearm is under-insured", () => {
    const status = coverageStatus(firearm(), "under_insured", undefined, {
      ...blanket,
      underInsured: true,
    });
    expect(status).toMatchObject({ tone: "warn", label: "Under-insured" });
    expect(status.detail).toContain("Homeowner's blanket");
    expect(status.detail).toMatch(/limit/);
  });

  it("confirms a scheduled firearm is covered", () => {
    const status = coverageStatus(
      firearm({ insurancePolicyId: 7, scheduledCoverageAmount: 400000 }),
      "none",
      policy,
      blanket,
    );
    expect(status).toMatchObject({ tone: "ok", label: "Covered" });
    expect(status.detail).toBe("Scheduled for $4,000 on Collectibles rider.");
  });

  it("confirms an unscheduled firearm is covered by the blanket policy in force", () => {
    const status = coverageStatus(firearm(), "none", undefined, blanket);
    expect(status).toMatchObject({ tone: "ok", label: "Covered" });
    expect(status.detail).toBe("Covered by Homeowner's blanket.");
  });

  it("stays neutral when there is no value to check", () => {
    const status = coverageStatus(firearm({ estimatedValue: null }), "none", undefined, blanket);
    expect(status.tone).toBe("neutral");
    expect(status.detail).toMatch(/estimated value/i);
  });

  it("does not track disposed firearms", () => {
    const status = coverageStatus(firearm({ status: "disposed" }), "none", undefined, blanket);
    expect(status).toMatchObject({ tone: "neutral", label: "Not tracked" });
  });
});

describe("coverageShortfall", () => {
  const policySummary = (overrides: Partial<PolicySummary>): PolicySummary => ({
    policyId: 1,
    policyName: "Rider",
    isExpired: false,
    isExpiringSoon: false,
    individuallyScheduled: [],
    ...overrides,
  });
  const summaryOf = (
    byPolicy: PolicySummary[],
    blanketSummary: BlanketSummary | null = null,
  ): ValueSummary => ({
    collectionTotal: 0,
    blanket: blanketSummary,
    byPolicy,
    uninsured: [],
  });

  it("is zero without a summary or when nothing is short", () => {
    expect(coverageShortfall(null)).toBe(0);
    expect(coverageShortfall(summaryOf([], { ...blanket, total: 90, limit: 100 }))).toBe(0);
  });

  it("counts how far the unscheduled firearms exceed the blanket limit, not their value", () => {
    const summary = summaryOf([], { ...blanket, total: 150000, limit: 100000 });
    expect(coverageShortfall(summary)).toBe(50000);
  });

  it("counts how far each scheduled amount is below its firearm's value", () => {
    const summary = summaryOf([
      policySummary({
        individuallyScheduled: [
          { firearmId: 1, estimatedValue: 400000, scheduledAmount: 300000, underInsured: true },
          { firearmId: 2, estimatedValue: 200000, scheduledAmount: 250000, underInsured: false },
          { firearmId: 3, estimatedValue: 100000, scheduledAmount: 0, underInsured: true },
        ],
      }),
    ]);
    expect(coverageShortfall(summary)).toBe(200000);
  });

  it("leaves expired policies out, since their firearms count as uninsured", () => {
    const summary = summaryOf(
      [
        policySummary({
          isExpired: true,
          individuallyScheduled: [
            { firearmId: 1, estimatedValue: 500000, scheduledAmount: 1, underInsured: true },
          ],
        }),
        policySummary({
          policyId: 2,
          individuallyScheduled: [
            { firearmId: 2, estimatedValue: 120000, scheduledAmount: 100000, underInsured: true },
          ],
        }),
      ],
      null,
    );
    expect(coverageShortfall(summary)).toBe(20000);
  });
});
