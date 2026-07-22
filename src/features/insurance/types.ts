// Mirrors src-tauri/src/models/insurance_policy.rs and
// src-tauri/src/services/valuation.rs's wire shapes (camelCase, per
// contracts/tauri-commands.md), User Story 3.

import type { CoverageKind, Firearm } from "../firearms/types";

export interface InsurancePolicy {
  id: number;
  name: string;
  policyNumber: string;
  insuranceCompany: string;
  companyContact: string | null;
  agentName: string | null;
  agentContact: string | null;
  blanketCoverageLimit: number;
  effectiveStartDate: string;
  effectiveEndDate: string;
  createdAt: string;
  updatedAt: string;
}

export type InsurancePolicyInput = Omit<InsurancePolicy, "id" | "createdAt" | "updatedAt">;

export interface AssignCoverageInput {
  policyId: number | null;
  coverageKind?: CoverageKind;
  scheduledCoverageAmount?: number;
}

export interface IndividualCoverage {
  firearmId: number;
  estimatedValue: number;
  scheduledAmount: number;
  underInsured: boolean;
}

export interface PolicySummary {
  policyId: number;
  policyName: string;
  isExpired: boolean;
  isExpiringSoon: boolean;
  blanketTotal: number;
  blanketLimit: number;
  blanketUnderInsured: boolean;
  individuallyScheduled: IndividualCoverage[];
}

export interface UnassignedFirearm {
  firearmId: number;
  estimatedValue: number;
}

export interface ValueSummary {
  collectionTotal: number;
  byPolicy: PolicySummary[];
  unassigned: UnassignedFirearm[];
}

export type { Firearm };
