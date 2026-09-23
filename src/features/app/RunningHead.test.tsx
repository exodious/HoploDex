import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { NavigationContext } from "./navigation";
import type { Navigation } from "./navigation";
import { RunningHead } from "./RunningHead";

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

/** Reports the anchor at `bottom` px from the top of the viewport, with the
 * top bar's lower edge at 56px. */
function scrollAnchorTo(bottom: number) {
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

const goBack = vi.fn();
const navigation: Navigation = {
  route: { page: "firearm", id: 1, from: "collection" },
  navigate: () => {},
  open: () => {},
  back: { label: "Collection", go: goBack },
  openDialog: () => {},
};

function Page() {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  return (
    <>
      <header ref={setAnchor}>
        <h1 id="page-heading" tabIndex={-1}>
          Colt Python
        </h1>
      </header>
      <RunningHead
        anchor={anchor}
        headingId="page-heading"
        title="Colt Python"
        stamp="V1"
        actions={<button type="button">Edit</button>}
      />
    </>
  );
}

function renderPage() {
  render(
    <NavigationContext.Provider value={navigation}>
      <Page />
    </NavigationContext.Provider>,
  );
}

describe("RunningHead", () => {
  beforeEach(() => {
    observed = [];
    goBack.mockReset();
    vi.stubGlobal("IntersectionObserver", FakeIntersectionObserver);
    window.scrollTo = vi.fn() as unknown as typeof window.scrollTo;
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("stays out of the way while the page's heading is in view", () => {
    renderPage();
    expect(observed[0].target?.tagName).toBe("HEADER");
    scrollAnchorTo(400);
    expect(screen.queryByRole("button", { name: /back to top/ })).not.toBeInTheDocument();
  });

  it("pins the way back, the name and the actions once the heading scrolls away", async () => {
    const user = userEvent.setup();
    renderPage();
    scrollAnchorTo(-200);

    expect(screen.getByRole("button", { name: /Colt Python.*V1.*back to top/ })).toBeVisible();
    expect(screen.getByRole("button", { name: "Edit" })).toBeVisible();
    const back = screen.getByRole("button", { name: /Collection/ });
    expect(back).toHaveAttribute("aria-keyshortcuts", "Escape");
    await user.click(back);
    expect(goBack).toHaveBeenCalled();

    scrollAnchorTo(300);
    expect(screen.queryByRole("button", { name: /back to top/ })).not.toBeInTheDocument();
  });

  it("returns to the top and puts focus on the page's heading", async () => {
    const user = userEvent.setup();
    renderPage();
    scrollAnchorTo(-200);

    await user.click(screen.getByRole("button", { name: /back to top/ }));
    expect(window.scrollTo).toHaveBeenCalledWith(expect.objectContaining({ top: 0 }));
    expect(screen.getByRole("heading", { name: "Colt Python" })).toHaveFocus();
  });
});
