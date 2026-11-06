import { beforeEach, describe, expect, it, vi } from "vitest";
import { createSessionScope } from "../session/sessionScope";
import * as databasesService from "./databasesService";

// 008 contracts/tauri-commands.md "Every request names
// its session": the open database's settings, saved-passphrase, backup and
// passphrase-change commands carry its session; choosing, creating and
// opening a database, and the backups of one that isn't open, do not.
// 009 contracts/tauri-commands.md "Commands (added)": the four wrappers for the
// upgrade (`upgrade_database`, `cancel_upgrade`, `check_backup`, and
// `restore_backup`'s `upgrade`) and the `upgrade:progress` event.

const backend = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
vi.mock("../../services/tauriClient", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../services/tauriClient")>()),
  invoke: backend.invoke,
  listen: backend.listen,
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
    backend.listen.mockReset().mockReturnValue(() => {});
  });

  it.each([
    // No passphrase is ever sent: the backend asks for it in the native
    // dialog (amended 2026-11-04 to 003 FR-007).
    ["save_passphrase", undefined, () => databasesService.savePassphrase(scope)],
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
    ["restore_backup", { checkId: 5 }, () => databasesService.restoreBackup(scope, 5)],
    ["check_backup", { backupPath: "/b" }, () => databasesService.checkBackup(scope, "/b")],
    [
      "restore_backup",
      { checkId: 5, upgrade: true },
      () => databasesService.restoreBackup(scope, 5, undefined, true),
    ],
    ["change_passphrase", undefined, () => databasesService.changePassphrase(scope)],
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
    ["open_database", { path: "/a" }, () => databasesService.openDatabase("/a")],
    [
      "open_database",
      { path: "/a", useSavedPassphrase: true },
      () => databasesService.openDatabase("/a", { useSavedPassphrase: true }),
    ],
    [
      "create_database",
      { folder: "/f", name: "N" },
      () => databasesService.createDatabase({ folder: "/f", name: "N" }),
    ],
    ["drop_backup_check", { checkId: 5 }, () => databasesService.dropBackupCheck(5)],
    ["remove_recent_database", { path: "/a" }, () => databasesService.removeRecentDatabase("/a")],
    [
      "locate_database",
      { path: "/a", newPath: "/b" },
      () => databasesService.locateDatabase("/a", "/b"),
    ],
    ["list_backups", { databasePath: "/a" }, () => databasesService.listBackups(null, "/a")],
    [
      "restore_backup",
      { checkId: 5, databasePath: "/a" },
      () => databasesService.restoreBackup(null, 5, "/a"),
    ],
    [
      "restore_backup",
      { checkId: 5, databasePath: "/a", upgrade: true },
      () => databasesService.restoreBackup(null, 5, "/a", true),
    ],
    [
      "check_backup",
      { backupPath: "/b", databasePath: "/a" },
      () => databasesService.checkBackup(null, "/b", "/a"),
    ],
    // 009 contracts/tauri-commands.md: all three are unscoped.
    ["upgrade_database", { awaitingId: 7 }, () => databasesService.upgradeDatabase(7)],
    [
      "upgrade_database",
      { awaitingId: 7, backupFolder: "/mnt/usb/Backups" },
      () => databasesService.upgradeDatabase(7, "/mnt/usb/Backups"),
    ],
    ["cancel_upgrade", { awaitingId: 7 }, () => databasesService.cancelUpgrade(7)],
  ])("sends %s with no session", async (command, args, call) => {
    await call();
    // No options at all: only a scope names a session.
    expect(backend.invoke.mock.calls).toEqual([args === undefined ? [command] : [command, args]]);
  });

  it("sends the database it names with no session even when a scope is held", async () => {
    await databasesService.listBackups(scope, "/a");
    await databasesService.restoreBackup(scope, 5, "/a");
    await databasesService.checkBackup(scope, "/b", "/a");
    expect(backend.invoke.mock.calls.map((call) => call.length)).toEqual([2, 2, 2]);
  });

  // The tables above compare with toEqual, which ignores a key set to
  // undefined; the backend must not be sent one (009 contracts/tauri-commands.md).
  it("leaves out the folder, the database and the upgrade flag when there are none", async () => {
    await databasesService.upgradeDatabase(7);
    await databasesService.restoreBackup(scope, 5);
    await databasesService.restoreBackup(scope, 5, undefined, false);
    await databasesService.checkBackup(scope, "/b");
    await databasesService.openDatabase("/a", {});
    expect(backend.invoke.mock.calls.map(([, args]) => Object.keys(args).sort())).toEqual([
      ["awaitingId"],
      ["checkId"],
      ["checkId"],
      ["backupPath"],
      ["path"],
    ]);
  });

  it("returns what the upgrade, the check and the cancel answer", async () => {
    const status = { sessionId: 3 };
    backend.invoke
      .mockResolvedValueOnce(status)
      .mockResolvedValueOnce({ checkId: 5, upgradeTo: "0.2.0" })
      .mockResolvedValueOnce(null);
    await expect(databasesService.upgradeDatabase(7)).resolves.toBe(status);
    await expect(databasesService.checkBackup(scope, "/b")).resolves.toEqual({
      checkId: 5,
      upgradeTo: "0.2.0",
    });
    await expect(databasesService.cancelUpgrade(7)).resolves.toBeNull();
  });

  it("listens to upgrade:progress and hands on each payload", () => {
    const stop = vi.fn();
    backend.listen.mockReturnValue(stop);
    const handler = vi.fn();

    const unlisten = databasesService.onUpgradeProgress(handler);

    expect(backend.listen).toHaveBeenCalledExactlyOnceWith(
      "upgrade:progress",
      expect.any(Function),
    );
    const payload = { phase: "backingUp", processed: 1, total: 4, showNow: true };
    backend.listen.mock.calls[0][1](payload);
    expect(handler).toHaveBeenCalledExactlyOnceWith(payload);
    expect(unlisten).toBe(stop);
  });

  it("refuses to work on the open database with no scope", async () => {
    await expect(databasesService.listBackups(null)).rejects.toMatchObject({
      code: "DATABASE_CLOSED",
    });
    await expect(databasesService.restoreBackup(null, 5)).rejects.toMatchObject({
      code: "DATABASE_CLOSED",
    });
    await expect(databasesService.checkBackup(null, "/b")).rejects.toMatchObject({
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
