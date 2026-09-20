import { describe, expect, it } from "vitest";
import type { Firearm } from "../firearms/types";
import type { InsurancePolicy, PolicySummary, ValueSummary } from "./types";
import { coverageShortfall, coverageStatus, policyExpiry } from "./coverage";

const policy: InsurancePolicy = {
  id: 7,
  name: "Collectibles rider",
  policyNumber: "R-1",
  insuranceCompany: "Acme",
  companyContact: null,
  agentName: null,
  agentContact: null,
  blanketCoverageLimit: 100000,
  effectiveStartDate: "2025-01-01",
  effectiveEndDate: "2026-12-31",
  createdAt: "",
  updatedAt: "",
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
    coverageKind: null,
    scheduledCoverageAmount: null,
    createdAt: "",
    updatedAt: "",
    ...overrides,
  };
}

describe("policyExpiry", () => {
  it("mirrors the backend's 30-day expiring-soon window", () => {
    expect(policyExpiry("2026-01-31", "2026-01-01")).toMatchObject({
      expired: false,
      expiringSoon: true,
      daysLeft: 30,
    });
    expect(policyExpiry("2026-02-01", "2026-01-01").expiringSoon).toBe(false);
    expect(policyExpiry("2025-12-31", "2026-01-01")).toMatchObject({
      expired: true,
      expiringSoon: false,
    });
    expect(policyExpiry("2026-01-01", "2026-01-01").expired).toBe(false);
  });
});

describe("coverageStatus", () => {
  it("explains an uninsured firearm with no policy", () => {
    const status = coverageStatus(firearm(), "uninsured", undefined, "2026-01-01");
    expect(status).toMatchObject({
      tone: "warn",
      label: "Uninsured",
      detail: "No policy assigned.",
    });
  });

  it("blames an expired policy when one is assigned", () => {
    const status = coverageStatus(
      firearm({ insurancePolicyId: 7, coverageKind: "blanket" }),
      "uninsured",
      { ...policy, effectiveEndDate: "2025-12-01" },
      "2026-01-01",
    );
    expect(status.label).toBe("Uninsured");
    expect(status.detail).toContain("expired");
  });

  it("shows the shortfall for an under-scheduled firearm", () => {
    const status = coverageStatus(
      firearm({
        insurancePolicyId: 7,
        coverageKind: "individually_scheduled",
        scheduledCoverageAmount: 300000,
      }),
      "under_insured",
      policy,
      "2026-01-01",
    );
    expect(status).toMatchObject({ tone: "warn", label: "Under-insured" });
    expect(status.detail).toBe("Scheduled for $3,000 — $1,000 short of its value.");
  });

  it("confirms coverage without a warning", () => {
    const status = coverageStatus(
      firearm({ insurancePolicyId: 7, coverageKind: "blanket" }),
      "none",
      policy,
      "2026-01-01",
    );
    expect(status).toMatchObject({ tone: "ok", label: "Covered" });
    expect(status.detail).toBe("Blanket coverage on Collectibles rider.");
  });

  it("stays neutral when there is no value to check", () => {
    const status = coverageStatus(firearm({ estimatedValue: null }), "none", undefined);
    expect(status).toMatchObject({ tone: "neutral", label: "Not insured" });
  });

  it("does not track disposed firearms", () => {
    const status = coverageStatus(firearm({ status: "disposed" }), "none", undefined);
    expect(status).toMatchObject({ tone: "neutral", label: "Not tracked" });
  });
});

describe("coverageShortfall", () => {
  const policySummary = (overrides: Partial<PolicySummary>): PolicySummary => ({
    policyId: 1,
    policyName: "Rider",
    isExpired: false,
    isExpiringSoon: false,
    blanketTotal: 0,
    blanketLimit: 0,
    blanketUnderInsured: false,
    individuallyScheduled: [],
    ...overrides,
  });
  const summaryOf = (...byPolicy: PolicySummary[]): ValueSummary => ({
    collectionTotal: 0,
    byPolicy,
    unassigned: [],
  });

  it("is zero without a summary or when nothing is short", () => {
    expect(coverageShortfall(null)).toBe(0);
    expect(
      coverageShortfall(summaryOf(policySummary({ blanketTotal: 90, blanketLimit: 100 }))),
    ).toBe(0);
  });

  it("counts how far blanket firearms exceed the limit, not their value", () => {
    const summary = summaryOf(policySummary({ blanketTotal: 150000, blanketLimit: 100000 }));
    expect(coverageShortfall(summary)).toBe(50000);
  });

  it("counts how far each scheduled amount is below its firearm's value", () => {
    const summary = summaryOf(
      policySummary({
        individuallyScheduled: [
          { firearmId: 1, estimatedValue: 400000, scheduledAmount: 300000, underInsured: true },
          { firearmId: 2, estimatedValue: 200000, scheduledAmount: 250000, underInsured: false },
          { firearmId: 3, estimatedValue: 100000, scheduledAmount: 0, underInsured: true },
        ],
      }),
    );
    expect(coverageShortfall(summary)).toBe(200000);
  });

  it("leaves expired policies out, since their firearms count as uninsured", () => {
    const summary = summaryOf(
      policySummary({ isExpired: true, blanketTotal: 500000, blanketLimit: 1 }),
      policySummary({ policyId: 2, blanketTotal: 120000, blanketLimit: 100000 }),
    );
    expect(coverageShortfall(summary)).toBe(20000);
  });
});
