import { useId } from "react";
import type { ReactNode } from "react";

/** Stable ids tying a control to its label, hint, and error (WCAG 1.3.1 /
 * 3.3.1: the error is announced and programmatically associated). */
export function useFieldIds(
  id: string | undefined,
  hint?: ReactNode,
  error?: string,
  changed?: boolean,
) {
  const generatedId = useId();
  const inputId = id ?? generatedId;
  const hintId = hint ? `${inputId}-hint` : undefined;
  const errorId = error ? `${inputId}-error` : undefined;
  // The "Changed" tag is read first, then the hint and error (014 FR-006).
  const changedId = changed ? `${inputId}-changed` : undefined;
  const describedBy = [changedId, hintId, errorId].filter(Boolean).join(" ") || undefined;
  return { inputId, hintId, errorId, changedId, describedBy };
}
