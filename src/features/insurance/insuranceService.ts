import type { SessionScope } from "../session/sessionScope";
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

export function listInsurancePolicies(scope: SessionScope): Promise<InsurancePolicy[]> {
  return scope.invoke<InsurancePolicy[]>("list_insurance_policies");
}

export function createInsurancePolicy(
  scope: SessionScope,
  input: InsurancePolicyInput,
): Promise<InsurancePolicy> {
  return scope.invoke<InsurancePolicy>("create_insurance_policy", { input });
}

export function updateInsurancePolicy(
  scope: SessionScope,
  id: number,
  input: InsurancePolicyInput,
): Promise<InsurancePolicy> {
  return scope.invoke<InsurancePolicy>("update_insurance_policy", { id, input });
}

export function getPolicyDeletionImpact(
  scope: SessionScope,
  id: number,
): Promise<PolicyDeletionImpact> {
  return scope.invoke<PolicyDeletionImpact>("get_policy_deletion_impact", { id });
}

export function deleteInsurancePolicy(
  scope: SessionScope,
  id: number,
  confirmed: boolean,
  scheduledFirearms?: ScheduledFirearmsAction,
): Promise<DeletePolicyResult> {
  return scope.invoke<DeletePolicyResult>("delete_insurance_policy", {
    id,
    confirmed,
    scheduledFirearms,
  });
}

export function assignFirearmCoverage(
  scope: SessionScope,
  firearmId: number,
  input: AssignCoverageInput,
): Promise<Firearm> {
  return scope.invoke<Firearm>("assign_firearm_coverage", { firearmId, input });
}

export function getValueSummary(scope: SessionScope): Promise<ValueSummary> {
  return scope.invoke<ValueSummary>("get_value_summary");
}
