import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { scrollAnchorTo, stubIntersectionObserver } from "../../test/intersectionObserver";
import { CollectionContext } from "../app/collectionStore";
import type { CollectionState } from "../app/collectionStore";
import { NavigationContext } from "../app/navigation";
import type { Navigation } from "../app/navigation";
import * as insuranceService from "./insuranceService";
import { PolicyPage } from "./PolicyPage";
import type { InsurancePolicy } from "./types";

vi.mock("./insuranceService");

const policy: InsurancePolicy = {
  id: 7,
  name: "Collector Floater",
  policyNumber: "CF-100",
  insuranceCompany: "Acme Mutual",
  companyContact: null,
  agentName: null,
  agentContact: null,
  notes: null,
  blanketCoverageLimit: null,
  effectiveStartDate: "2026-01-01",
  effectiveEndDate: "2027-01-01",
  createdAt: "2026-01-01 00:00:00",
  updatedAt: "2026-01-01 00:00:00",
  isInForce: true,
  isExpired: false,
  isExpiringSoon: false,
  expiringWarning: false,
  expiredWarning: false,
};

const collection: CollectionState = {
  firearms: [],
  firearmsById: new Map(),
  summary: null,
  policies: [policy],
  policiesById: new Map([[policy.id, policy]]),
  actionTypes: { actions: [], allowedByFirearmType: {} },
  loaded: true,
  error: null,
  revision: 1,
  refresh: async () => {},
};

const navigation: Navigation = {
  route: { page: "policy", id: policy.id },
  navigate: () => {},
  open: () => {},
  back: { label: "Colt Python", go: vi.fn() },
  openDialog: () => {},
};

function renderPage() {
  render(
    <CollectionContext.Provider value={collection}>
      <NavigationContext.Provider value={navigation}>
        <PolicyPage id={policy.id} />
      </NavigationContext.Provider>
    </CollectionContext.Provider>,
  );
}

const strip = () => document.querySelector<HTMLElement>(".hd-runhead");
const headingActions = () => document.querySelector<HTMLElement>(".hd-policy__actions")!;
const stripActions = () => strip()!.querySelector<HTMLElement>(".hd-runhead__actions")!;
const labels = (container: HTMLElement) =>
  within(container)
    .getAllByRole("button")
    .map((b) => b.getAttribute("aria-label") ?? b.textContent?.trim());

/** The title of whichever dialog is open. */
function openDialogTitle() {
  const dialog = screen.queryByRole("dialog") ?? screen.queryByRole("alertdialog");
  const titleId = dialog?.getAttribute("aria-labelledby");
  return titleId ? document.getElementById(titleId)?.textContent : undefined;
}

function scrolledPastHeader() {
  renderPage();
  expect(screen.getByRole("heading", { name: policy.name })).toBeInTheDocument();
  expect(strip()).toBeNull();
  scrollAnchorTo(-400);
  expect(strip()).not.toBeNull();
}

describe("PolicyPage pinned strip (FR-041, US3/AC17)", () => {
  beforeEach(() => {
    stubIntersectionObserver();
    window.scrollTo = vi.fn() as unknown as typeof window.scrollTo;
    vi.mocked(insuranceService.getPolicyDeletionImpact).mockResolvedValue({
      isExpired: false,
      isBlanketInForce: false,
      scheduledFirearmCount: 0,
      scheduledFirearms: [],
      blanketFirearmCount: 0,
      unscheduleOutcome: "uninsured",
      otherPolicies: [],
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("keeps the way back, the name and the heading's actions in reach", () => {
    scrolledPastHeader();

    const back = within(strip()!).getByRole("button", { name: /Colt Python/ });
    expect(back).toHaveAttribute("aria-keyshortcuts", "Escape");
    expect(
      within(strip()!).getByRole("button", { name: /Collector Floater.*CF-100.*back to top/ }),
    ).toBeInTheDocument();
    expect(labels(stripActions())).toEqual(["Edit", "Delete Collector Floater"]);
    expect(labels(stripActions())).toEqual(labels(headingActions()));
  });

  it.each(["Edit", "Delete Collector Floater"])(
    "the strip's %s opens the same dialog as the heading's",
    async (action) => {
      const user = userEvent.setup();
      scrolledPastHeader();

      await user.click(within(headingActions()).getByRole("button", { name: action }));
      const fromHeading = openDialogTitle();
      expect(fromHeading).toBeTruthy();
      await user.keyboard("{Escape}");
      await waitFor(() => expect(openDialogTitle()).toBeUndefined());

      await user.click(within(stripActions()).getByRole("button", { name: action }));
      expect(openDialogTitle()).toBe(fromHeading);
    },
  );
});
