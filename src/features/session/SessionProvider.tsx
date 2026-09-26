import { Fragment, useCallback, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { DatabaseChooser } from "../databases/DatabaseChooser";
import * as databasesService from "../databases/databasesService";
import type { CreateDatabaseInput, DatabaseStatus, NoteKind } from "../databases/types";
import * as sessionService from "./sessionService";
import { SessionContext } from "./sessionStore";
import type { SessionState } from "./sessionStore";

type Phase =
  | { kind: "starting" }
  | { kind: "chooser" }
  /** `opens` counts every open, so reopening the same file still mounts a
   * fresh collection tree. */
  | { kind: "open"; status: DatabaseStatus; opens: number };

/** Holds the session and shows the chooser whenever no database is open.
 * `children`, the collection's providers and shell, render only while one
 * is, keyed by that open, so a close or switch drops every piece of
 * collection state with the tree that held it (contracts/ui-databases.md §3). */
export function SessionProvider({ children }: { children: ReactNode }) {
  const [phase, setPhase] = useState<Phase>({ kind: "starting" });

  const opened = useCallback((status: DatabaseStatus) => {
    setPhase((current) => ({
      kind: "open",
      status,
      opens: current.kind === "open" ? current.opens + 1 : 1,
    }));
  }, []);

  // A reloaded window finds its database still open in the backend.
  useEffect(() => {
    let current = true;
    sessionService.getDatabaseStatus().then(
      (status) => current && opened(status),
      () => current && setPhase({ kind: "chooser" }),
    );
    return () => {
      current = false;
    };
  }, [opened]);

  const openDatabase = useCallback(
    async (path: string, passphrase: string) => {
      opened(await databasesService.openDatabase(path, passphrase));
    },
    [opened],
  );

  const createDatabase = useCallback(
    async (input: CreateDatabaseInput) => {
      opened(await databasesService.createDatabase(input));
    },
    [opened],
  );

  const refreshStatus = useCallback(async () => {
    const status = await sessionService.getDatabaseStatus();
    setPhase((current) => (current.kind === "open" ? { ...current, status } : current));
  }, []);

  const dismissNote = useCallback(async (note: NoteKind) => {
    await sessionService.dismissNote(note);
    setPhase((current) =>
      current.kind === "open"
        ? {
            ...current,
            status: { ...current.status, notes: withoutNote(current.status.notes, note) },
          }
        : current,
    );
  }, []);

  const value = useMemo<SessionState>(
    () => ({
      status: phase.kind === "open" ? phase.status : null,
      openDatabase,
      createDatabase,
      refreshStatus,
      dismissNote,
    }),
    [phase, openDatabase, createDatabase, refreshStatus, dismissNote],
  );

  return (
    <SessionContext.Provider value={value}>
      {phase.kind === "chooser" && <DatabaseChooser />}
      {phase.kind === "open" && (
        <Fragment key={`${phase.status.path}#${phase.opens}`}>{children}</Fragment>
      )}
    </SessionContext.Provider>
  );
}

function withoutNote(notes: DatabaseStatus["notes"], note: NoteKind): DatabaseStatus["notes"] {
  switch (note) {
    case "diskEncryption":
      return { ...notes, diskEncryption: false };
    case "openedBackup":
      return { ...notes, openedBackup: null };
    case "restored":
      return { ...notes, restoredWithPassphraseOf: null, damagedFileKeptAt: null };
  }
}
