import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CommandFailure } from "../../services/tauriClient";
import * as insuranceService from "./insuranceService";
import { PolicyDeleteDialog } from "./PolicyDeleteDialog";
import type { InsurancePolicy, PolicyDeletionImpact } from "./types";
import type { RecordLabel } from "../mounts/types";

vi.mock("./insuranceService");

const policy = { id: 3, name: "Collectibles rider" } as InsurancePolicy;

// specs/006-accessory-links FR-009, contracts/ui-accessories.md §10: the
// impact counts records of both kinds, and lists them as `RecordLabel`s.
const colt: RecordLabel = {
  record: { kind: "firearm", id: 1 },
  make: "Colt",
  model: "Python",
  nickname: "Snake",
  typeName: "Handgun",
  serialNumber: "V1",
  status: "active",
};
const glock: RecordLabel = {
  record: { kind: "firearm", id: 2 },
  make: "Glock",
  model: "19",
  nickname: null,
  typeName: "Handgun",
  serialNumber: "G1",
  status: "active",
};
const optic: RecordLabel = {
  record: { kind: "accessory", id: 3 },
  make: "Leupold",
  model: "VX-5HD 3-15x44",
  nickname: null,
  typeName: "Optic",
  serialNumber: null,
  status: "active",
};

const impact: PolicyDeletionImpact = {
  isExpired: false,
  isBlanketInForce: false,
  scheduledCounts: { firearms: 2, accessories: 0 },
  scheduledRecords: [colt, glock],
  blanketCounts: { firearms: 0, accessories: 0 },
  unscheduleOutcome: "uninsured",
  otherPolicies: [
    { id: 4, name: "Homeowner's rider", isExpired: false },
    { id: 5, name: "Old rider", isExpired: true },
  ],
};

function renderDialog(overrides: Partial<PolicyDeletionImpact> = {}) {
  vi.mocked(insuranceService.getPolicyDeletionImpact).mockResolvedValue({
    ...impact,
    ...overrides,
  });
  const onDeleted = vi.fn().mockResolvedValue(undefined);
  const onOpenChange = vi.fn();
  render(
    <PolicyDeleteDialog open onOpenChange={onOpenChange} policy={policy} onDeleted={onDeleted} />,
  );
  return { onDeleted, onOpenChange };
}

const deleteButton = () => screen.findByRole("button", { name: "Delete policy" });

describe("PolicyDeleteDialog (FR-034)", () => {
  beforeEach(() => {
    vi.mocked(insuranceService.deleteInsurancePolicy).mockResolvedValue({
      deleted: true,
      movedCount: 0,
      unscheduledCount: 0,
    });
  });

  it("deletes a policy with nothing scheduled on it straight away", async () => {
    const user = userEvent.setup();
    const { onDeleted } = renderDialog({
      scheduledCounts: { firearms: 0, accessories: 0 },
      scheduledRecords: [],
    });

    await user.click(await deleteButton());

    expect(insuranceService.deleteInsurancePolicy).toHaveBeenCalledWith(3, true, undefined);
    expect(onDeleted).toHaveBeenCalled();
  });

  it("lists the scheduled records, with nicknames, and holds delete back until a choice is made", async () => {
    renderDialog();

    const items = await screen.findAllByRole("listitem");
    expect(items[0]).toHaveTextContent("Colt Python “Snake”");
    expect(items[1]).toHaveTextContent("Glock 19");
    expect(await deleteButton()).toBeDisabled();
  });

  it("counts and lists scheduled firearms and accessories together, each by its name (US1, FR-009)", async () => {
    const user = userEvent.setup();
    renderDialog({
      scheduledCounts: { firearms: 2, accessories: 1 },
      scheduledRecords: [colt, glock, optic],
    });

    const items = await screen.findAllByRole("listitem");
    expect(items).toHaveLength(3);
    expect(items[2]).toHaveTextContent("Leupold VX-5HD 3-15x44 · Optic");
    expect(
      screen.getByText(/2 firearms and 1 accessory are scheduled under it/),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("radio", { name: /Leave unscheduled/ }));
    expect(
      screen.getByRole("checkbox", {
        name: "2 firearms and 1 accessory will lose their scheduled coverage",
      }),
    ).toBeInTheDocument();
  });

  it("moves the firearms to another policy, reminding the user to check its coverage", async () => {
    const user = userEvent.setup();
    renderDialog();

    await user.click(await screen.findByRole("radio", { name: /Move to another policy/ }));
    expect(await deleteButton()).toBeDisabled(); // no target chosen yet
    expect(screen.getByText(/keeps its scheduled amount/i)).toBeInTheDocument();
    expect(
      screen.getByText(/confirm that the new policy actually covers these firearms\./i),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("combobox", { name: "Move to" }));
    await user.click(await screen.findByRole("option", { name: /Homeowner's rider/ }));
    await user.click(await deleteButton());

    expect(insuranceService.deleteInsurancePolicy).toHaveBeenCalledWith(3, true, {
      action: "move",
      targetPolicyId: 4,
    });
  });

  it("warns when the chosen target has itself expired", async () => {
    const user = userEvent.setup();
    renderDialog();

    await user.click(await screen.findByRole("radio", { name: /Move to another policy/ }));
    await user.click(screen.getByRole("combobox", { name: "Move to" }));
    await user.click(await screen.findByRole("option", { name: /Old rider/ }));

    expect(screen.getByText(/that policy has expired/i)).toBeInTheDocument();
  });

  it("needs the stronger confirmation to leave a non-expired policy's firearms unscheduled", async () => {
    const user = userEvent.setup();
    renderDialog();

    await user.click(await screen.findByRole("radio", { name: /Leave unscheduled/ }));
    expect(screen.getByText(/will be uninsured/i)).toBeInTheDocument();
    expect(await deleteButton()).toBeDisabled();

    await user.click(
      screen.getByRole("checkbox", { name: /2 firearms will lose their scheduled coverage/ }),
    );
    await user.click(await deleteButton());

    expect(insuranceService.deleteInsurancePolicy).toHaveBeenCalledWith(3, true, {
      action: "unschedule",
      confirmUnschedule: true,
    });
  });

  it("says unscheduled firearms will be blanket-covered when another blanket policy is in force", async () => {
    const user = userEvent.setup();
    renderDialog({ unscheduleOutcome: "blanket" });

    await user.click(await screen.findByRole("radio", { name: /Leave unscheduled/ }));

    expect(screen.getByText(/covered by the blanket policy in force/i)).toBeInTheDocument();
  });

  it("asks only for the warning to be acknowledged when the policy has expired", async () => {
    const user = userEvent.setup();
    renderDialog({ isExpired: true });

    expect(await screen.findByText(/already treated as uninsured/i)).toBeInTheDocument();
    await user.click(screen.getByRole("radio", { name: /Leave unscheduled/ }));
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
    await user.click(await deleteButton());

    expect(insuranceService.deleteInsurancePolicy).toHaveBeenCalledWith(3, true, {
      action: "unschedule",
    });
  });

  it("offers only leaving them unscheduled when there is no other policy", async () => {
    renderDialog({ otherPolicies: [] });

    expect(await screen.findByRole("radio", { name: /Leave unscheduled/ })).toBeInTheDocument();
    expect(screen.queryByRole("radio", { name: /Move to another policy/ })).not.toBeInTheDocument();
  });

  it("warns how many unscheduled firearms lose their blanket coverage when the blanket policy in force is deleted", async () => {
    renderDialog({
      scheduledCounts: { firearms: 0, accessories: 0 },
      scheduledRecords: [],
      isBlanketInForce: true,
      blanketCounts: { firearms: 10, accessories: 2 },
    });

    expect(
      await screen.findByText(/10 firearms and 2 accessories that aren't scheduled/),
    ).toBeInTheDocument();
    expect(screen.getByText(/lose its blanket coverage/i)).toBeInTheDocument();
  });

  it("counts an accessory among the records that lose blanket coverage", async () => {
    renderDialog({
      scheduledCounts: { firearms: 0, accessories: 0 },
      scheduledRecords: [],
      isBlanketInForce: true,
      blanketCounts: { firearms: 0, accessories: 1 },
    });

    expect(await screen.findByText(/1 accessory that isn't scheduled/)).toBeInTheDocument();
  });

  it("keeps the dialog open and shows the reason when the deletion is refused", async () => {
    const user = userEvent.setup();
    vi.mocked(insuranceService.deleteInsurancePolicy).mockRejectedValue(
      new CommandFailure({
        code: "POLICY_HAS_FIREARMS",
        message: "This policy still covers 2 firearm(s).",
      }),
    );
    const { onDeleted, onOpenChange } = renderDialog({
      scheduledCounts: { firearms: 0, accessories: 0 },
      scheduledRecords: [],
    });

    await user.click(await deleteButton());

    expect(await screen.findByText("This policy still covers 2 firearm(s).")).toBeInTheDocument();
    expect(onDeleted).not.toHaveBeenCalled();
    expect(onOpenChange).not.toHaveBeenCalledWith(false);
  });
});
