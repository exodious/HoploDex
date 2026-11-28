import { createContext, useContext, useEffect, useRef } from "react";

/** What `Dialog` offers the form inside it (014 research.md §4). */
export interface CloseGuardContextValue {
  /** Registers whether the form has unsaved changes, read at each close. */
  register: (isDirty: () => boolean) => () => void;
  /** Closes with `close`, after "Discard changes?" when the form is dirty. */
  request: (close: () => void) => void;
}

export const CloseGuardContext = createContext<CloseGuardContextValue | null>(null);

/** Makes Escape, the dialog's close button and the returned `requestClose`
 * (for the form's Cancel) ask "Discard changes?" while `isDirty`, and close at
 * once otherwise (014 FR-021a). Saving, the quit question and a lock close the
 * form without it. Outside a `Dialog`, `requestClose` just closes. */
export function useCloseGuard(isDirty: boolean, close: () => void): () => void {
  const context = useContext(CloseGuardContext);
  const dirty = useRef(isDirty);
  dirty.current = isDirty;
  const latestClose = useRef(close);
  latestClose.current = close;
  useEffect(() => context?.register(() => dirty.current), [context]);
  return () => {
    if (context) context.request(() => latestClose.current());
    else latestClose.current();
  };
}
