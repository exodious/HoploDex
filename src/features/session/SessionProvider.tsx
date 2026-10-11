import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import { LockContext, ToastProvider } from "../../components";
import { CommandFailure, setWindowTitle } from "../../services/tauriClient";
import { ClosingScreen } from "./ClosingScreen";
import { CollectionFault, FaultBoundary } from "./FaultScreen";
import { DatabaseChooser } from "../databases/DatabaseChooser";
import * as databasesService from "../databases/databasesService";
import type {
  CreateDatabaseInput,
  DatabaseStatus,
  NoteKind,
  PendingSummary,
} from "../databases/types";
import * as sessionService from "./sessionService";
import { PendingChangesDialog } from "./PendingChangesDialog";
import { createSessionScope, SessionScopeContext } from "./sessionScope";
import type { SessionScope } from "./sessionScope";
import { SessionContext } from "./sessionStore";
import type { SessionState } from "./sessionStore";
import { UnsavedChangesPrompt } from "./UnsavedChangesPrompt";
import { useIdleActivity } from "./useIdleActivity";
import { currentDraft, getDirtyForm } from "./usePendingDraft";

type Phase =
  | { kind: "starting" }
  /** `selectedPath`: the database just closed, which the chooser selects
   * (FR-033). */
  | { kind: "chooser"; selectedPath: string | null }
  /** `opens` counts every open, so reopening the same file still mounts a
   * fresh collection tree. */
  | { kind: "open"; status: DatabaseStatus; opens: number; scope: SessionScope }
  /** A normal close is running (a backup may be under way). */
  | { kind: "closing"; name: string; opens: number };

/** Where resuming the open's pending changes is (specs/008 research.md §6):
 * `opening` from the moment Resume editing is answered until their form
 * reports it opened, `failed` when it reports it can't. `attempt` counts the
 * resumes, so a retry mounts the collection afresh and its pages load again. */
interface Resume {
  state: "idle" | "opening" | "failed";
  attempt: number;
}

const NOT_RESUMING: Resume = { state: "idle", attempt: 0 };

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
 * Quitting, and a normal close, ask first about a form with unsaved input
 * (FR-010, §6); a lock never asks, and keeps it as pending changes, which
 * the next open offers before the collection shows (FR-039, §13). Ctrl/⌘+L
 * locks from anywhere (FR-035), and so does every dialog's lock button; input
 * is reported for the idle lock (FR-034). The window's title names the open
 * database (§4). */
export function SessionProvider({ children }: { children: ReactNode }) {
  const [phase, setPhase] = useState<Phase>({ kind: "starting" });
  const [question, setQuestion] = useState<Question | null>(null);
  const [settingsRequested, setSettingsRequested] = useState(false);
  const [resume, setResume] = useState<Resume>(NOT_RESUMING);
  const openPath = useRef<string | null>(null);
  openPath.current = phase.kind === "open" ? phase.status.path : null;

  // The latest open's scope, ended or not: a closing event is matched against
  // its id even after it ended. Every end is synchronous and made here, in
  // an event handler, never inside a state updater, so nothing of the next
  // session can render or send before the old scope is dead (research.md §3).
  const scopeRef = useRef<SessionScope | null>(null);

  /** Takes the pending changes the open reported out of the status, which
   * lets the dialog go. */
  const clearPending = useCallback((scope: SessionScope) => {
    // The attempt stays: the collection beneath the dialog is the one the
    // form opened in.
    setResume((current) => ({ ...current, state: "idle" }));
    setPhase((current) =>
      current.kind === "open" && current.scope === scope
        ? { ...current, status: { ...current.status, pendingChanges: null } }
        : current,
    );
  }, []);

  /** The dialog goes when the resumed form reports it opened, and comes back
   * with its error when it reports it can't (contracts/ui.md §1). */
  const watchResume = useCallback(
    (scope: SessionScope) => {
      const reportOpened = scope.resumeOpened;
      const reportFailed = scope.resumeFailed;
      scope.resumeOpened = async () => {
        try {
          await reportOpened();
        } finally {
          // Even if the backend couldn't be told, the form is open.
          if (!scope.ended) clearPending(scope);
        }
      };
      scope.resumeFailed = async (reason) => {
        let summary: PendingSummary | null = null;
        try {
          summary = await reportFailed(reason);
        } finally {
          if (!scope.ended) {
            setResume((current) => ({ ...current, state: "failed" }));
            if (summary) {
              const kept = summary;
              setPhase((current) =>
                current.kind === "open" && current.scope === scope
                  ? { ...current, status: { ...current.status, pendingChanges: kept } }
                  : current,
              );
            }
          }
        }
        return summary;
      };
    },
    [clearPending],
  );

  const toChooser = useCallback((selectedPath: string | null) => {
    scopeRef.current?.end();
    setQuestion(null);
    setResume(NOT_RESUMING);
    setPhase({ kind: "chooser", selectedPath });
  }, []);

  const opened = useCallback(
    (status: DatabaseStatus) => {
      scopeRef.current?.end();
      const scope = createSessionScope(status.sessionId);
      scopeRef.current = scope;
      watchResume(scope);
      setResume(NOT_RESUMING);
      setPhase((current) => ({
        kind: "open",
        status,
        opens: current.kind === "open" || current.kind === "closing" ? current.opens + 1 : 1,
        scope,
      }));
    },
    [watchResume],
  );

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
  // shows the backup the close may make (FR-027). The scope ends first, and
  // only for its own session: a late event of an older one, or one with no
  // session id, changes nothing.
  useEffect(
    () =>
      sessionService.onSessionClosing((closing) => {
        const scope = scopeRef.current;
        if (!scope || closing.sessionId !== scope.id) return;
        scope.end();
        setPhase((current) =>
          current.kind === "open" && current.scope === scope
            ? { kind: "closing", name: current.status.name, opens: current.opens }
            : current,
        );
      }),
    [],
  );

  // However the database closed (a close here, a take-over noticed by the
  // backend), everything from it goes, and the chooser selects it.
  useEffect(
    () =>
      sessionService.onSessionClosed((closed) => {
        if (closed.sessionId !== scopeRef.current?.id) return;
        toChooser(closed.databasePath);
      }),
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
    const scope = scopeRef.current;
    if (!scope) return;
    try {
      const locking = sessionService.lockDatabase(scope, currentDraft());
      // The request is on its way with this session's id: nothing more goes
      // out under it (research.md §3).
      scope.end();
      await locking;
    } catch (error) {
      // Closed already, by another lock or a take-over.
      if (!(error instanceof CommandFailure && error.code === "DATABASE_CLOSED")) throw error;
    }
  }, []);

  const isOpen = phase.kind === "open";
  useIdleActivity(isOpen);

  const lockFromDialog = useCallback(() => void lockDatabase(), [lockDatabase]);

  const openName =
    phase.kind === "open" ? phase.status.name : phase.kind === "closing" ? phase.name : null;
  useEffect(() => setWindowTitle(openName ? `${openName} — HoploDex` : "HoploDex"), [openName]);

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
   * the collection mounts beneath the dialog, which stays up until the form
   * they belong to has opened with them (FR-003, research.md §6). */
  const resolvePending = useCallback(
    async (action: "resume" | "discard") => {
      const scope = openScope(scopeRef.current);
      const { draft } = await sessionService.resolvePendingChanges(scope, action);
      if (action === "discard" || !draft) {
        clearPending(scope);
        return;
      }
      scope.resumed = { draft, state: "opening" };
      setResume((current) => ({ state: "opening", attempt: current.attempt + 1 }));
    },
    [clearPending],
  );

  const closeDatabase = useCallback(
    (reason: "closed" | "switched") =>
      unlessUnsaved(async () => {
        const path = openPath.current;
        try {
          await sessionService.closeDatabase(openScope(scopeRef.current), reason);
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
      opened(
        await databasesService.restoreBackup(
          scopeRef.current,
          backupPath,
          passphrase,
          databasePath,
        ),
      );
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
    await sessionService.dismissNote(openScope(scopeRef.current), note);
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
      <LockContext.Provider value={isOpen ? lockFromDialog : null}>
        {phase.kind === "chooser" && <DatabaseChooser selectPath={phase.selectedPath} />}
        {phase.kind === "open" && phase.status.pendingChanges && (
          <PendingChangesDialog
            pending={phase.status.pendingChanges}
            opening={resume.state === "opening"}
            failed={resume.state === "failed"}
            onResume={() => resolvePending("resume")}
            onDiscard={() => resolvePending("discard")}
          />
        )}
        {phase.kind === "open" && (!phase.status.pendingChanges || resume.state !== "idle") && (
          // While the dialog is up (the changes are opening or failed to),
          // the collection mounts beneath it, out of reach.
          <Inert active={phase.status.pendingChanges != null}>
            {/* Every toast of the session goes with the tree keyed by the open,
              and a late `notify` of its closures reaches no provider
              (research.md §4). */}
            <ToastProvider key={`${phase.status.path}#${phase.opens}.${resume.attempt}`}>
              {/* A screen that fails to render leaves the session, and with it
                the way to close the database and to quit, in place. */}
              <FaultBoundary
                scope={phase.scope}
                fallback={
                  <CollectionFault
                    name={phase.status.name}
                    onClose={() => closeDatabase("closed")}
                  />
                }
              >
                <SessionScopeContext.Provider value={phase.scope}>
                  {children}
                </SessionScopeContext.Provider>
              </FaultBoundary>
            </ToastProvider>
          </Inert>
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
      </LockContext.Provider>
    </SessionContext.Provider>
  );
}

/** Keeps `children` out of reach while `active`: no focus, no clicks, nothing
 * for assistive technology. The wrapper takes no part in the layout. */
function Inert({ active, children }: { active: boolean; children: ReactNode }) {
  const element = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    if (element.current) (element.current as HTMLElement & { inert: boolean }).inert = active;
  }, [active]);
  return (
    <div ref={element} style={{ display: "contents" }}>
      {children}
    </div>
  );
}

/** The open session's scope; a command sent with none open fails as any other
 * would, with `DATABASE_CLOSED`. */
function openScope(scope: SessionScope | null): SessionScope {
  if (!scope) {
    throw new CommandFailure({ code: "DATABASE_CLOSED", message: "The database was closed." });
  }
  return scope;
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
