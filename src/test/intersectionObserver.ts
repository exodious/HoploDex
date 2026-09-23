import { act } from "@testing-library/react";
import { vi } from "vitest";

type Callback = (entries: Partial<IntersectionObserverEntry>[]) => void;
let observed: { callback: Callback; target: Element | null }[] = [];

class FakeIntersectionObserver {
  private entry: { callback: Callback; target: Element | null };
  constructor(callback: Callback) {
    this.entry = { callback, target: null };
    observed.push(this.entry);
  }
  observe(target: Element) {
    this.entry.target = target;
  }
  disconnect() {
    observed = observed.filter((o) => o !== this.entry);
  }
}

/** Replaces IntersectionObserver (jsdom has none) with one the test drives
 * through `scrollAnchorTo`. Undo with `vi.unstubAllGlobals()`. */
export function stubIntersectionObserver() {
  observed = [];
  vi.stubGlobal("IntersectionObserver", FakeIntersectionObserver);
}

/** The elements currently observed, oldest first. */
export function observedTargets(): (Element | null)[] {
  return observed.map((o) => o.target);
}

/** Reports every observed element at `bottom` px from the top of the
 * viewport, with the top bar's lower edge at 56px. */
export function scrollAnchorTo(bottom: number) {
  act(() => {
    for (const { callback } of observed) {
      callback([
        {
          isIntersecting: bottom > 56,
          boundingClientRect: { bottom } as DOMRectReadOnly,
          rootBounds: { top: 56 } as DOMRectReadOnly,
        },
      ]);
    }
  });
}
