import { useEffect, useRef } from "react";
import type { Draft } from "../databases/types";
import * as sessionService from "./sessionService";

/**
 * The firearm or policy form with unsaved input, for the session layer
 * (specs/003 plan.md): closing, switching or quitting asks save, discard or
 * cancel about it first (FR-010, contracts/ui-databases.md §6), and a lock
 * keeps its input as pending changes instead (FR-039). While it has unsaved
 * input, its draft is mirrored into the backend's memory, so a lock the
 * backend starts on its own (idle, screen lock, sleep, shutdown) has it
 * without asking the webview (research.md §16).
 */
export interface DirtyForm {
  /** What the question names, such as "Glock 19 (edit)" or "New firearm". */
  label: string;
  /** Runs the form's own submit. Resolves `true` once saved, and `false`
   * when it wasn't (a validation error, a refusal), leaving the form showing
   * why. */
  submit: () => Promise<boolean>;
}

/** Which form a draft belongs to: its version, what it edits, and how. */
export interface DraftTarget {
  /** The form's `FORM_VERSION`: a kept draft of another version can only be
   * discarded. */
  formVersion: number;
  kind: Draft["kind"];
  mode: Draft["mode"];
  targetId: number | null;
}

export interface RegisteredForm extends DirtyForm {
  /** Its input differs from what it opened with. */
  isDirty: boolean;
  /** The form and its own state, as a draft keeps them. */
  draft: DraftTarget & { values: unknown };
}

interface Registration {
  current: RegisteredForm;
}

/** Every mounted form, the most recently opened last. */
const registered: Registration[] = [];

/** How long after the last edit a draft is staged (research.md §16). */
export const STAGE_DELAY_MS = 250;

function draftOf({ label, draft }: RegisteredForm): Draft {
  return { ...draft, label };
}

// --- Staging ------------------------------------------------------------------

/** The draft waiting to be staged, and the form it came from. */
let waiting: { from: Registration; draft: Draft | null } | null = null;
let timer: ReturnType<typeof setTimeout> | undefined;
/** What the backend holds, serialized, and the form it came from. */
let staged: { json: string; from: Registration | null } = { json: "null", from: null };

function flush(): void {
  clearTimeout(timer);
  timer = undefined;
  if (!waiting) return;
  const { from, draft } = waiting;
  waiting = null;
  const json = JSON.stringify(draft);
  if (json === staged.json) return;
  staged = { json, from: draft ? from : null };
  // Best-effort: nothing is open any more, or the next edit tries again.
  void Promise.resolve(sessionService.stagePendingChanges(draft)).catch(() => {});
}

function schedule(from: Registration, draft: Draft | null): void {
  waiting = { from, draft };
  clearTimeout(timer);
  timer = setTimeout(flush, STAGE_DELAY_MS);
}

/** Stages at once whatever is waiting: a field lost focus, or the window
 * did. */
export function flushPendingDraft(): void {
  if (waiting) flush();
}

/** Registers the calling form for as long as it is mounted, and mirrors its
 * draft to the backend while it has unsaved input: 250 ms after the last
 * edit, or at once when a field loses focus, and `null` once it is clean or
 * gone. */
export function useDirtyForm(form: RegisteredForm): void {
  const registration = useRef<Registration>({ current: form });
  registration.current.current = form;

  useEffect(() => {
    const entry = registration.current;
    registered.push(entry);
    return () => {
      const index = registered.indexOf(entry);
      if (index >= 0) registered.splice(index, 1);
      // Saved, cancelled or unmounted: its draft is not unsaved input any
      // more.
      if (waiting?.from === entry || staged.from === entry) {
        schedule(entry, null);
        flush();
      }
    };
  }, []);

  const json = form.isDirty ? JSON.stringify(draftOf(form)) : "null";
  useEffect(() => {
    const entry = registration.current;
    // Clean from the start: nothing to stage or clear.
    if (json === "null" && staged.from !== entry && waiting?.from !== entry) return;
    schedule(entry, JSON.parse(json) as Draft | null);
  }, [json]);

  useEffect(listenForBlur, []);
}

/** How many mounted forms want drafts flushed when focus leaves a field. */
let blurListeners = 0;

/** Flushes the waiting draft when a field or the window loses focus, while
 * any form is mounted. */
function listenForBlur(): () => void {
  if (blurListeners++ === 0) {
    window.addEventListener("focusout", flushPendingDraft);
    window.addEventListener("blur", flushPendingDraft);
  }
  return () => {
    if (--blurListeners === 0) {
      window.removeEventListener("focusout", flushPendingDraft);
      window.removeEventListener("blur", flushPendingDraft);
    }
  };
}

function latestDirty(): RegisteredForm | null {
  for (let i = registered.length - 1; i >= 0; i--) {
    if (registered[i].current.isDirty) return registered[i].current;
  }
  return null;
}

/** The most recently opened form with unsaved input, if any. */
export function getDirtyForm(): DirtyForm | null {
  const form = latestDirty();
  return form && { label: form.label, submit: form.submit };
}

/** The unsaved input at this moment, for "lock now" (FR-035), which keeps
 * it exactly as it is rather than as last staged. */
export function currentDraft(): Draft | null {
  const form = latestDirty();
  return form && draftOf(form);
}

// --- Resuming -----------------------------------------------------------------

/** Pending changes the user chose to resume (FR-039), until their form
 * opens with them. */
let resumed: Draft | null = null;

/** Keeps `draft` for its form to open with. */
export function setResumedDraft(draft: Draft | null): void {
  resumed = draft;
}

/** The draft waiting to be resumed, for the page that opens its form. */
export function peekResumedDraft(): Draft | null {
  return resumed;
}

function matches(draft: Draft, target: DraftTarget): boolean {
  return (
    draft.formVersion === target.formVersion &&
    draft.kind === target.kind &&
    draft.mode === target.mode &&
    draft.targetId === target.targetId
  );
}

/** The state a form opening as `target` starts with: `base`, what it would
 * start with anyway, overlaid with the resumed draft's values when the draft
 * belongs to it. A draft's values come from the database, perhaps written
 * on another computer, so only the fields `base` has are taken, and only
 * when they are of the same kind: a draft that is short of fields, or has a
 * field of another shape, can't break the form, whose own values fill the
 * gaps. Read in a state initializer, which may run twice;
 * {@link useResumedDraftTaken} then clears the draft once the form is
 * mounted. */
export function resumedValues<T extends object>(target: DraftTarget, base: T): T {
  if (!resumed || !matches(resumed, target)) return base;
  const values: unknown = resumed.values;
  if (typeof values !== "object" || values === null || Array.isArray(values)) return base;
  const kept = values as Partial<Record<keyof T, unknown>>;
  const merged = { ...base };
  for (const key of Object.keys(base) as (keyof T)[]) {
    const value = kept[key];
    if (value !== undefined && typeof value === typeof base[key]) merged[key] = value as T[keyof T];
  }
  return merged;
}

/** Clears the resumed draft once the form it belongs to has opened with it. */
export function useResumedDraftTaken(target: DraftTarget): void {
  const first = useRef(target);
  useEffect(() => {
    if (resumed && matches(resumed, first.current)) resumed = null;
  }, []);
}
