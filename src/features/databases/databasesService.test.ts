import { beforeEach, describe, expect, it, vi } from "vitest";
import { createSessionScope } from "../session/sessionScope";
import * as databasesService from "./databasesService";

// 008 contracts/tauri-commands.md "Every request names
// its session": the open database's settings, saved-passphrase, backup and
// passphrase-change commands carry its session; choosing, creating and
// opening a database, and the backups of one that isn't open, do not.

const backend = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("../../services/tauriClient", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../services/tauriClient")>()),
  invoke: backend.invoke,
}));

const backupSettings = {
  enabled: true,
  keepCount: 5,
  location: { kind: "default" },
} as const;
const lockSettings = { idleEnabled: true, idleMinutes: 15, onScreenLock: true } as const;

describe("databasesService", () => {
  const scope = createSessionScope(31);

  beforeEach(() => {
    backend.invoke.mockReset().mockResolvedValue(undefined);
  });

  it.each([
    ["save_passphrase", { passphrase: "pw" }, () => databasesService.savePassphrase(scope, "pw")],
    ["forget_saved_passphrase", {}, () => databasesService.forgetSavedPassphrase(scope)],
    [
      "forget_saved_passphrase",
      { path: "/a.hoplodex" },
      () => databasesService.forgetSavedPassphrase(scope, "/a.hoplodex"),
    ],
    [
      "update_backup_settings",
      backupSettings,
      () => databasesService.updateBackupSettings(scope, backupSettings),
    ],
    [
      "update_lock_settings",
      lockSettings,
      () => databasesService.updateLockSettings(scope, lockSettings),
    ],
    ["list_backups", {}, () => databasesService.listBackups(scope)],
    [
      "restore_backup",
      { backupPath: "/b", backupPassphrase: "pw" },
      () => databasesService.restoreBackup(scope, "/b", "pw"),
    ],
    [
      "change_passphrase",
      { currentPassphrase: "old", newPassphrase: "new" },
      () => databasesService.changePassphrase(scope, "old", "new"),
    ],
    [
      "delete_all_backups",
      { confirmed: true },
      () => databasesService.deleteAllBackups(scope, true),
    ],
  ])("sends %s for the open database with the session", async (command, args, call) => {
    await call();
    expect(backend.invoke).toHaveBeenCalledExactlyOnceWith(command, args, { session: 31 });
  });

  it.each([
    ["get_chooser_state", undefined, () => databasesService.getChooserState()],
    [
      "open_database",
      { path: "/a", passphrase: "pw" },
      () => databasesService.openDatabase("/a", "pw"),
    ],
    ["remove_recent_database", { path: "/a" }, () => databasesService.removeRecentDatabase("/a")],
    [
      "locate_database",
      { path: "/a", newPath: "/b" },
      () => databasesService.locateDatabase("/a", "/b"),
    ],
    ["list_backups", { databasePath: "/a" }, () => databasesService.listBackups(null, "/a")],
    [
      "restore_backup",
      { backupPath: "/b", backupPassphrase: "pw", databasePath: "/a" },
      () => databasesService.restoreBackup(null, "/b", "pw", "/a"),
    ],
  ])("sends %s with no session", async (command, args, call) => {
    await call();
    // No options at all: only a scope names a session.
    expect(backend.invoke.mock.calls).toEqual([args === undefined ? [command] : [command, args]]);
  });

  it("sends the database it names with no session even when a scope is held", async () => {
    await databasesService.listBackups(scope, "/a");
    await databasesService.restoreBackup(scope, "/b", "pw", "/a");
    expect(backend.invoke.mock.calls.map((call) => call.length)).toEqual([2, 2]);
  });

  it("refuses to work on the open database with no scope", async () => {
    await expect(databasesService.listBackups(null)).rejects.toMatchObject({
      code: "DATABASE_CLOSED",
    });
    await expect(databasesService.restoreBackup(null, "/b", "pw")).rejects.toMatchObject({
      code: "DATABASE_CLOSED",
    });
    expect(backend.invoke).not.toHaveBeenCalled();
  });

  it("refuses a scoped command once the scope has ended, without sending it", async () => {
    const ended = createSessionScope(32);
    ended.end();
    await expect(databasesService.updateLockSettings(ended, lockSettings)).rejects.toMatchObject({
      code: "DATABASE_CLOSED",
    });
    expect(backend.invoke).not.toHaveBeenCalled();
  });
});
