import { beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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
vi.mock("./FirearmThumbnail", () => ({ FirearmThumbnail: () => null }));

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

  // 004 US1 Scenario 6, 005 US2 Scenario 9: the trigger names the grouping
  it("shows the chosen grouping on the trigger", async () => {
    renderPage("caliber");
    const trigger = await screen.findByRole("button", { name: "Group by, Caliber" });
    expect(trigger).toHaveTextContent("Group byCaliber");
  });

  // 004 US1 Scenario 6 (Cartridge), 005 US2 Scenario 9 (Registered as)
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

  // 001 US2 Scenario 2: the page regroups by the chosen item, and None ungroups.
  // 004 US1 Scenario 6, 005 US2 Scenario 9: choosing Cartridge or Registered as regroups
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

// 001 US3 Scenarios 7 and 14: the collection's attention banner names a policy
// the backend flags as expiring, and leaves out one whose warning is
// suppressed because another blanket policy takes over.
describe("the attention banner's expiring policies (FR-028)", () => {
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
