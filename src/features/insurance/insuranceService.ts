import { invoke } from "../../services/tauriClient";
import type { Firearm } from "../firearms/types";
import type {
  AssignCoverageInput,
  DeletePolicyResult,
  InsurancePolicy,
  InsurancePolicyInput,
  PolicyDeletionImpact,
  ScheduledFirearmsAction,
  ValueSummary,
} from "./types";

export function listInsurancePolicies(): Promise<InsurancePolicy[]> {
  return invoke<InsurancePolicy[]>("list_insurance_policies");
}

export function createInsurancePolicy(input: InsurancePolicyInput): Promise<InsurancePolicy> {
  return invoke<InsurancePolicy>("create_insurance_policy", { input });
}

export function updateInsurancePolicy(
  id: number,
  input: InsurancePolicyInput,
): Promise<InsurancePolicy> {
  return invoke<InsurancePolicy>("update_insurance_policy", { id, input });
}

export function getPolicyDeletionImpact(id: number): Promise<PolicyDeletionImpact> {
  return invoke<PolicyDeletionImpact>("get_policy_deletion_impact", { id });
}

export function deleteInsurancePolicy(
  id: number,
  confirmed: boolean,
  scheduledFirearms?: ScheduledFirearmsAction,
): Promise<DeletePolicyResult> {
  return invoke<DeletePolicyResult>("delete_insurance_policy", {
    id,
    confirmed,
    scheduledFirearms,
  });
}

export function assignFirearmCoverage(
  firearmId: number,
  input: AssignCoverageInput,
): Promise<Firearm> {
  return invoke<Firearm>("assign_firearm_coverage", { firearmId, input });
}

export function getValueSummary(): Promise<ValueSummary> {
  return invoke<ValueSummary>("get_value_summary");
}
