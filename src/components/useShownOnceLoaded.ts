import { useEffect, useState } from "react";

/** How long a dialog waits for what it shows before it opens anyway. */
export const LOAD_WAIT_MS = 400;

/**
 * Whether a dialog that shows something the backend must read should be
 * shown yet (#116). A dialog is centred, so one that opens and then grows
 * when its data arrives moves, by half the growth, and WebKitGTK only
 * repaints the new size on the next mouse move or key press. So it opens
 * once `loaded`, at the size it will keep, or after `waitMs` if the read is
 * slow, which is the one case it still grows.
 *
 * `open` is whether the screen wants the dialog; `loaded` must be false again
 * whenever `open` is, so the next opening reads afresh.
 */
export function useShownOnceLoaded(open: boolean, loaded: boolean, waitMs = LOAD_WAIT_MS) {
  const [waited, setWaited] = useState(false);
  useEffect(() => {
    if (!open) return;
    const timer = window.setTimeout(() => setWaited(true), waitMs);
    return () => {
      window.clearTimeout(timer);
      setWaited(false);
    };
  }, [open, waitMs]);
  return open && (loaded || waited);
}
