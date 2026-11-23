import { useEffect, useState } from "react";
import type { RefObject } from "react";
import { begin, end } from "../../lib/busy";

/** How long typing pauses before a search is sent. */
export const SEARCH_DEBOUNCE_MS = 150;

/** `value`, once it has stopped changing for `delayMs`. The app counts as
 * busy (`lib/busy.ts`) from the change until the new value is handed on, so
 * a test can wait for the search it started. */
export function useDebounced<T>(value: T, delayMs: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    if (Object.is(value, debounced)) return;
    begin();
    const timer = window.setTimeout(() => setDebounced(value), delayMs);
    // Ends only once the new value is rendered: this cleanup runs in the same
    // effect flush as the consumer's effect that starts the search, so no frame
    // sees the app idle in between.
    return () => {
      window.clearTimeout(timer);
      end();
    };
  }, [value, debounced, delayMs]);
  return debounced;
}

/** "/" (outside a text field) or Ctrl/⌘+F jumps to the search box, unless a
 * dialog is open. */
export function useSearchShortcut(searchRef: RefObject<HTMLInputElement | null>) {
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      const target = event.target as HTMLElement;
      const typing = target.closest("input, textarea, select, [contenteditable]");
      const find = (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f";
      if ((event.key === "/" && !typing) || find) {
        if (document.querySelector('[role="dialog"], [role="alertdialog"]')) return;
        event.preventDefault();
        searchRef.current?.focus();
        searchRef.current?.select();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [searchRef]);
}
