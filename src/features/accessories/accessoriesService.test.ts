import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import * as accessoriesService from "./accessoriesService";

// These tests mock Tauri's own `invoke`, below `tauriClient`, so they pin the
// arguments the backend receives. Tauri matches the top-level names to the
// command's parameters (camelCase of the Rust snake_case) and a struct
// parameter is one object under its name: `assign_accessory_coverage` takes
// `accessory_id` and `input`, so a flat `{ accessoryId, policyId, ... }` is
// rejected by the real app although a test mocking `accessoriesService` passes
// (US1/AC8, contracts/tauri-commands.md "Accessories (new)").
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

beforeEach(() => {
  vi.mocked(invoke).mockReset().mockResolvedValue({});
});

describe("accessoriesService IPC arguments", () => {
  it("assign_accessory_coverage: accessoryId and an input object, as the firearm's call", async () => {
    await accessoriesService.assignAccessoryCoverage(3, 7, 400);

    expect(invoke).toHaveBeenCalledWith("assign_accessory_coverage", {
      accessoryId: 3,
      input: { policyId: 7, scheduledCoverageAmount: 400 },
    });
  });

  it("assign_accessory_coverage: unscheduling sends null for both", async () => {
    await accessoriesService.assignAccessoryCoverage(3, null, null);

    expect(invoke).toHaveBeenCalledWith("assign_accessory_coverage", {
      accessoryId: 3,
      input: { policyId: null, scheduledCoverageAmount: null },
    });
  });

  it("create_accessory and list_accessories take one input object", async () => {
    const input = { make: "Leupold" } as never;
    await accessoriesService.createAccessory(input);
    await accessoriesService.listAccessories({ query: "vx" });

    expect(invoke).toHaveBeenNthCalledWith(1, "create_accessory", { input });
    expect(invoke).toHaveBeenNthCalledWith(2, "list_accessories", { input: { query: "vx" } });
  });

  it("update_accessory, dispose_accessory and reverse_accessory_disposition take id and input", async () => {
    const input = { make: "Leupold" } as never;
    await accessoriesService.updateAccessory(3, input);
    await accessoriesService.disposeAccessory(3, input);
    await accessoriesService.reverseAccessoryDisposition(3, { history: "keep" });

    expect(invoke).toHaveBeenNthCalledWith(1, "update_accessory", { id: 3, input });
    expect(invoke).toHaveBeenNthCalledWith(2, "dispose_accessory", { id: 3, input });
    expect(invoke).toHaveBeenNthCalledWith(3, "reverse_accessory_disposition", {
      id: 3,
      input: { history: "keep" },
    });
  });

  it("get_accessory and delete_accessory take id (and confirmed)", async () => {
    await accessoriesService.getAccessory(3);
    await accessoriesService.deleteAccessory(3, true);

    expect(invoke).toHaveBeenNthCalledWith(1, "get_accessory", { id: 3 });
    expect(invoke).toHaveBeenNthCalledWith(2, "delete_accessory", { id: 3, confirmed: true });
  });

  it("list_accessory_kinds takes nothing", async () => {
    await accessoriesService.listAccessoryKinds();

    expect(invoke).toHaveBeenCalledWith("list_accessory_kinds", undefined);
  });
});
