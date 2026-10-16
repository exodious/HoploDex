import { beforeEach, describe, expect, it, vi } from "vitest";
import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ACCESSORY_KINDS, FIREARM_TYPES } from "../../test/collectionFixtures";
import type { AccessorySummary } from "../accessories/types";
import type { FirearmSummary } from "../browse/types";
import { CollectionContext } from "../app/collectionStore";
import type { CollectionState } from "../app/collectionStore";
import { NavigationContext } from "../app/navigation";
import type { Navigation } from "../app/navigation";
import * as insuranceService from "./insuranceService";
import { InsurancePage } from "./InsurancePage";
import type { InsurancePolicy, ValueSummary } from "./types";
import { render } from "../../test/sessionScope";

// 006 US1, FR-008, FR-009, contracts/ui-accessories.md §10: active accessories
// count in the value summary as their own subtotal, in the blanket policy's
// count and in the records no policy covers, and a policy lists the
// accessories scheduled on it with its firearms.

vi.mock("./insuranceService");

const blanketPolicy: InsurancePolicy = {
  id: 1,
  name: "Homeowner's blanket",
  policyNumber: "HB-1",
  insuranceCompany: "Acme",
  companyContact: null,
  agentName: null,
  agentContact: null,
  notes: null,
  blanketCoverageLimit: 10_000,
  effectiveStartDate: "2025-07-01",
  effectiveEndDate: "2099-06-30",
  createdAt: "",
  updatedAt: "",
  isInForce: true,
  isExpired: false,
  isExpiringSoon: false,
  expiringWarning: false,
  expiredWarning: false,
};

const rider: InsurancePolicy = {
  ...blanketPolicy,
  id: 2,
  name: "Optics rider",
  policyNumber: "OR-2",
  blanketCoverageLimit: null,
};

const colt: FirearmSummary = {
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
  status: "active",
  thumbnailPhotoId: null,
  genericThumbnailKey: "handgun",
  estimatedValue: 4_000,
  insuranceWarning: "none",
  insurancePolicyId: null,
  scheduledCoverageAmount: null,
  mountedOn: null,
  mountedCounts: { firearms: 0, accessories: 0 },
};

function accessory(id: number, overrides: Partial<AccessorySummary>): AccessorySummary {
  return {
    id,
    accessoryKindId: 1,
    kindName: "Optic",
    genericThumbnailKey: "optic",
    make: "Leupold",
    model: "VX-5HD 3-15x44",
    serialNumber: null,
    caliber: null,
    cartridge: null,
    status: "active",
    thumbnailPhotoId: null,
    estimatedValue: 1_000,
    insuranceWarning: "none",
    insurancePolicyId: null,
    scheduledCoverageAmount: null,
    mountedOn: null,
    mountedCounts: { firearms: 0, accessories: 0 },
    ...overrides,
  };
}

// An optic scheduled on the rider, a magazine under the blanket, and a sling
// that was disposed of and so counts nowhere.
const optic = accessory(8, {
  insurancePolicyId: rider.id,
  scheduledCoverageAmount: 1_000,
});
const magazine = accessory(9, {
  accessoryKindId: 3,
  kindName: "Magazine",
  genericThumbnailKey: "magazine",
  make: "Walther",
  model: "P38 magazines, pair",
  estimatedValue: 180,
});
const disposed = accessory(10, {
  accessoryKindId: 10,
  kindName: "Sling",
  genericThumbnailKey: "sling",
  make: "Magpul",
  model: "MS1",
  estimatedValue: 50,
  status: "disposed",
});

function summaryWith(overrides: Partial<ValueSummary> = {}): ValueSummary {
  return {
    collectionTotal: 5_180,
    firearmsTotal: 4_000,
    accessoriesTotal: 1_180,
    blanket: {
      policyId: blanketPolicy.id,
      policyName: blanketPolicy.name,
      limit: 10_000,
      total: 4_180,
      underInsured: false,
      firearmCount: 1,
      accessoryCount: 1,
    },
    byPolicy: [
      {
        policyId: rider.id,
        policyName: rider.name,
        isExpired: false,
        isExpiringSoon: false,
        individuallyScheduled: [
          {
            record: { kind: "accessory", id: optic.id },
            estimatedValue: 1_000,
            scheduledAmount: 1_000,
            underInsured: false,
          },
        ],
      },
    ],
    uninsured: [],
    ...overrides,
  } as ValueSummary;
}

function collectionWith(
  summary: ValueSummary,
  accessories: AccessorySummary[] = [optic, magazine, disposed],
): CollectionState {
  return {
    firearms: [colt],
    firearmsById: new Map([[colt.id, colt]]),
    accessories,
    accessoriesById: new Map(accessories.map((a) => [a.id, a])),
    summary,
    policies: [blanketPolicy, rider],
    policiesById: new Map([
      [blanketPolicy.id, blanketPolicy],
      [rider.id, rider],
    ]),
    actionTypes: { actions: [], allowedByFirearmType: {} },
    actionTypesFailed: false,
    firearmTypes: { types: FIREARM_TYPES },
    firearmTypesFailed: false,
    accessoryKinds: { kinds: ACCESSORY_KINDS },
    accessoryKindsFailed: false,
    registrationClasses: { classes: [] },
    registrationClassesFailed: false,
    loaded: true,
    error: null,
    revision: 1,
    refresh: async () => {},
  } as unknown as CollectionState;
}

const open = vi.fn();

function renderPage(collection: CollectionState) {
  const navigation: Navigation = {
    route: { page: "insurance" },
    navigate: () => {},
    open,
    back: null,
    openDialog: () => {},
  };
  render(
    <CollectionContext.Provider value={collection}>
      <NavigationContext.Provider value={navigation}>
        <InsurancePage />
      </NavigationContext.Provider>
    </CollectionContext.Provider>,
  );
}

const head = () => document.querySelector<HTMLElement>(".hd-page-head")!;

beforeEach(() => {
  open.mockReset();
  vi.mocked(insuranceService.getPolicyDeletionImpact).mockResolvedValue({
    isExpired: false,
    isBlanketInForce: false,
    scheduledCounts: { firearms: 0, accessories: 0 },
    scheduledRecords: [],
    blanketCounts: { firearms: 0, accessories: 0 },
    unscheduleOutcome: "uninsured",
    otherPolicies: [],
  });
});

// 001 US3-6
describe("InsurancePage value summary (006 FR-008, contracts/ui-accessories.md §10)", () => {
  it("shows the collection total with the firearms and accessories subtotals beside it", () => {
    renderPage(collectionWith(summaryWith()));

    expect(head()).toHaveTextContent("$5,180");
    expect(head()).toHaveTextContent("Firearms $4,000");
    expect(head()).toHaveTextContent("Accessories $1,180");
  });

  it("shows the accessories line even when it is $0", () => {
    renderPage(collectionWith(summaryWith({ collectionTotal: 4_000, accessoriesTotal: 0 }), []));

    expect(head()).toHaveTextContent("Accessories $0");
  });

  it("puts the accessories line under the collection total", () => {
    renderPage(collectionWith(summaryWith()));

    const line = within(head()).getByText(/Accessories/);
    const total = within(head()).getByText("$5,180");
    expect(Boolean(total.compareDocumentPosition(line) & Node.DOCUMENT_POSITION_FOLLOWING)).toBe(
      true,
    );
  });
});

describe("InsurancePage policies (006 FR-009)", () => {
  it("counts the firearms and the accessories the blanket policy covers", () => {
    renderPage(collectionWith(summaryWith()));

    const card = screen.getByRole("heading", { name: "Homeowner's blanket" }).closest("article")!;
    expect(within(card).getByText("1 firearm and 1 accessory")).toBeInTheDocument();
  });

  it("lists the accessories scheduled on a policy by their names, and opens them", async () => {
    const user = userEvent.setup();
    renderPage(collectionWith(summaryWith()));

    const card = screen.getByRole("heading", { name: "Optics rider" }).closest("article")!;
    expect(within(card).getByText("Scheduled individually")).toBeInTheDocument();
    await user.click(within(card).getByRole("button", { name: "Leupold VX-5HD 3-15x44 · Optic" }));

    expect(open).toHaveBeenCalledWith({ page: "accessory", id: 8, from: "insurance" });
  });

  it("words the empty state for firearms and accessories alike", () => {
    renderPage({ ...collectionWith(summaryWith()), policies: [], policiesById: new Map() });

    expect(
      screen.getByText(/Add each policy that covers your firearms and accessories\./),
    ).toHaveTextContent(
      "Add each policy that covers your firearms and accessories. A blanket policy, with a coverage limit, covers every firearm and accessory you haven't scheduled, with nothing to assign. A policy without a limit covers only the firearms and accessories you schedule on it from their records.",
    );
  });

  it("heads a scheduled table by the kinds in it, not 'Record' (issue #56)", () => {
    renderPage(collectionWith(summaryWith()));

    const card = screen.getByRole("heading", { name: "Optics rider" }).closest("article")!;
    expect(within(card).getByRole("columnheader", { name: "Accessory" })).toBeInTheDocument();
  });

  it("counts each part of the coverage overview by kind (issue #56)", () => {
    renderPage(collectionWith(summaryWith()));

    const legend = document.querySelector<HTMLElement>(".hd-overview__legend")!;
    expect(legend).toHaveTextContent(/\d (firearms?|accessor(y|ies))/);
    expect(legend).not.toHaveTextContent(/\brecords?\b/);
  });

  it("does not list a disposed accessory", () => {
    renderPage(collectionWith(summaryWith()));

    expect(screen.queryByRole("button", { name: "Magpul MS1 · Sling" })).not.toBeInTheDocument();
  });
});

describe("InsurancePage records no policy covers (006 FR-008)", () => {
  it("lists an uninsured accessory with the uninsured firearms, by its name", async () => {
    const user = userEvent.setup();
    const bare = { ...magazine, insuranceWarning: "uninsured" as const };
    renderPage(
      collectionWith(
        summaryWith({
          blanket: null,
          uninsured: [{ record: { kind: "accessory", id: bare.id }, estimatedValue: 180 }],
        }),
        [bare],
      ),
    );

    const list = screen.getByRole("heading", { name: /Uninsured/ }).closest("div")!;
    await user.click(
      within(list).getByRole("button", { name: "Walther P38 magazines, pair · Magazine" }),
    );

    expect(open).toHaveBeenCalledWith({ page: "accessory", id: 9, from: "insurance" });
  });
});

// 001 ui-review (PR #1 regression): the legend's Under-insured figure is how
// much coverage is missing, not the firearms' value, and Uninsured includes a
// firearm scheduled on an expired policy (001 US3-8).
describe("InsurancePage coverage overview legend", () => {
  const firearm = (id: number, overrides: Partial<FirearmSummary>): FirearmSummary => ({
    ...colt,
    id,
    make: "InsE2EReview",
    serialNumber: `REV-${id}`,
    ...overrides,
  });
  const scheduledPolicy: InsurancePolicy = { ...rider, id: 3, name: "Review Policy" };
  const expiredPolicy: InsurancePolicy = {
    ...rider,
    id: 4,
    name: "Review Rider (expired)",
    isInForce: false,
    isExpired: true,
    expiredWarning: true,
  };
  // Unscheduled, so under the blanket policy and $4,000 over its limit.
  const rifle = firearm(1, {
    model: "Rifle",
    estimatedValue: 5_000,
    insuranceWarning: "under_insured",
  });
  // Scheduled for $1,500 of a $2,000 value: $500 short.
  const pistol = firearm(2, {
    model: "Pistol",
    estimatedValue: 2_000,
    insuranceWarning: "under_insured",
    insurancePolicyId: scheduledPolicy.id,
    scheduledCoverageAmount: 1_500,
  });
  // Scheduled on a policy that has expired (for far less than its value):
  // uninsured, its whole value, and nothing toward the shortfall.
  const pump = firearm(3, {
    model: "Pump",
    estimatedValue: 600,
    insuranceWarning: "uninsured",
    insurancePolicyId: expiredPolicy.id,
    scheduledCoverageAmount: 100,
  });

  it("reads Under-insured $4,500 short and Uninsured $600", () => {
    const summary = {
      collectionTotal: 7_600,
      firearmsTotal: 7_600,
      accessoriesTotal: 0,
      blanket: {
        policyId: blanketPolicy.id,
        policyName: blanketPolicy.name,
        limit: 1_000,
        total: 5_000,
        underInsured: true,
        firearmCount: 1,
        accessoryCount: 0,
      },
      byPolicy: [
        {
          policyId: scheduledPolicy.id,
          policyName: scheduledPolicy.name,
          isExpired: false,
          isExpiringSoon: false,
          individuallyScheduled: [
            {
              record: { kind: "firearm", id: pistol.id },
              estimatedValue: 2_000,
              scheduledAmount: 1_500,
              underInsured: true,
            },
          ],
        },
        {
          policyId: expiredPolicy.id,
          policyName: expiredPolicy.name,
          isExpired: true,
          isExpiringSoon: false,
          individuallyScheduled: [
            {
              record: { kind: "firearm", id: pump.id },
              estimatedValue: 600,
              scheduledAmount: 100,
              underInsured: false,
            },
          ],
        },
      ],
      uninsured: [{ record: { kind: "firearm", id: pump.id }, estimatedValue: 600 }],
    } as ValueSummary;
    const firearms = [rifle, pistol, pump];
    renderPage({
      ...collectionWith(summary, []),
      firearms,
      firearmsById: new Map(firearms.map((f) => [f.id, f])),
      policies: [blanketPolicy, scheduledPolicy, expiredPolicy],
      policiesById: new Map([blanketPolicy, scheduledPolicy, expiredPolicy].map((p) => [p.id, p])),
    });

    // The legend's parts are separate elements, so their text runs together.
    const legend = document
      .querySelector<HTMLElement>(".hd-overview__legend")!
      .textContent!.replace(/\s+/g, "");
    expect(legend).toContain("Under-insured$4,500short");
    expect(legend).toContain("Uninsured$600");
  });
});
