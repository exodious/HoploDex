import { useEffect, useRef } from "react";

/**
 * The firearm or policy form with unsaved input, for the session layer
 * (specs/003 plan.md): closing, switching or quitting asks save, discard or
 * cancel about it first (FR-010, contracts/ui-databases.md §6). The staging
 * of its draft for a lock (research.md §16) comes with User Story 6.
 */
export interface DirtyForm {
  /** What the question names, such as "Glock 19 (edit)" or "New firearm". */
  label: string;
  /** Runs the form's own submit. Resolves `true` once saved, and `false`
   * when it wasn't (a validation error, a refusal), leaving the form showing
   * why. */
  submit: () => Promise<boolean>;
}

interface Registration {
  current: DirtyForm & { isDirty: boolean };
}

/** Every mounted form, the most recently opened last. */
const registered: Registration[] = [];

/** Registers the calling form for as long as it is mounted. `isDirty` is
 * whether its input differs from what it opened with. */
export function useDirtyForm(form: DirtyForm & { isDirty: boolean }): void {
  const registration = useRef<Registration>({ current: form });
  registration.current.current = form;

  useEffect(() => {
    const entry = registration.current;
    registered.push(entry);
    return () => {
      const index = registered.indexOf(entry);
      if (index >= 0) registered.splice(index, 1);
    };
  }, []);
}

/** The most recently opened form with unsaved input, if any. */
export function getDirtyForm(): DirtyForm | null {
  for (let i = registered.length - 1; i >= 0; i--) {
    const { label, isDirty, submit } = registered[i].current;
    if (isDirty) return { label, submit };
  }
  return null;
}
