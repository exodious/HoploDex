import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "../../services/tauriClient";
import { createSessionScope } from "../session/sessionScope";
import * as mountsService from "./mountsService";

// Pins the arguments the backend receives (see accessoriesService.test.ts):
// both commands take one struct parameter named `input`.
vi.mock("../../services/tauriClient", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../services/tauriClient")>()),
  invoke: vi.fn(),
}));

const scope = createSessionScope(4);
const SESSION = { session: 4 };

beforeEach(() => {
  vi.mocked(invoke).mockReset().mockResolvedValue({});
});

describe("mountsService IPC arguments", () => {
  it("mount_record: item and host under input; null unmounts", async () => {
    await mountsService.mountRecord(
      scope,
      { kind: "accessory", id: 3 },
      { kind: "firearm", id: 9 },
    );
    await mountsService.mountRecord(scope, { kind: "accessory", id: 3 }, null);

    expect(invoke).toHaveBeenNthCalledWith(
      1,
      "mount_record",
      {
        input: { item: { kind: "accessory", id: 3 }, host: { kind: "firearm", id: 9 } },
      },
      SESSION,
    );
    expect(invoke).toHaveBeenNthCalledWith(
      2,
      "mount_record",
      {
        input: { item: { kind: "accessory", id: 3 }, host: null },
      },
      SESSION,
    );
  });

  it("list_mount_candidates: the whole input under input", async () => {
    const input = { role: "item", record: { kind: "firearm", id: 9 }, query: "vx" } as const;
    await mountsService.listMountCandidates(scope, input);

    expect(invoke).toHaveBeenCalledWith("list_mount_candidates", { input }, SESSION);
  });
});
