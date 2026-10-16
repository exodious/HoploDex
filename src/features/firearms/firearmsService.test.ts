import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "../../services/tauriClient";
import { createSessionScope } from "../session/sessionScope";
import * as firearmsService from "./firearmsService";

// Pins the arguments the backend receives and the session each call names
// (008 FR-001), the entry commands included.
vi.mock("../../services/tauriClient", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../services/tauriClient")>()),
  invoke: vi.fn(),
}));

const scope = createSessionScope(4);
const SESSION = { session: 4 };

beforeEach(() => {
  vi.mocked(invoke).mockReset().mockResolvedValue({ suggestions: [] });
});

describe("firearmsService IPC arguments", () => {
  it("record commands name the scope's session", async () => {
    const input = { make: "Glock" } as never;
    await firearmsService.createFirearm(scope, input, true);
    await firearmsService.updateFirearm(scope, 3, input);
    await firearmsService.getFirearm(scope, 3);
    await firearmsService.deleteFirearm(scope, 3, true);

    expect(invoke).toHaveBeenNthCalledWith(
      1,
      "create_firearm",
      { input, confirmedWarnings: true },
      SESSION,
    );
    expect(invoke).toHaveBeenNthCalledWith(
      2,
      "update_firearm",
      { id: 3, input, confirmedWarnings: undefined },
      SESSION,
    );
    expect(invoke).toHaveBeenNthCalledWith(3, "get_firearm", { id: 3 }, SESSION);
    expect(invoke).toHaveBeenNthCalledWith(
      4,
      "delete_firearm",
      { id: 3, confirmed: true },
      SESSION,
    );
  });

  it("entry commands name the scope's session", async () => {
    await firearmsService.suggestEntries(scope, "model", "gl", "Glock");
    await firearmsService.settleEntry(scope, "make", "glock");
    await firearmsService.listActionTypes(scope);
    await firearmsService.listFirearmTypes(scope);
    await firearmsService.listRegistrationClasses(scope);

    expect(invoke).toHaveBeenNthCalledWith(
      1,
      "suggest_entries",
      { input: { field: "model", text: "gl", make: "Glock" } },
      SESSION,
    );
    expect(invoke).toHaveBeenNthCalledWith(
      2,
      "settle_entry",
      { input: { field: "make", text: "glock" } },
      SESSION,
    );
    expect(invoke).toHaveBeenNthCalledWith(3, "list_action_types", undefined, SESSION);
    expect(invoke).toHaveBeenNthCalledWith(4, "list_firearm_types", undefined, SESSION);
    expect(invoke).toHaveBeenNthCalledWith(5, "list_registration_classes", undefined, SESSION);
  });

  it("refuses after the scope ended, without sending", async () => {
    const ended = createSessionScope(5);
    ended.end();

    await expect(firearmsService.getFirearm(ended, 3)).rejects.toMatchObject({
      code: "DATABASE_CLOSED",
    });
    expect(invoke).not.toHaveBeenCalled();
  });
});
