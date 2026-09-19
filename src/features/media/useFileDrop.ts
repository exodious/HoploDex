import { useEffect, useRef, useState } from "react";
import { listenForFileDrops } from "../../services/tauriClient";

/** Whether the user is in the middle of a dialog, where a stray drop
 * shouldn't quietly change the record underneath it. */
function dialogOpen(): boolean {
  return document.querySelector('[role="dialog"], [role="alertdialog"]') != null;
}

/** Accepts files dragged onto the window from the desktop, the ones
 * `accepts` approves of by path. Where in the window they land doesn't
 * matter — a photo panel and a document panel can listen side by side and
 * each takes only its own kind — so `dragging` is true while any acceptable
 * file is being dragged over the window, for the caller to show a drop
 * target. */
export function useFileDrop(accepts: (path: string) => boolean, onDrop: (paths: string[]) => void) {
  const [dragging, setDragging] = useState(false);
  const latest = useRef({ accepts, onDrop });
  latest.current = { accepts, onDrop };

  useEffect(
    () =>
      listenForFileDrops((event) => {
        if (event.type === "leave") {
          setDragging(false);
          return;
        }
        const wanted = event.paths.filter(latest.current.accepts);
        if (event.type === "enter") {
          setDragging(wanted.length > 0 && !dialogOpen());
          return;
        }
        setDragging(false);
        if (wanted.length > 0 && !dialogOpen()) latest.current.onDrop(wanted);
      }),
    [],
  );

  return dragging;
}
