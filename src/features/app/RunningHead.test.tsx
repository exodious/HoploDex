import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import {
  observedTargets,
  scrollAnchorTo,
  stubIntersectionObserver,
} from "../../test/intersectionObserver";
import { NavigationContext } from "./navigation";
import type { Navigation } from "./navigation";
import { RunningHead } from "./RunningHead";

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
    goBack.mockReset();
    stubIntersectionObserver();
    window.scrollTo = vi.fn() as unknown as typeof window.scrollTo;
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("stays out of the way while the page's heading is in view", () => {
    renderPage();
    expect(observedTargets()[0]?.tagName).toBe("HEADER");
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
