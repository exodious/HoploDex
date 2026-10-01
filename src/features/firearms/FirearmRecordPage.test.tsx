import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CollectionContext } from "../app/collectionStore";
import type { CollectionState } from "../app/collectionStore";
import { NavigationContext } from "../app/navigation";
import type { Navigation } from "../app/navigation";
import { scrollAnchorTo, stubIntersectionObserver } from "../../test/intersectionObserver";
import { FirearmRecordPage } from "./FirearmRecordPage";
import { ORIGIN_OPTIONS } from "./types";
import type { FirearmDetail, Origin } from "./types";
import {
  FIREARM_TYPES,
  REGISTRATION_CLASSES,
  ACCESSORY_KINDS,
} from "../../test/collectionFixtures";

const getFirearm = vi.fn();

vi.mock("./firearmsService", () => ({ getFirearm: (id: number) => getFirearm(id) }));
// The media panels and the thumbnail talk to Tauri; they aren't under test.
vi.mock("../media/PhotoGallery", () => ({ PhotoGallery: () => null }));
vi.mock("../media/DocumentList", () => ({ DocumentList: () => null }));
vi.mock("../browse/FirearmThumbnail", () => ({ FirearmThumbnail: () => null }));

const firearm: FirearmDetail = {
  id: 1,
  make: "Colt",
  model: "Python",
  nickname: null,
  serialNumber: "V1",
  noSerialAttested: false,
  caliber: ".357",
  firearmTypeId: 1,
  cartridge: null,
  actionTypeId: null,
  notes: null,
  accessories: null,
  status: "active",
  estimatedValue: 1250,
  acquisitionSource: null,
  acquisitionDate: null,
  acquisitionPrice: null,
  dispositionType: null,
  dispositionRecipient: null,
  dispositionDate: null,
  dispositionPrice: null,
  thumbnailPhotoId: null,
  insurancePolicyId: null,
  scheduledCoverageAmount: null,
  barrelLengthHundredths: null,
  overallLengthHundredths: null,
  weightTenthsOz: null,
  capacity: null,
  finish: null,
  condition: null,
  origin: null,
  yearOfManufacture: null,
  countryOfManufacture: null,
  importerName: null,
  originalMake: null,
  originalModel: null,
  originalSerialNumber: null,
  registrationClassId: null,
  registrationForm: null,
  registrationApproved: null,
  registeredTo: null,
  createdAt: "2025-01-01 00:00:00",
  updatedAt: "2025-01-01 00:00:00",
  dispositionHistory: [],
};

const collection: CollectionState = {
  firearms: [],
  firearmsById: new Map(),
  accessories: [],
  accessoriesById: new Map(),
  summary: null,
  policies: [],
  policiesById: new Map(),
  actionTypes: {
    actions: [
      { id: 1, name: "Semi-automatic" },
      { id: 3, name: "Bolt action" },
    ],
    allowedByFirearmType: {},
  },
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

function renderPage() {
  render(
    <CollectionContext.Provider value={collection}>
      <FirearmRecordPage id={1} />
    </CollectionContext.Provider>,
  );
}

describe("FirearmRecordPage 'Add' links (FR-038, US1 Acceptance Scenario 15)", () => {
  beforeEach(() => {
    getFirearm.mockReset().mockResolvedValue(firearm);
    Element.prototype.scrollIntoView = vi.fn();
  });

  it("opens the edit form on the notes field, focused and its section highlighted", async () => {
    const user = userEvent.setup();
    renderPage();

    const notesRow = (await screen.findByText("No notes recorded.")).closest("p")!;
    await user.click(notesRow.querySelector("button")!);

    const notes = await screen.findByLabelText("Notes");
    await waitFor(() => expect(notes).toHaveFocus());
    expect(notes.closest("[data-highlight]")).not.toBeNull();
    expect(Element.prototype.scrollIntoView).toHaveBeenCalled();
  });

  it("opens the edit form on the accessories field when that Add link is followed", async () => {
    const user = userEvent.setup();
    renderPage();

    const row = (await screen.findByText("No accessories recorded.")).closest("p")!;
    await user.click(row.querySelector("button")!);

    const accessories = await screen.findByLabelText("Accessories");
    await waitFor(() => expect(accessories).toHaveFocus());
    expect(accessories.closest("[data-highlight]")).not.toBeNull();
  });

  it("opens the plain edit form, with no highlight, from the Edit button", async () => {
    const user = userEvent.setup();
    renderPage();

    await user.click(await screen.findByRole("button", { name: /^Edit/ }));

    const make = await screen.findByLabelText(/^Make/);
    await waitFor(() => expect(make).toHaveFocus());
    expect(document.querySelector("[data-highlight]")).toBeNull();
  });
});

describe("FirearmRecordPage physical details (FR-039, US1 Acceptance Scenario 17)", () => {
  beforeEach(() => {
    getFirearm.mockReset();
  });

  it("shows the recorded values in a Physical details panel", async () => {
    getFirearm.mockResolvedValue({
      ...firearm,
      barrelLengthHundredths: 1625,
      overallLengthHundredths: 3600,
      weightTenthsOz: 405,
      capacity: 15,
      finish: "Cerakote flat dark earth",
      condition: "excellent",
    });
    renderPage();

    const panel = await screen.findByRole("region", { name: "Physical details" });
    expect(within(panel).getByText("16.25 in")).toBeInTheDocument();
    expect(within(panel).getByText("36 in")).toBeInTheDocument();
    expect(within(panel).getByText("2 lb 8.5 oz")).toBeInTheDocument();
    expect(within(panel).getByText("15 rounds")).toBeInTheDocument();
    expect(within(panel).getByText("Cerakote flat dark earth")).toBeInTheDocument();
    expect(within(panel).getByText("Excellent")).toBeInTheDocument();
  });

  it("shows only the values that are recorded", async () => {
    getFirearm.mockResolvedValue({ ...firearm, capacity: 1, condition: "like_new" });
    renderPage();

    const panel = await screen.findByRole("region", { name: "Physical details" });
    expect(within(panel).getByText("1 round")).toBeInTheDocument();
    expect(within(panel).getByText("Like new")).toBeInTheDocument();
    expect(within(panel).queryByText("Barrel length")).not.toBeInTheDocument();
    expect(within(panel).queryByText("Weight")).not.toBeInTheDocument();
    expect(within(panel).queryByText("Finish")).not.toBeInTheDocument();
  });

  it("omits the panel when none are recorded", async () => {
    getFirearm.mockResolvedValue(firearm);
    renderPage();

    await screen.findByText("No notes recorded.");
    expect(screen.queryByRole("region", { name: "Physical details" })).not.toBeInTheDocument();
  });
});

// specs/004-cartridges-action-types FR-027, contracts/ui-entry.md §7.
describe("FirearmRecordPage cartridge, caliber and action", () => {
  beforeEach(() => {
    getFirearm.mockReset();
  });

  /** The title block's value for `label`. */
  const titleValue = (label: string) =>
    screen.getByText(label, { selector: "dt" }).nextElementSibling;

  it("shows Cartridge, Caliber and Action in that order, the action by name", async () => {
    getFirearm.mockResolvedValue({
      ...firearm,
      cartridge: ".357 Magnum",
      actionTypeId: 3,
    });
    renderPage();

    await screen.findByText("No notes recorded.");
    const labels = [...document.querySelectorAll(".hd-titleblock dt")].map((dt) => dt.textContent);
    expect(labels.slice(0, 3)).toEqual(["Cartridge", "Caliber", "Action"]);
    expect(titleValue("Cartridge")).toHaveTextContent(".357 Magnum");
    expect(titleValue("Action")).toHaveTextContent("Bolt action");
  });

  it("shows an unrecorded cartridge and action as a dash", async () => {
    getFirearm.mockResolvedValue(firearm);
    renderPage();

    await screen.findByText("No notes recorded.");
    expect(titleValue("Cartridge")).toHaveTextContent("—");
    expect(titleValue("Action")).toHaveTextContent("—");
  });
});

// specs/002-firearm-identification contracts/ui-identification.md §5, FR-013, US1-3
describe("FirearmRecordPage identification (US1)", () => {
  beforeEach(() => {
    getFirearm.mockReset();
  });

  it("shows Origin, Year of manufacture, Country of manufacture and Importer when recorded", async () => {
    getFirearm.mockResolvedValue({
      ...firearm,
      origin: "imported",
      yearOfManufacture: 1943,
      countryOfManufacture: "Belgium",
      importerName: "Global Arms Import Co.",
    });
    renderPage();

    const panel = await screen.findByRole("region", { name: "Identification" });
    expect(within(panel).getByText("Imported")).toBeInTheDocument();
    expect(within(panel).getByText("1943")).toBeInTheDocument();
    expect(within(panel).getByText("Belgium")).toBeInTheDocument();
    expect(within(panel).getByText("Global Arms Import Co.")).toBeInTheDocument();
  });

  it("shows United States as the country for a re-imported firearm", async () => {
    getFirearm.mockResolvedValue({
      ...firearm,
      origin: "reimported",
      importerName: "Century International Arms",
    });
    renderPage();

    const panel = await screen.findByRole("region", { name: "Identification" });
    expect(within(panel).getByText("United States")).toBeInTheDocument();
  });

  it("shows Origin: Unspecified and no importer or country row when there is no origin", async () => {
    getFirearm.mockResolvedValue(firearm);
    renderPage();

    const panel = await screen.findByRole("region", { name: "Identification" });
    expect(within(panel).getByText("Unspecified")).toBeInTheDocument();
    expect(within(panel).queryByText("Country of manufacture")).not.toBeInTheDocument();
    expect(within(panel).queryByText("Importer")).not.toBeInTheDocument();
  });

  // specs/002-firearm-identification research.md §6: the origin label must
  // agree across the SQL CASE (0002_fts5.sql), Origin::label() and
  // ORIGIN_OPTIONS — this is the TypeScript side of that guard, checking
  // ORIGIN_OPTIONS' labels are what the record page actually shows.
  it.each(ORIGIN_OPTIONS.filter((o) => o.value !== "").map((o) => [o.value, o.label]))(
    "shows %s as %s",
    async (origin, label) => {
      getFirearm.mockResolvedValue({ ...firearm, origin: origin as Origin });
      renderPage();

      const panel = await screen.findByRole("region", { name: "Identification" });
      expect(within(panel).getByText(label)).toBeInTheDocument();
    },
  );
});

// specs/002-firearm-identification contracts/ui-identification.md §5, US2-1, US2-2
describe("FirearmRecordPage original maker's marks (US2)", () => {
  beforeEach(() => {
    getFirearm.mockReset();
  });

  it("shows a labeled block with the recorded maker, model and serial number", async () => {
    getFirearm.mockResolvedValue({
      ...firearm,
      origin: "imported",
      originalMake: "Fabrique Nationale",
      originalModel: "High Power",
      originalSerialNumber: "FN-99001",
    });
    renderPage();

    const block = await screen.findByRole("region", { name: "Original maker's marks" });
    expect(within(block).getByText("Fabrique Nationale")).toBeInTheDocument();
    expect(within(block).getByText("High Power")).toBeInTheDocument();
    expect(within(block).getByText("FN-99001")).toBeInTheDocument();
  });

  it("renders when only one of the three values is recorded", async () => {
    getFirearm.mockResolvedValue({ ...firearm, origin: "imported", originalMake: "Inland" });
    renderPage();

    const block = await screen.findByRole("region", { name: "Original maker's marks" });
    expect(within(block).getByText("Inland")).toBeInTheDocument();
  });

  it("is absent when no original marks are recorded", async () => {
    getFirearm.mockResolvedValue(firearm);
    renderPage();

    await screen.findByRole("region", { name: "Identification" });
    expect(
      screen.queryByRole("region", { name: "Original maker's marks" }),
    ).not.toBeInTheDocument();
  });
});

describe("FirearmRecordPage pinned strip (FR-041, US1/AC18)", () => {
  const navigation: Navigation = {
    route: { page: "firearm", id: 1, from: "collection" },
    navigate: () => {},
    open: () => {},
    back: { label: "Collection", go: vi.fn() },
    openDialog: () => {},
  };

  function renderWithBack() {
    render(
      <CollectionContext.Provider value={collection}>
        <NavigationContext.Provider value={navigation}>
          <FirearmRecordPage id={1} />
        </NavigationContext.Provider>
      </CollectionContext.Provider>,
    );
  }

  const strip = () => document.querySelector<HTMLElement>(".hd-runhead");
  const headingActions = () => document.querySelector<HTMLElement>(".hd-record__actions")!;
  const stripActions = () => strip()!.querySelector<HTMLElement>(".hd-runhead__actions")!;
  const labels = (container: HTMLElement) =>
    within(container)
      .getAllByRole("button")
      .map((b) => b.textContent?.trim());

  /** The title of whichever dialog is open. */
  function openDialogTitle() {
    const dialog = screen.queryByRole("dialog") ?? screen.queryByRole("alertdialog");
    const titleId = dialog?.getAttribute("aria-labelledby");
    return titleId ? document.getElementById(titleId)?.textContent : undefined;
  }

  async function scrolledPastPlate(detail: FirearmDetail = firearm) {
    getFirearm.mockResolvedValue(detail);
    renderWithBack();
    await screen.findByRole("heading", { level: 1, name: "Colt Python" });
    expect(strip()).toBeNull();
    scrollAnchorTo(-400);
    expect(strip()).not.toBeNull();
  }

  beforeEach(() => {
    getFirearm.mockReset();
    stubIntersectionObserver();
    window.scrollTo = vi.fn() as unknown as typeof window.scrollTo;
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("keeps the way back, the name and the heading's actions in reach", async () => {
    await scrolledPastPlate();

    const back = within(strip()!).getByRole("button", { name: /Collection/ });
    expect(back).toHaveAttribute("aria-keyshortcuts", "Escape");
    expect(
      within(strip()!).getByRole("button", { name: /Colt Python.*V1.*back to top/ }),
    ).toBeInTheDocument();
    expect(labels(stripActions())).toEqual(["Edit", "Mark disposed", "Delete"]);
    expect(labels(stripActions())).toEqual(labels(headingActions()));
  });

  it("offers Restore to collection, not Mark disposed, on a disposed firearm", async () => {
    await scrolledPastPlate({ ...firearm, status: "disposed", dispositionType: "sold" });

    expect(labels(stripActions())).toEqual(["Edit", "Restore to collection", "Delete"]);
    expect(labels(stripActions())).toEqual(labels(headingActions()));
  });

  it.each([
    ["Edit", firearm],
    ["Mark disposed", firearm],
    ["Restore to collection", { ...firearm, status: "disposed", dispositionType: "sold" }],
    ["Delete", firearm],
  ] as [string, FirearmDetail][])(
    "the strip's %s opens the same dialog as the heading's",
    async (action, detail) => {
      const user = userEvent.setup();
      await scrolledPastPlate(detail);

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

// specs/004-cartridges-action-types FR-027, contracts/ui-entry.md §7.
describe("FirearmRecordPage cartridge", () => {
  beforeEach(() => {
    getFirearm.mockReset();
  });

  /** The title block's cells, as label and value pairs, in order. */
  async function titleCells(): Promise<[string, string][]> {
    await screen.findByRole("heading", { level: 1, name: "Colt Python" });
    const block = document.querySelector(".hd-titleblock")!;
    return Array.from(block.querySelectorAll(".hd-titleblock__cell")).map((cell) => [
      cell.querySelector("dt")!.textContent ?? "",
      cell.querySelector("dd")!.textContent ?? "",
    ]);
  }

  it("shows the cartridge before the caliber in the title block", async () => {
    getFirearm.mockResolvedValue({ ...firearm, cartridge: ".357 Magnum", caliber: ".357" });
    renderPage();

    const cells = await titleCells();
    const labels = cells.map(([label]) => label);
    expect(labels.indexOf("Cartridge")).toBe(labels.indexOf("Caliber") - 1);
    expect(cells).toContainEqual(["Cartridge", ".357 Magnum"]);
    expect(cells).toContainEqual(["Caliber", ".357"]);
  });

  it("shows an unrecorded cartridge the way it shows an unrecorded acquisition date", async () => {
    getFirearm.mockResolvedValue({ ...firearm, cartridge: null, acquisitionDate: null });
    renderPage();

    const cells = await titleCells();
    const acquired = cells.find(([label]) => label === "Acquired")![1];
    expect(cells).toContainEqual(["Cartridge", acquired]);
  });
});

// specs/005-regulated-item-types US1-3 (contracts/ui-registration.md §4): a
// Suppressor's cartridge is its rating and its caliber its bore, and it has no
// action.
describe("FirearmRecordPage title block by type", () => {
  beforeEach(() => {
    Element.prototype.scrollIntoView = vi.fn();
  });

  it("labels a Suppressor's cartridge 'Rated cartridge', keeps 'Caliber', and lists no Action, Barrel length or Capacity", async () => {
    getFirearm.mockReset().mockResolvedValue({
      ...firearm,
      make: "SilencerCo",
      model: "Omega 300",
      caliber: ".30",
      cartridge: ".300 Winchester Magnum",
      firearmTypeId: 5,
    });
    renderPage();

    const rating = await screen.findByText("Rated cartridge");
    expect(rating.closest("div")).toHaveTextContent(".300 Winchester Magnum");
    expect(screen.getByText("Caliber").closest("div")).toHaveTextContent(".30");
    expect(screen.queryByText("Cartridge")).not.toBeInTheDocument();
    expect(screen.queryByText("Caliber rating")).not.toBeInTheDocument();
    expect(screen.queryByText("Action")).not.toBeInTheDocument();
    expect(screen.queryByText("Barrel length")).not.toBeInTheDocument();
    expect(screen.queryByText("Capacity")).not.toBeInTheDocument();
  });

  it("leaves a Rifle as it was", async () => {
    getFirearm.mockReset().mockResolvedValue({ ...firearm, firearmTypeId: 2 });
    renderPage();

    expect(await screen.findByText("Caliber")).toBeInTheDocument();
    expect(screen.getByText("Cartridge")).toBeInTheDocument();
    expect(screen.getByText("Action")).toBeInTheDocument();
    expect(screen.queryByText("Rated cartridge")).not.toBeInTheDocument();
  });
});

// specs/005-regulated-item-types contracts/ui-registration.md §4, FR-018, US2-3, US2-4
describe("FirearmRecordPage registration (US2)", () => {
  beforeEach(() => {
    getFirearm.mockReset();
    Element.prototype.scrollIntoView = vi.fn();
  });

  const registered = {
    ...firearm,
    firearmTypeId: 5,
    registrationClassId: 1,
    registrationForm: "Form 4",
    registrationApproved: "2026-02-10",
    registeredTo: "Smith Family Trust",
  };

  it("lists the registration in a panel after the original marks and before the physical details", async () => {
    getFirearm.mockResolvedValue({
      ...registered,
      originalMake: "FN",
      origin: "imported",
      finish: "Blued",
    });
    renderPage();

    const panel = await screen.findByRole("region", { name: "Registration" });
    const rows = Array.from(panel.querySelectorAll(".hd-facts__row")).map((row) => [
      row.querySelector("dt")?.textContent,
      row.querySelector("dd")?.textContent,
    ]);
    expect(rows).toEqual([
      ["Registered as", "Suppressor"],
      ["Form", "Form 4"],
      ["Approved", "Feb 10, 2026"],
      ["Registered to", "Smith Family Trust"],
    ]);

    const titles = screen.getAllByRole("heading", { level: 2 }).map((h) => h.textContent);
    const at = (title: string) => titles.indexOf(title);
    expect(at("Original maker's marks")).toBeLessThan(at("Registration"));
    expect(at("Registration")).toBeLessThan(at("Physical details"));
  });

  it("omits the rows that have no value and shows no status row", async () => {
    getFirearm.mockResolvedValue({
      ...registered,
      registrationForm: null,
      registrationApproved: null,
    });
    renderPage();

    const panel = await screen.findByRole("region", { name: "Registration" });
    expect(within(panel).getByText("Registered as")).toBeInTheDocument();
    expect(within(panel).getByText("Registered to")).toBeInTheDocument();
    expect(within(panel).queryByText("Form")).not.toBeInTheDocument();
    expect(within(panel).queryByText("Approved")).not.toBeInTheDocument();
    expect(within(panel).queryByText(/status/i)).not.toBeInTheDocument();
  });

  it("shows no panel when no classification is recorded", async () => {
    getFirearm.mockResolvedValue(firearm);
    renderPage();

    await screen.findByRole("heading", { level: 1, name: "Colt Python" });
    expect(screen.queryByRole("region", { name: "Registration" })).not.toBeInTheDocument();
  });

  it("opens the form on the Registration section from the panel's Edit link", async () => {
    const user = userEvent.setup();
    getFirearm.mockResolvedValue(registered);
    renderPage();

    const panel = await screen.findByRole("region", { name: "Registration" });
    await user.click(within(panel).getByRole("button", { name: "Edit" }));

    const select = await screen.findByRole("combobox", { name: "Registered as" });
    await waitFor(() => expect(select).toHaveFocus());
    expect(screen.getByRole("button", { name: /^Registration/ })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
  });

  // FR-014, SC-002, US2-8: nothing on the page judges legal status.
  it.each([
    [
      "a Rifle with a 10.5 in barrel and no classification",
      { ...firearm, firearmTypeId: 2, barrelLengthHundredths: 1050 },
    ],
    ["a Suppressor with no classification", { ...firearm, firearmTypeId: 5 }],
    [
      "a Rifle registered as Machine gun with a Semi-automatic action",
      { ...firearm, firearmTypeId: 2, actionTypeId: 1, registrationClassId: 5 },
    ],
  ])("shows no regulatory text for %s", async (_name, record) => {
    getFirearm.mockResolvedValue(record);
    const { container } = render(
      <CollectionContext.Provider value={collection}>
        <FirearmRecordPage id={1} />
      </CollectionContext.Provider>,
    );
    await screen.findByRole("heading", { level: 1, name: "Colt Python" });

    const text = (container.textContent ?? "")
      .replace("Registered as", "")
      .replace("Registered to", "");
    expect(text).not.toMatch(/regulat|\bNFA\b|pending|unregistered|compliant|required/i);
    expect(text).not.toMatch(/register/i);
  });
});
