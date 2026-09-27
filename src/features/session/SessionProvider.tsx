import { Fragment, useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import { CommandFailure } from "../../services/tauriClient";
import { ClosingScreen } from "./ClosingScreen";
import { DatabaseChooser } from "../databases/DatabaseChooser";
import * as databasesService from "../databases/databasesService";
import type { CreateDatabaseInput, DatabaseStatus, NoteKind } from "../databases/types";
import * as sessionService from "./sessionService";
import { PendingChangesDialog } from "./PendingChangesDialog";
import { SessionContext } from "./sessionStore";
import type { SessionState } from "./sessionStore";
import { UnsavedChangesPrompt } from "./UnsavedChangesPrompt";
import { useIdleActivity } from "./useIdleActivity";
import { currentDraft, getDirtyForm, setResumedDraft } from "./usePendingDraft";

type Phase =
  | { kind: "starting" }
  /** `selectedPath`: the database just closed, which the chooser selects
   * (FR-033). */
  | { kind: "chooser"; selectedPath: string | null }
  /** `opens` counts every open, so reopening the same file still mounts a
   * fresh collection tree. */
  | { kind: "open"; status: DatabaseStatus; opens: number }
  /** A normal close is running (a backup may be under way). */
  | { kind: "closing"; name: string; opens: number };

/** The save / discard / cancel question, and what happens once it is
 * answered with anything but cancel. */
interface Question {
  label: string;
  save: () => Promise<boolean>;
  proceed: () => Promise<void>;
}

/** Holds the session and shows the chooser whenever no database is open.
 * `children`, the collection's providers and shell, render only while one
 * is, keyed by that open, so a close or switch drops every piece of
 * collection state with the tree that held it (contracts/ui-databases.md §3).
 * Closing, switching and quitting ask first about a form with unsaved input
 * (FR-010, §6); a lock never asks, and keeps it as pending changes, which
 * the next open offers before the collection shows (FR-039, §13). Ctrl/⌘+L
 * locks from anywhere (FR-035), and input is reported for the idle lock
 * (FR-034). */
export function SessionProvider({ children }: { children: ReactNode }) {
  const [phase, setPhase] = useState<Phase>({ kind: "starting" });
  const [question, setQuestion] = useState<Question | null>(null);
  const [settingsRequested, setSettingsRequested] = useState(false);
  const openPath = useRef<string | null>(null);
  openPath.current = phase.kind === "open" ? phase.status.path : null;

  const toChooser = useCallback((selectedPath: string | null) => {
    setQuestion(null);
    setPhase({ kind: "chooser", selectedPath });
  }, []);

  const opened = useCallback((status: DatabaseStatus) => {
    setPhase((current) => ({
      kind: "open",
      status,
      opens: current.kind === "open" || current.kind === "closing" ? current.opens + 1 : 1,
    }));
  }, []);

  // A reloaded window finds its database still open in the backend.
  useEffect(() => {
    let current = true;
    sessionService.getDatabaseStatus().then(
      (status) => current && opened(status),
      () => current && toChooser(null),
    );
    return () => {
      current = false;
    };
  }, [opened, toChooser]);

  // A normal close replaces the collection with the closing screen, which
  // shows the backup the close may make (FR-027).
  useEffect(
    () =>
      sessionService.onSessionClosing(() =>
        setPhase((current) =>
          current.kind === "open"
            ? { kind: "closing", name: current.status.name, opens: current.opens }
            : current,
        ),
      ),
    [],
  );

  // However the database closed (a close here, a take-over noticed by the
  // backend), everything from it goes, and the chooser selects it.
  useEffect(
    () => sessionService.onSessionClosed((closed) => toChooser(closed.databasePath)),
    [toChooser],
  );

  /** Runs `proceed` at once, or once the user has chosen to save or discard
   * a form's unsaved input. */
  const unlessUnsaved = useCallback((proceed: () => Promise<void>) => {
    const form = getDirtyForm();
    if (!form) return proceed();
    setQuestion({ label: form.label, save: form.submit, proceed });
    return Promise.resolve();
  }, []);

  /** "Lock now": at once, with no question, keeping unsaved input exactly
   * as it is now (FR-033, FR-035). The backend's `session:closing` and
   * `session:closed` then take the collection away. */
  const lockDatabase = useCallback(async () => {
    try {
      await sessionService.lockDatabase(currentDraft());
    } catch (error) {
      // Closed already, by another lock or a take-over.
      if (!(error instanceof CommandFailure && error.code === "DATABASE_CLOSED")) throw error;
    }
  }, []);

  const isOpen = phase.kind === "open";
  useIdleActivity(isOpen);

  // Ctrl/⌘+L locks from any focus, inside dialogs too: listened for before
  // anything else sees the key.
  useEffect(() => {
    if (!isOpen) return;
    function onKeyDown(event: KeyboardEvent) {
      if (!(event.ctrlKey || event.metaKey) || event.altKey || event.shiftKey) return;
      if (event.key.toLowerCase() !== "l") return;
      event.preventDefault();
      event.stopPropagation();
      void lockDatabase();
    }
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [isOpen, lockDatabase]);

  /** Resumes or discards the pending changes the open reported. On resume,
   * the form they belong to opens with them once the collection shows. */
  const resolvePending = useCallback(async (action: "resume" | "discard") => {
    const { draft } = await sessionService.resolvePendingChanges(action);
    setResumedDraft(action === "resume" ? (draft ?? null) : null);
    setPhase((current) =>
      current.kind === "open"
        ? { ...current, status: { ...current.status, pendingChanges: null } }
        : current,
    );
  }, []);

  const closeDatabase = useCallback(
    (reason: "closed" | "switched") =>
      unlessUnsaved(async () => {
        const path = openPath.current;
        try {
          await sessionService.closeDatabase(reason);
        } catch (error) {
          // Already closed, by a lock or a take-over: the chooser it is.
          if (!(error instanceof CommandFailure && error.code === "DATABASE_CLOSED")) throw error;
        }
        toChooser(path);
      }),
    [unlessUnsaved, toChooser],
  );

  // The window's close button, or quitting from the OS (research.md §17).
  useEffect(
    () =>
      sessionService.onQuitRequested(
        () => void unlessUnsaved(() => sessionService.quitApplication()),
      ),
    [unlessUnsaved],
  );

  const openDatabase = useCallback(
    async (...open: Parameters<typeof databasesService.openDatabase>) => {
      opened(await databasesService.openDatabase(...open));
    },
    [opened],
  );

  const createDatabase = useCallback(
    async (input: CreateDatabaseInput) => {
      opened(await databasesService.createDatabase(input));
    },
    [opened],
  );

  const restoreBackup = useCallback(
    async (backupPath: string, passphrase: string, databasePath?: string) => {
      // The restored database is new to the collection views: they remount.
      opened(await databasesService.restoreBackup(backupPath, passphrase, databasePath));
    },
    [opened],
  );

  const requestSettings = useCallback(() => setSettingsRequested(true), []);
  const clearSettingsRequest = useCallback(() => setSettingsRequested(false), []);

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
      closeDatabase,
      lockDatabase,
      refreshStatus,
      dismissNote,
      restoreBackup,
      settingsRequested,
      requestSettings,
      clearSettingsRequest,
    }),
    [
      phase,
      openDatabase,
      createDatabase,
      closeDatabase,
      lockDatabase,
      refreshStatus,
      dismissNote,
      restoreBackup,
      settingsRequested,
      requestSettings,
      clearSettingsRequest,
    ],
  );

  return (
    <SessionContext.Provider value={value}>
      {phase.kind === "chooser" && <DatabaseChooser selectPath={phase.selectedPath} />}
      {phase.kind === "open" && phase.status.pendingChanges && (
        <PendingChangesDialog
          name={phase.status.name}
          pending={phase.status.pendingChanges}
          onResume={() => resolvePending("resume")}
          onDiscard={() => resolvePending("discard")}
        />
      )}
      {phase.kind === "open" && !phase.status.pendingChanges && (
        <Fragment key={`${phase.status.path}#${phase.opens}`}>{children}</Fragment>
      )}
      {phase.kind === "closing" && <ClosingScreen name={phase.name} />}
      <UnsavedChangesPrompt
        label={question?.label ?? null}
        onSave={async () => {
          if (!question) return;
          // A save that fails leaves the form open showing why, and nothing
          // closes.
          if (await question.save()) await question.proceed();
        }}
        onDiscard={() => void question?.proceed()}
        onCancel={() => setQuestion(null)}
      />
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
