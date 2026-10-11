import { beforeEach, describe, expect, it, vi } from "vitest";
import { createSessionScope } from "./sessionScope";
import * as sessionService from "./sessionService";

// specs/008-hardening-batch contracts/tauri-commands.md "Every request names
// its session": the commands that act on the open database carry its session;
// the ones that act on the running operation, the window or the app do not.

const backend = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("../../services/tauriClient", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../services/tauriClient")>()),
  invoke: backend.invoke,
}));

describe("sessionService", () => {
  const scope = createSessionScope(23);

  beforeEach(() => {
    backend.invoke.mockReset().mockResolvedValue(undefined);
  });

  it.each([
    [
      "dismiss_note",
      { note: "diskEncryption" },
      () => sessionService.dismissNote(scope, "diskEncryption"),
    ],
    ["close_database", { reason: "closed" }, () => sessionService.closeDatabase(scope, "closed")],
    ["lock_database", { draft: null }, () => sessionService.lockDatabase(scope, null)],
    [
      "stage_pending_changes",
      { draft: null },
      () => sessionService.stagePendingChanges(scope, null),
    ],
    [
      "resolve_pending_changes",
      { action: "resume" },
      () => sessionService.resolvePendingChanges(scope, "resume"),
    ],
  ])("sends %s with the session", async (command, args, call) => {
    await call();
    expect(backend.invoke).toHaveBeenCalledExactlyOnceWith(command, args, { session: 23 });
  });

  it.each([
    ["get_database_status", undefined, () => sessionService.getDatabaseStatus()],
    ["skip_backup", undefined, () => sessionService.skipBackup()],
    ["quit_application", undefined, () => sessionService.quitApplication()],
    ["note_activity", undefined, () => sessionService.noteActivity()],
    [
      "set_idle_paused",
      { reason: "nativeDialog", paused: true },
      () => sessionService.setIdlePaused(true),
    ],
  ])("sends %s with no session", async (command, args, call) => {
    await call();
    // No options at all: only a scope names a session.
    expect(backend.invoke.mock.calls).toEqual([args === undefined ? [command] : [command, args]]);
  });

  it("refuses a scoped command once the scope has ended, without sending it", async () => {
    const ended = createSessionScope(24);
    ended.end();
    await expect(sessionService.lockDatabase(ended, null)).rejects.toMatchObject({
      code: "DATABASE_CLOSED",
    });
    expect(backend.invoke).not.toHaveBeenCalled();
  });
});
