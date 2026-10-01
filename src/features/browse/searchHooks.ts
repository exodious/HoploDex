import { useEffect, useState } from "react";
import type { RefObject } from "react";

/** How long typing pauses before a search is sent. */
export const SEARCH_DEBOUNCE_MS = 150;

/** `value`, once it has stopped changing for `delayMs`. */
export function useDebounced<T>(value: T, delayMs: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const timer = window.setTimeout(() => setDebounced(value), delayMs);
    return () => window.clearTimeout(timer);
  }, [value, delayMs]);
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
