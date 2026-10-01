// Mirrors src-tauri/src/commands/insurance.rs, src-tauri/src/models/
// insurance_policy.rs and src-tauri/src/services/{insurance_status,
// valuation}.rs's wire shapes (camelCase, per contracts/tauri-commands.md),
// User Story 3.

import type { RecordCounts } from "../mounts/recordCounts";
import type { RecordLabel, RecordRef } from "../mounts/types";

export interface InsurancePolicy {
  id: number;
  name: string;
  policyNumber: string;
  insuranceCompany: string;
  companyContact: string | null;
  agentName: string | null;
  agentContact: string | null;
  /** Free-form notes (FR-027); null when there are none. */
  notes: string | null;
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
  /** specs/006-accessory-links: a firearm or an accessory. */
  record: RecordRef;
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
  accessoryCount: number;
  underInsured: boolean;
}

/** An unscheduled firearm or accessory with no blanket policy in force to
 * cover it. */
export interface UninsuredRecord {
  record: RecordRef;
  estimatedValue: number;
}

export interface ValueSummary {
  /** `firearmsTotal + accessoriesTotal` (006 SC-005). */
  collectionTotal: number;
  firearmsTotal: number;
  /** 006 FR-008: the active accessories' subtotal. */
  accessoriesTotal: number;
  blanket: BlanketSummary | null;
  byPolicy: PolicySummary[];
  uninsured: UninsuredRecord[];
}

/** `get_policy_deletion_impact`'s output: what deleting a policy would do,
 * for the FR-034 dialog to explain before anything changes. */
export interface PolicyDeletionImpact {
  isExpired: boolean;
  isBlanketInForce: boolean;
  /** Firearms and accessories (006 FR-009), counted by kind (issue #56). */
  scheduledCounts: RecordCounts;
  scheduledRecords: RecordLabel[];
  /** Unscheduled firearms and accessories that lose blanket coverage if this
   * is the blanket policy in force (none otherwise). */
  blanketCounts: RecordCounts;
  /** What becomes of firearms left unscheduled. */
  unscheduleOutcome: "blanket" | "uninsured";
  otherPolicies: { id: number; name: string; isExpired: boolean }[];
}

/** How the firearms and accessories scheduled under a deleted policy are resolved (FR-034). */
export type ScheduledFirearmsAction =
  { action: "move"; targetPolicyId: number } | { action: "unschedule"; confirmUnschedule?: true };

export interface DeletePolicyResult {
  deleted: boolean;
  movedCount: number;
  unscheduledCount: number;
}
