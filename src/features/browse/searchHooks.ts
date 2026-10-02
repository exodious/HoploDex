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
    let pending = true;
    const settle = () => {
      if (pending) end();
      pending = false;
    };
    const timer = window.setTimeout(() => {
      setDebounced(value);
      settle();
    }, delayMs);
    return () => {
      window.clearTimeout(timer);
      settle();
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
