import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { scrollAnchorTo, stubIntersectionObserver } from "../../test/intersectionObserver";
import { CollectionContext } from "../app/collectionStore";
import type { CollectionState } from "../app/collectionStore";
import {
  FIREARM_TYPES,
  REGISTRATION_CLASSES,
  ACCESSORY_KINDS,
} from "../../test/collectionFixtures";
import { CollectionPage } from "./CollectionPage";
import type { BrowseState } from "./types";
import type { InsurancePolicy } from "../insurance/types";
import { toIso } from "../../lib/dates";
import { render } from "../../test/sessionScope";

const listFirearms = vi.fn();

vi.mock("./browseService", () => ({
  listFirearms: (_scope: unknown, input: unknown) => listFirearms(input),
}));
// Each render of a row's thumbnail is counted: it renders once for each time
// its row does.
const thumbnailRendered = vi.hoisted(() => vi.fn());
vi.mock("./FirearmThumbnail", () => ({
  FirearmThumbnail: () => {
    thumbnailRendered();
    return null;
  },
}));

const summary = {
  id: 1,
  make: "Colt",
  model: "Python",
  nickname: null,
  serialNumber: "V1",
  caliber: ".357",
  cartridge: null,
  firearmTypeName: "Handgun",
  actionTypeName: null,
  registeredAs: null,
  status: "active" as const,
  thumbnailPhotoId: null,
  genericThumbnailKey: "handgun",
  estimatedValue: 1250,
  insuranceWarning: "none" as const,
  insurancePolicyId: null,
  scheduledCoverageAmount: null,
  mountedOn: null,
  mountedCounts: { firearms: 0, accessories: 0 },
};

const collection: CollectionState = {
  firearms: [summary],
  firearmsById: new Map([[summary.id, summary]]),
  accessories: [],
  accessoriesById: new Map(),
  summary: null,
  policies: [],
  policiesById: new Map(),
  actionTypes: { actions: [], allowedByFirearmType: {} },
  actionTypesFailed: false,
  firearmTypes: { types: FIREARM_TYPES },
  firearmTypesFailed: false,
  accessoryKinds: { kinds: ACCESSORY_KINDS },
  accessoryKindsFailed: false,
  registrationClasses: { classes: REGISTRATION_CLASSES },
  registrationClassesFailed: false,
  loaded: true,
  error: null,
  revision: 1,
  refresh: async () => {},
};

function renderPage(
  groupBy: BrowseState["groupBy"] = undefined,
  state: CollectionState = collection,
) {
  const changes: BrowseState[] = [];
  const browse: BrowseState = { query: "", groupBy, includeDisposed: false, view: "list" };
  render(
    <CollectionContext.Provider value={state}>
      <CollectionPage browse={browse} onBrowseChange={(next) => changes.push(next)} />
    </CollectionContext.Provider>,
  );
  return changes;
}

describe("the Group by menu (specs/005 contracts/ui-registration.md §6, FR-016)", () => {
  beforeEach(() => {
    listFirearms.mockReset().mockResolvedValue({ groups: [{ key: "All", firearms: [summary] }] });
  });

  it("reads Group by None when ungrouped and names the grouping when grouped", async () => {
    renderPage();
    const trigger = await screen.findByRole("button", { name: "Group by, None" });
    expect(trigger).toHaveAttribute("aria-haspopup", "menu");
    expect(trigger).toHaveTextContent("Group byNone");
  });

  // 004 US1-6, 005 US2-9: the trigger names the grouping
  it("shows the chosen grouping on the trigger", async () => {
    renderPage("caliber");
    const trigger = await screen.findByRole("button", { name: "Group by, Caliber" });
    expect(trigger).toHaveTextContent("Group byCaliber");
  });

  // 004 US1-6 (Cartridge), 005 US2-9 (Registered as)
  it("offers None, then three headed sections of radio items", async () => {
    const user = userEvent.setup();
    renderPage("caliber");
    await user.click(await screen.findByRole("button", { name: "Group by, Caliber" }));

    const menu = await screen.findByRole("menu");
    const labels = within(menu)
      .getAllByRole("menuitemradio")
      .map((item) => item.textContent);
    expect(labels).toEqual([
      "None",
      "Type",
      "Action",
      "Caliber",
      "Cartridge",
      "Make",
      "Origin",
      "Registered as",
      "Registered to",
    ]);
    expect(within(menu).getByRole("group", { name: "The firearm" })).toHaveTextContent(
      /TypeActionCaliberCartridge/,
    );
    expect(within(menu).getByRole("group", { name: "Its maker" })).toHaveTextContent(/MakeOrigin/);
    expect(within(menu).getByRole("group", { name: "Registration" })).toHaveTextContent(
      /Registered asRegistered to/,
    );
    const checked = within(menu)
      .getAllByRole("menuitemradio")
      .filter((item) => item.getAttribute("aria-checked") === "true");
    expect(checked.map((item) => item.textContent)).toEqual(["Caliber"]);
  });

  // 001 US2-2: the page regroups by the chosen item, and None ungroups.
  // 004 US1-6, 005 US2-9: choosing Cartridge or Registered as regroups
  it("regroups by the chosen item", async () => {
    const user = userEvent.setup();
    const changes = renderPage("caliber");
    await user.click(await screen.findByRole("button", { name: "Group by, Caliber" }));
    await user.click(await screen.findByRole("menuitemradio", { name: "Registered as" }));

    expect(changes).toHaveLength(1);
    expect(changes[0].groupBy).toBe("registered_as");
    await waitFor(() => expect(screen.queryByRole("menu")).not.toBeInTheDocument());
  });

  it("chooses None to ungroup", async () => {
    const user = userEvent.setup();
    const changes = renderPage("make");
    await user.click(await screen.findByRole("button", { name: "Group by, Make" }));
    await user.click(await screen.findByRole("menuitemradio", { name: "None" }));
    expect(changes[0].groupBy).toBeUndefined();
  });

  it("changes nothing on Escape", async () => {
    const user = userEvent.setup();
    const changes = renderPage("caliber");
    const trigger = await screen.findByRole("button", { name: "Group by, Caliber" });
    await user.click(trigger);
    await screen.findByRole("menu");
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).not.toBeInTheDocument());
    expect(changes).toEqual([]);
    expect(trigger).toHaveFocus();
  });

  it("keeps List and Tiles as a segmented control", async () => {
    renderPage();
    const view = await screen.findByRole("radiogroup", { name: "View" });
    expect(within(view).getAllByRole("radio")).toHaveLength(2);
  });
});

// 001 US3-7 and US3-14: the collection's attention banner names a policy
// the backend flags as expiring, and leaves out one whose warning is
// suppressed because another blanket policy takes over.
describe("the attention banner's expiring policies (001 FR-028)", () => {
  beforeEach(() => {
    listFirearms.mockReset().mockResolvedValue({ groups: [{ key: "All", firearms: [summary] }] });
  });

  const policyEndingIn = (id: number, name: string, days: number): InsurancePolicy => {
    const end = new Date();
    end.setDate(end.getDate() + days);
    return {
      id,
      name,
      policyNumber: `P-${id}`,
      insuranceCompany: "Acme",
      companyContact: null,
      agentName: null,
      agentContact: null,
      notes: null,
      blanketCoverageLimit: 10_000,
      effectiveStartDate: "2020-01-01",
      effectiveEndDate: toIso(end.getFullYear(), end.getMonth() + 1, end.getDate()),
      createdAt: "",
      updatedAt: "",
      isInForce: true,
      isExpired: false,
      isExpiringSoon: true,
      expiringWarning: true,
      expiredWarning: false,
    };
  };

  it("names a policy expiring within 30 days and leaves out one a successor takes over from", async () => {
    const expiring = policyEndingIn(1, "Homeowner's blanket", 10);
    const renewed = { ...policyEndingIn(2, "Old floater", 12), expiringWarning: false };
    renderPage(undefined, { ...collection, policies: [expiring, renewed] });

    await screen.findByRole("button", { name: "Review insurance" });
    const banner = document.querySelector<HTMLElement>(".hd-attention")!;
    expect(banner).toHaveTextContent("“Homeowner's blanket” expires in 10 days.");
    expect(banner).not.toHaveTextContent("Old floater");
  });

  it("shows no banner when the only expiring policy has a successor", async () => {
    const renewed = { ...policyEndingIn(2, "Old floater", 12), expiringWarning: false };
    renderPage(undefined, { ...collection, policies: [renewed] });

    await screen.findByRole("button", { name: "Group by, None" });
    expect(document.querySelector(".hd-attention")).toBeNull();
  });
});

// Issue #51, constitution IV: with thousands of firearms the first rows mount
// and the rest as the end of the list nears. Each batch used to render every
// row already mounted (about 400 ms at 5,000 to 10,000 rows), and a regrouping
// with every row mounted drew them all again under the new columns.
describe("with thousands of firearms", () => {
  beforeEach(() => stubIntersectionObserver());
  afterEach(() => vi.unstubAllGlobals());

  const many = Array.from({ length: 400 }, (_, i) => ({
    ...summary,
    id: 100 + i,
    model: `Model ${i}`,
  }));
  const rowCount = () => screen.getAllByRole("row").length - 1; // less the heading row

  function renderMany() {
    listFirearms.mockReset().mockResolvedValue({ groups: [{ key: "All", firearms: many }] });
    let regroup: (groupBy: BrowseState["groupBy"]) => void = () => {};
    function Harness() {
      const [browse, setBrowse] = useState<BrowseState>({
        query: "",
        groupBy: undefined,
        includeDisposed: false,
        view: "list",
      });
      regroup = (groupBy) => setBrowse((current) => ({ ...current, groupBy }));
      return (
        <CollectionContext.Provider value={{ ...collection, firearms: many }}>
          <CollectionPage browse={browse} onBrowseChange={setBrowse} />
        </CollectionContext.Provider>
      );
    }
    render(<Harness />);
    return (groupBy: BrowseState["groupBy"]) => act(() => regroup(groupBy));
  }

  it("renders only the rows of the next batch, not the ones already mounted", async () => {
    thumbnailRendered.mockClear();
    renderMany();
    await screen.findByText("Colt Model 0");
    expect(rowCount()).toBe(150);
    expect(thumbnailRendered).toHaveBeenCalledTimes(150);
    thumbnailRendered.mockClear();

    scrollAnchorTo(500);
    await waitFor(() => expect(rowCount()).toBe(300));

    expect(thumbnailRendered).toHaveBeenCalledTimes(150);
    thumbnailRendered.mockClear();
    scrollAnchorTo(500);
    await waitFor(() => expect(rowCount()).toBe(400));
    expect(thumbnailRendered).toHaveBeenCalledTimes(100);
  });

  it("draws no more than the first batch of rows when the grouping changes", async () => {
    const regroup = renderMany();
    await screen.findByText("Colt Model 0");
    scrollAnchorTo(500);
    await waitFor(() => expect(rowCount()).toBe(300));
    // The new list is still being fetched.
    listFirearms.mockReturnValue(new Promise(() => {}));
    thumbnailRendered.mockClear();

    regroup("type");

    expect(rowCount() - 1).toBe(150); // less the group's title row
    expect(thumbnailRendered.mock.calls.length).toBeLessThanOrEqual(150);
  });
});
