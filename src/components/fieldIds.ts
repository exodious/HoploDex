import { useId } from "react";
import type { ReactNode } from "react";

/** Stable ids tying a control to its label, hint, and error (WCAG 1.3.1 /
 * 3.3.1: the error is announced and programmatically associated). */
export function useFieldIds(id: string | undefined, hint?: ReactNode, error?: string) {
  const generatedId = useId();
  const inputId = id ?? generatedId;
  const hintId = hint ? `${inputId}-hint` : undefined;
  const errorId = error ? `${inputId}-error` : undefined;
  const describedBy = [hintId, errorId].filter(Boolean).join(" ") || undefined;
  return { inputId, hintId, errorId, describedBy };
}
