import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "../../services/tauriClient";
import { createSessionScope } from "../session/sessionScope";
import * as accessoriesService from "./accessoriesService";

// These tests mock `tauriClient`'s `invoke`, below the scope, so they pin the
// arguments the backend receives and the session the call names (008 FR-001).
// Tauri matches the top-level names to the
// command's parameters (camelCase of the Rust snake_case) and a struct
// parameter is one object under its name: `assign_accessory_coverage` takes
// `accessory_id` and `input`, so a flat `{ accessoryId, policyId, ... }` is
// rejected by the real app although a test mocking `accessoriesService` passes
// (006 US1-8, 006 contracts/tauri-commands.md "Accessories (new)").
vi.mock("../../services/tauriClient", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../services/tauriClient")>()),
  invoke: vi.fn(),
}));

const scope = createSessionScope(4);
const SESSION = { session: 4 };

beforeEach(() => {
  vi.mocked(invoke).mockReset().mockResolvedValue({});
});

describe("accessoriesService IPC arguments", () => {
  it("assign_accessory_coverage: accessoryId and an input object, as the firearm's call", async () => {
    await accessoriesService.assignAccessoryCoverage(scope, 3, 7, 400);

    expect(invoke).toHaveBeenCalledWith(
      "assign_accessory_coverage",
      {
        accessoryId: 3,
        input: { policyId: 7, scheduledCoverageAmount: 400 },
      },
      SESSION,
    );
  });

  it("assign_accessory_coverage: unscheduling sends null for both", async () => {
    await accessoriesService.assignAccessoryCoverage(scope, 3, null, null);

    expect(invoke).toHaveBeenCalledWith(
      "assign_accessory_coverage",
      {
        accessoryId: 3,
        input: { policyId: null, scheduledCoverageAmount: null },
      },
      SESSION,
    );
  });

  it("create_accessory and list_accessories take one input object", async () => {
    const input = { make: "Leupold" } as never;
    await accessoriesService.createAccessory(scope, input);
    await accessoriesService.listAccessories(scope, { query: "vx" });

    expect(invoke).toHaveBeenNthCalledWith(1, "create_accessory", { input }, SESSION);
    expect(invoke).toHaveBeenNthCalledWith(
      2,
      "list_accessories",
      { input: { query: "vx" } },
      SESSION,
    );
  });

  it("update_accessory, dispose_accessory and reverse_accessory_disposition take id and input", async () => {
    const input = { make: "Leupold" } as never;
    await accessoriesService.updateAccessory(scope, 3, input);
    await accessoriesService.disposeAccessory(scope, 3, input);
    await accessoriesService.reverseAccessoryDisposition(scope, 3, { history: "keep" });

    expect(invoke).toHaveBeenNthCalledWith(1, "update_accessory", { id: 3, input }, SESSION);
    expect(invoke).toHaveBeenNthCalledWith(2, "dispose_accessory", { id: 3, input }, SESSION);
    expect(invoke).toHaveBeenNthCalledWith(
      3,
      "reverse_accessory_disposition",
      {
        id: 3,
        input: { history: "keep" },
      },
      SESSION,
    );
  });

  it("get_accessory and delete_accessory take id (and confirmed)", async () => {
    await accessoriesService.getAccessory(scope, 3);
    await accessoriesService.deleteAccessory(scope, 3, true);

    expect(invoke).toHaveBeenNthCalledWith(1, "get_accessory", { id: 3 }, SESSION);
    expect(invoke).toHaveBeenNthCalledWith(
      2,
      "delete_accessory",
      { id: 3, confirmed: true },
      SESSION,
    );
  });

  it("list_accessory_kinds takes nothing", async () => {
    await accessoriesService.listAccessoryKinds(scope);

    expect(invoke).toHaveBeenCalledWith("list_accessory_kinds", undefined, SESSION);
  });
});
