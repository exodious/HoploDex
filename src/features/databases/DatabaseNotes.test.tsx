import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { ToastProvider } from "../../components";
import { formatDateTime } from "../../lib/dates";
import { SessionContext } from "../session/sessionStore";
import type { SessionState } from "../session/sessionStore";
import { DatabaseNotes } from "./DatabaseNotes";
import type { DatabaseStatus } from "./types";

const MADE_AT = "2026-09-20T09:00:00Z";

function renderNotes(notes: Partial<DatabaseStatus["notes"]>) {
  const status: DatabaseStatus = {
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
}

describe("DatabaseNotes (contracts/ui-databases.md §0 naming, §10)", () => {
  it("calls the open database the database, not by its name", () => {
    renderNotes({ restoredWithPassphraseOf: MADE_AT });

    expect(screen.getByRole("region", { name: "Restored from a backup" })).toHaveTextContent(
      `The database was restored from a backup and now opens with the passphrase it had on ${formatDateTime(MADE_AT)}.`,
    );
  });

  it("names, in quotes, the database a directly opened backup was made of", () => {
    renderNotes({ openedBackup: { backupOfName: "Shared collection", madeAt: MADE_AT } });

    expect(screen.getByRole("region", { name: "A backup" })).toHaveTextContent(
      `This is a backup of “Shared collection” made on ${formatDateTime(MADE_AT)}. Changes here aren't part of “Shared collection”.`,
    );
  });
});
