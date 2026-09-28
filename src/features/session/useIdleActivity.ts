import { useEffect } from "react";
import * as sessionService from "./sessionService";

/** Input that counts for the idle lock: keyboard, pointer and touch to this
 * window (FR-034). */
const ACTIVITY_EVENTS = ["keydown", "pointerdown", "pointermove", "wheel", "touchstart"] as const;

/** At most one `note_activity` a second (research.md §15). */
const THROTTLE_MS = 1000;

/** The idle clock is best-effort from here: a report that fails (nothing is
 * open any more) changes nothing. */
async function quietly(call: () => Promise<void>): Promise<void> {
  try {
    await call();
  } catch {
    // Nothing to do: the next report or pause will try again.
  }
}

/** Reports input to the backend's idle clock while `active`, at most once a
 * second, on the leading and trailing edge, so the last input is always
 * reported within a second (research.md §15). */
export function useIdleActivity(active: boolean): void {
  useEffect(() => {
    if (!active) return;
    let last = -Infinity;
    let trailing: ReturnType<typeof setTimeout> | undefined;

    function report() {
      last = Date.now();
      void quietly(sessionService.noteActivity);
    }

    function onActivity() {
      const wait = last + THROTTLE_MS - Date.now();
      if (wait <= 0) {
        clearTimeout(trailing);
        trailing = undefined;
        report();
      } else if (trailing === undefined) {
        trailing = setTimeout(() => {
          trailing = undefined;
          report();
        }, wait);
      }
    }

    for (const event of ACTIVITY_EVENTS) {
      window.addEventListener(event, onActivity, { capture: true, passive: true });
    }
    return () => {
      clearTimeout(trailing);
      for (const event of ACTIVITY_EVENTS) {
        window.removeEventListener(event, onActivity, { capture: true });
      }
    };
  }, [active]);
}

/** Runs `open`, a native file or folder dialog, with the idle clock paused:
 * the user gives the window no input while choosing (research.md §15). The
 * clock resumes, starting the idle time again, however `open` ends. */
export async function withIdlePaused<T>(open: () => Promise<T>): Promise<T> {
  await quietly(() => sessionService.setIdlePaused(true));
  try {
    return await open();
  } finally {
    void quietly(() => sessionService.setIdlePaused(false));
  }
}

/** Pauses the idle clock while `input`, an `<input type="file">`, has the
 * system's file chooser open. The chooser gives the page no promise, so the
 * pause starts on the input's `click` and ends on its `change` or `cancel`,
 * or, where neither fires, the window's next `focus` (research.md §15).
 * Returns the function that detaches it. */
export function pauseIdleForFileInput(input: HTMLInputElement): () => void {
  let paused = false;

  function resume() {
    if (!paused) return;
    paused = false;
    window.removeEventListener("focus", resume);
    void quietly(() => sessionService.setIdlePaused(false));
  }

  function onClick() {
    if (paused) return;
    paused = true;
    void quietly(() => sessionService.setIdlePaused(true));
    window.addEventListener("focus", resume);
  }

  input.addEventListener("click", onClick);
  input.addEventListener("change", resume);
  input.addEventListener("cancel", resume);
  return () => {
    input.removeEventListener("click", onClick);
    input.removeEventListener("change", resume);
    input.removeEventListener("cancel", resume);
    window.removeEventListener("focus", resume);
  };
}
