// Mirrors src-tauri/src/commands/insurance.rs, src-tauri/src/models/
// insurance_policy.rs and src-tauri/src/services/{insurance_status,
// valuation}.rs's wire shapes (camelCase, per contracts/tauri-commands.md),
// User Story 3.

export interface InsurancePolicy {
  id: number;
  name: string;
  policyNumber: string;
  insuranceCompany: string;
  companyContact: string | null;
  agentName: string | null;
  agentContact: string | null;
  /** Set for a blanket policy (FR-036); null for a schedule-only one. */
  blanketCoverageLimit: number | null;
  effectiveStartDate: string;
  effectiveEndDate: string;
  createdAt: string;
  updatedAt: string;
  /** Derived by the backend as of today. `isExpired`/`isExpiringSoon` are
   * facts about the dates; the `*Warning` flags are what to warn about,
   * which FR-028 suppresses for a blanket policy that has been renewed or
   * replaced. */
  isInForce: boolean;
  isExpired: boolean;
  isExpiringSoon: boolean;
  expiringWarning: boolean;
  expiredWarning: boolean;
}

/** What is sent to create or edit a policy: the stored fields only. */
export type InsurancePolicyInput = Omit<
  InsurancePolicy,
  | "id"
  | "createdAt"
  | "updatedAt"
  | "isInForce"
  | "isExpired"
  | "isExpiringSoon"
  | "expiringWarning"
  | "expiredWarning"
>;

/** `policyId: null` leaves the firearm unscheduled, i.e. covered by the
 * blanket policy in force (FR-036); a policy needs its own amount. */
export interface AssignCoverageInput {
  policyId: number | null;
  scheduledCoverageAmount?: number;
}

export interface IndividualCoverage {
  firearmId: number;
  estimatedValue: number;
  scheduledAmount: number;
  underInsured: boolean;
}

/** A policy that has firearms scheduled under it. */
export interface PolicySummary {
  policyId: number;
  policyName: string;
  isExpired: boolean;
  isExpiringSoon: boolean;
  individuallyScheduled: IndividualCoverage[];
}

/** The blanket policy in force today, against the combined value of every
 * active firearm that isn't individually scheduled. */
export interface BlanketSummary {
  policyId: number;
  policyName: string;
  limit: number;
  total: number;
  firearmCount: number;
  underInsured: boolean;
}

/** An unscheduled firearm with no blanket policy in force to cover it. */
export interface UninsuredFirearm {
  firearmId: number;
  estimatedValue: number;
}

export interface ValueSummary {
  collectionTotal: number;
  blanket: BlanketSummary | null;
  byPolicy: PolicySummary[];
  uninsured: UninsuredFirearm[];
}
