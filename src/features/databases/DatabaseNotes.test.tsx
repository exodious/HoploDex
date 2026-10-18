import { describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ToastProvider } from "../../components";
import { formatDateTime } from "../../lib/dates";
import { SessionContext } from "../session/sessionStore";
import type { SessionState } from "../session/sessionStore";
import { DatabaseNotes } from "./DatabaseNotes";
import type { DatabaseStatus } from "./types";

const MADE_AT = "2026-09-20T09:00:00Z";

function renderNotes(notes: Partial<DatabaseStatus["notes"]>) {
  const status: DatabaseStatus = {
    sessionId: 1,
    path: "/home/sam/Documents/HoploDex/Main collection.hoplodex",
    name: "Main collection",
    passphraseSaved: false,
    keyringAvailable: false,
    screenLockSupported: false,
    settings: {
      backups: {
        enabled: true,
        keepCount: 5,
        location: {
          kind: "default",
          path: "/home/sam/Documents/HoploDex/HoploDex backups",
          available: true,
        },
      },
      lock: { idleEnabled: true, idleMinutes: 10, onScreenLock: false },
    },
    pendingChanges: null,
    notes: {
      diskEncryption: false,
      openedBackup: null,
      restoredWithPassphraseOf: null,
      damagedFileKeptAt: null,
      upgraded: null,
      ...notes,
    },
  };
  const session = { status, dismissNote: vi.fn() } as unknown as SessionState;
  render(
    <ToastProvider>
      <SessionContext.Provider value={session}>
        <DatabaseNotes />
      </SessionContext.Provider>
    </ToastProvider>,
  );
  return session;
}

describe("DatabaseNotes (003 contracts/ui-databases.md §10, and §0 for naming)", () => {
  it("calls the open database the database, not by its name", () => {
    renderNotes({ restoredWithPassphraseOf: MADE_AT });

    expect(screen.getByRole("region", { name: "Restored from a backup" })).toHaveTextContent(
      `The database was restored from a backup and now opens with the passphrase it had on ${formatDateTime(MADE_AT)}.`,
    );
  });

  it("names, in quotes, the database a directly opened backup was made of", () => {
    renderNotes({
      openedBackup: { backupOfName: "Shared collection", madeAt: MADE_AT, beforeUpgradeTo: null },
    });

    expect(screen.getByRole("region", { name: "A backup" })).toHaveTextContent(
      `This is a backup of “Shared collection” made on ${formatDateTime(MADE_AT)}. Changes here aren't part of “Shared collection”.`,
    );
  });

  // 009 contracts/ui.md §7
  it("says a backup opened directly was made before an upgrade, and may not be removed", () => {
    renderNotes({
      openedBackup: {
        backupOfName: "Shared collection",
        madeAt: MADE_AT,
        beforeUpgradeTo: "0.2.0",
      },
    });

    const note = screen.getByRole("region", { name: "A backup" });
    expect(note).toHaveTextContent(
      `This is a backup of “Shared collection” made on ${formatDateTime(MADE_AT)}, before it was upgraded to HoploDex 0.2.0. Changes here aren't part of “Shared collection”. It's still in the backup folder: move the file elsewhere to keep it.`,
    );
    expect(note).not.toHaveTextContent("may be removed");
  });

  it("still says an ordinary backup opened directly may be removed", () => {
    renderNotes({
      openedBackup: { backupOfName: "Shared collection", madeAt: MADE_AT, beforeUpgradeTo: null },
    });

    const note = screen.getByRole("region", { name: "A backup" });
    expect(note).toHaveTextContent("where it may be removed when older backups are cleared");
    expect(note).not.toHaveTextContent("before it was upgraded");
  });
});

// 009 contracts/ui.md §5
describe("DatabaseNotes after an upgrade (009 contracts/ui.md §5)", () => {
  const BACKUP =
    "/home/sam/Documents/HoploDex/HoploDex backups/Main collection before 0.2.0.hoplodex";

  it("tells once that the database was upgraded, and where the backup from before it is", () => {
    renderNotes({ upgraded: { version: "0.2.0", backupPath: BACKUP, atBackupLocation: true } });

    const note = screen.getByRole("region", { name: "Upgraded" });
    expect(note).toHaveTextContent(
      `“Main collection” was upgraded for HoploDex 0.2.0. A backup of it from before the upgrade is ${BACKUP}. Earlier versions of HoploDex can open that backup, but not the upgraded database.`,
    );
    expect(note).not.toHaveTextContent("isn't your backup location");
  });

  it("adds that a backup in a folder the user chose isn't in the list of backups", () => {
    renderNotes({ upgraded: { version: "0.2.0", backupPath: BACKUP, atBackupLocation: false } });

    expect(screen.getByRole("region", { name: "Upgraded" })).toHaveTextContent(
      `Earlier versions of HoploDex can open that backup, but not the upgraded database. That folder isn't your backup location, so the backup isn't in the database's list of backups.`,
    );
  });

  it("tells a restore that upgraded in its own words, after the restored note", () => {
    renderNotes({
      restoredWithPassphraseOf: MADE_AT,
      upgraded: { version: "0.2.0", backupPath: BACKUP, atBackupLocation: true },
    });

    const regions = screen.getAllByRole("region");
    expect(regions.map((region) => region.getAttribute("aria-label"))).toEqual([
      "Restored from a backup",
      "Upgraded",
    ]);
    expect(regions[1]).toHaveTextContent(
      `The backup was made by an earlier version of HoploDex, so “Main collection” was upgraded for HoploDex 0.2.0 as it was restored. Earlier versions can still open the backup it came from, ${BACKUP}.`,
    );
    expect(regions[1]).not.toHaveTextContent("A backup of it from before the upgrade");
  });

  it("is dismissed as the upgraded note", async () => {
    const user = userEvent.setup();
    const session = renderNotes({
      upgraded: { version: "0.2.0", backupPath: BACKUP, atBackupLocation: true },
    });

    await user.click(
      within(screen.getByRole("region", { name: "Upgraded" })).getByRole("button", {
        name: "Dismiss",
      }),
    );

    expect(session.dismissNote).toHaveBeenCalledExactlyOnceWith("upgraded");
  });

  it("shows nothing when there is nothing to tell", () => {
    renderNotes({});

    expect(screen.queryByRole("region")).not.toBeInTheDocument();
  });
});
