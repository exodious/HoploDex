import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import * as mountsService from "./mountsService";

// Pins the arguments the backend receives (see accessoriesService.test.ts):
// both commands take one struct parameter named `input`.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

beforeEach(() => {
  vi.mocked(invoke).mockReset().mockResolvedValue({});
});

describe("mountsService IPC arguments", () => {
  it("mount_record: item and host under input; null unmounts", async () => {
    await mountsService.mountRecord({ kind: "accessory", id: 3 }, { kind: "firearm", id: 9 });
    await mountsService.mountRecord({ kind: "accessory", id: 3 }, null);

    expect(invoke).toHaveBeenNthCalledWith(1, "mount_record", {
      input: { item: { kind: "accessory", id: 3 }, host: { kind: "firearm", id: 9 } },
    });
    expect(invoke).toHaveBeenNthCalledWith(2, "mount_record", {
      input: { item: { kind: "accessory", id: 3 }, host: null },
    });
  });

  it("list_mount_candidates: the whole input under input", async () => {
    const input = { role: "item", record: { kind: "firearm", id: 9 }, query: "vx" } as const;
    await mountsService.listMountCandidates(input);

    expect(invoke).toHaveBeenCalledWith("list_mount_candidates", { input });
  });
});
