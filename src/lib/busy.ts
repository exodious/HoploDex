/**
 * A count of what the app is still working on, for the end-to-end tests to
 * wait on instead of sleeping: every backend call in flight (`invoke`) and
 * every debounce that has yet to send what it was waiting for (the search
 * box). The count is exposed as `window.__hoplodexBusy`, which is 0 when
 * the app is idle (e2e/support/ui.ts `settle()`).
 *
 * It is on in every build: it is one integer, and a build flag would mean
 * the tests run something other than what ships.
 */

declare global {
  interface Window {
    __hoplodexBusy?: number;
  }
}

let busy = 0;

function publish() {
  if (typeof window !== "undefined") window.__hoplodexBusy = busy;
}

/** Something started that the UI will show the result of. Pair it with one
 * {@link end}; `track` does both around a promise. */
export function begin() {
  busy += 1;
  publish();
}

/** What a {@link begin} started is over (it finished, failed or was
 * abandoned). Never takes the count below 0. */
export function end() {
  busy = Math.max(0, busy - 1);
  publish();
}

/** Marks the app busy until `work` settles, and returns its outcome. */
export async function track<T>(work: () => Promise<T>): Promise<T> {
  begin();
  try {
    return await work();
  } finally {
    end();
  }
}

/** The current count, for tests. */
export function busyCount(): number {
  return busy;
}

publish();
