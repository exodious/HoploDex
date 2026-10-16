import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "../../services/tauriClient";
import { createSessionScope } from "../session/sessionScope";
import * as insuranceService from "./insuranceService";

// Pins the arguments the backend receives and the session each call names
// (008 FR-001).
vi.mock("../../services/tauriClient", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../services/tauriClient")>()),
  invoke: vi.fn(),
}));

const scope = createSessionScope(4);
const SESSION = { session: 4 };

beforeEach(() => {
  vi.mocked(invoke).mockReset().mockResolvedValue({});
});

describe("insuranceService IPC arguments", () => {
  it("every command names the scope's session", async () => {
    const input = { name: "Home" } as never;
    await insuranceService.listInsurancePolicies(scope);
    await insuranceService.createInsurancePolicy(scope, input);
    await insuranceService.updateInsurancePolicy(scope, 2, input);
    await insuranceService.getPolicyDeletionImpact(scope, 2);
    await insuranceService.deleteInsurancePolicy(scope, 2, true);
    await insuranceService.assignFirearmCoverage(scope, 5, { policyId: 2 } as never);
    await insuranceService.getValueSummary(scope);

    expect(invoke).toHaveBeenNthCalledWith(1, "list_insurance_policies", undefined, SESSION);
    expect(invoke).toHaveBeenNthCalledWith(2, "create_insurance_policy", { input }, SESSION);
    expect(invoke).toHaveBeenNthCalledWith(3, "update_insurance_policy", { id: 2, input }, SESSION);
    expect(invoke).toHaveBeenNthCalledWith(4, "get_policy_deletion_impact", { id: 2 }, SESSION);
    expect(invoke).toHaveBeenNthCalledWith(
      5,
      "delete_insurance_policy",
      { id: 2, confirmed: true, scheduledFirearms: undefined },
      SESSION,
    );
    expect(invoke).toHaveBeenNthCalledWith(
      6,
      "assign_firearm_coverage",
      { firearmId: 5, input: { policyId: 2 } },
      SESSION,
    );
    expect(invoke).toHaveBeenNthCalledWith(7, "get_value_summary", undefined, SESSION);
  });
});
