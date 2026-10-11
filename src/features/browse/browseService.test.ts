import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "../../services/tauriClient";
import { createSessionScope } from "../session/sessionScope";
import { listFirearms } from "./browseService";

vi.mock("../../services/tauriClient", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../services/tauriClient")>()),
  invoke: vi.fn(),
}));

beforeEach(() => {
  vi.mocked(invoke).mockReset().mockResolvedValue({ groups: [] });
});

describe("browseService", () => {
  it("list_firearms names the scope's session and asks for nothing else first", async () => {
    await listFirearms(createSessionScope(4), { includeDisposed: true });

    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith(
      "list_firearms",
      { input: { includeDisposed: true } },
      { session: 4 },
    );
  });
});
