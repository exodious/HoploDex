import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ACCESSORY_KINDS } from "../../test/collectionFixtures";
import { formatDate } from "../../lib/dates";
import { CollectionContext } from "../app/collectionStore";
import type { CollectionState } from "../app/collectionStore";
import { NavigationContext } from "../app/navigation";
import type { Navigation } from "../app/navigation";
import type { DispositionHistoryEntry } from "../firearms/types";
import { AccessoryRecordPage } from "./AccessoryRecordPage";
import type { AccessoryDetail } from "./types";

// specs/006-accessory-links User Story 1, contracts/ui-accessories.md §12:
// the accessory's record page has the firearm record page's layout. The
// Mounted section (§5) and the "Mounted on" chain (§6) are User Story 2's.

const getAccessory = vi.fn();
const deleteAccessory = vi.fn();

vi.mock("./accessoriesService", () => ({
  getAccessory: (id: number) => getAccessory(id),
  deleteAccessory: (id: number, confirmed: boolean) => deleteAccessory(id, confirmed),
}));
// The edit form's make, model, cartridge and caliber use the shared entry
// commands (FR-003).
vi.mock("../firearms/firearmsService", () => ({
  settleEntry: async (_field: string, text: string) => ({
    value: text.trim(),
    changedBy: null,
    derivedCaliber: null,
  }),
  suggestEntries: async () => [],
}));
// The media panels talk to Tauri; they aren't under test, but the page's
// placement of them is, and so is the owner it gives them (FR-007a).
vi.mock("../media/PhotoGallery", () => ({
  PhotoGallery: (props: { owner: unknown }) => (
    <div data-testid="photo-gallery" data-owner={JSON.stringify(props.owner)} />
  ),
}));
vi.mock("../media/DocumentList", () => ({
  DocumentList: (props: { owner: unknown }) => (
    <div data-testid="documents" data-owner={JSON.stringify(props.owner)} />
  ),
}));
vi.mock("../browse/FirearmThumbnail", () => ({ FirearmThumbnail: () => null }));

const optic: AccessoryDetail = {
  id: 3,
  accessoryKindId: 1,
  make: "Leupold",
  model: "VX-5HD 3-15x44",
  serialNumber: "L-5521",
  caliber: "5.56mm",
  cartridge: "5.56x45mm NATO",
  notes: "Zeroed at 100 yards.",
  status: "active",
  estimatedValue: 1000,
  acquisitionSource: "Optics Planet",
  acquisitionDate: "2025-03-01",
  acquisitionPrice: 900,
  dispositionType: null,
  dispositionRecipient: null,
  dispositionDate: null,
  dispositionPrice: null,
  insurancePolicyId: null,
  scheduledCoverageAmount: null,
  mountedOn: null,
  thumbnailPhotoId: null,
  createdAt: "2025-01-01 00:00:00",
  updatedAt: "2025-01-01 00:00:00",
  dispositionHistory: [],
  mount: { chain: [], mounted: [] },
};

const NAME = "Leupold VX-5HD 3-15x44 · Optic";

const refresh = vi.fn();

const collection = {
  firearms: [],
  firearmsById: new Map(),
  accessories: [],
  accessoriesById: new Map(),
  summary: null,
  policies: [],
  policiesById: new Map(),
  accessoryKinds: { kinds: ACCESSORY_KINDS },
  accessoryKindsFailed: false,
  loaded: true,
  error: null,
  revision: 1,
  refresh,
} as unknown as CollectionState;

const goBack = vi.fn();

function renderPage(id = 3) {
  const navigation: Navigation = {
    route: { page: "accessory", id, from: "accessories" },
    navigate: () => {},
    open: () => {},
    back: { label: "Accessories", go: goBack },
    openDialog: () => {},
  };
  render(
    <CollectionContext.Provider value={collection}>
      <NavigationContext.Provider value={navigation}>
        <AccessoryRecordPage id={id} />
      </NavigationContext.Provider>
    </CollectionContext.Provider>,
  );
}

/** The value beside `label` in a region's list of facts. */
function fact(region: HTMLElement, label: string) {
  return within(region).getByText(label, { selector: "dt" }).nextElementSibling;
}

/** The title of whichever dialog is open. */
function openDialogTitle() {
  const dialog = screen.queryByRole("dialog") ?? screen.queryByRole("alertdialog");
  const titleId = dialog?.getAttribute("aria-labelledby");
  return titleId ? document.getElementById(titleId)?.textContent : undefined;
}

const actions = () => document.querySelector<HTMLElement>(".hd-record__actions")!;
const actionLabels = () =>
  within(actions())
    .getAllByRole("button")
    .map((b) => b.textContent?.trim());

function before(a: Element, b: Element) {
  return Boolean(a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING);
}

beforeEach(() => {
  getAccessory.mockReset().mockResolvedValue(optic);
  deleteAccessory.mockReset().mockResolvedValue({ deleted: true });
  refresh.mockReset().mockResolvedValue(undefined);
  goBack.mockReset();
  Element.prototype.scrollIntoView = vi.fn();
  window.scrollTo = vi.fn() as unknown as typeof window.scrollTo;
});

describe("AccessoryRecordPage bar (§12)", () => {
  it("loads the accessory by its id", async () => {
    renderPage();
    await screen.findByRole("heading", { level: 1 });
    expect(getAccessory).toHaveBeenCalledWith(3);
  });

  it("has Back, the accessory's name and Edit, Mark disposed and Delete", async () => {
    renderPage();

    expect(await screen.findByRole("heading", { level: 1, name: NAME })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Accessories/ })).toBeInTheDocument();
    expect(actionLabels()).toEqual(["Edit", "Mark disposed", "Delete"]);
  });

  it("goes back to where the accessory was opened from", async () => {
    const user = userEvent.setup();
    renderPage();

    await screen.findByRole("heading", { level: 1, name: NAME });
    await user.click(screen.getByRole("button", { name: /Accessories/ }));

    expect(goBack).toHaveBeenCalledTimes(1);
  });

  it("offers Restore, not Mark disposed, on a disposed accessory", async () => {
    getAccessory.mockResolvedValue({
      ...optic,
      status: "disposed",
      dispositionType: "sold",
      dispositionRecipient: "A buyer",
      dispositionDate: "2025-06-01",
      dispositionPrice: 800,
    });
    renderPage();

    await screen.findByRole("heading", { level: 1, name: NAME });
    const labels = actionLabels();
    expect(labels).toHaveLength(3);
    expect(labels[0]).toBe("Edit");
    expect(labels[1]).toMatch(/^Restore/);
    expect(labels[2]).toBe("Delete");
    expect(screen.queryByRole("button", { name: "Mark disposed" })).not.toBeInTheDocument();
  });

  it("names an accessory with neither make nor model by its kind", async () => {
    getAccessory.mockResolvedValue({ ...optic, make: null, model: null, accessoryKindId: 10 });
    renderPage();

    expect(await screen.findByRole("heading", { level: 1, name: "Sling" })).toBeInTheDocument();
  });
});

describe("AccessoryRecordPage layout (§12)", () => {
  it("lays the main column out as photos, Details, Value and acquisition, Notes, Documents, Disposition history", async () => {
    getAccessory.mockResolvedValue({
      ...optic,
      dispositionHistory: [
        {
          id: 1,
          owner: { kind: "accessory", id: 3 },
          dispositionType: "sold",
          dispositionRecipient: "A buyer",
          dispositionDate: "2025-06-01",
          dispositionPrice: 800,
          reversedAt: "2025-07-01 00:00:00",
        } as unknown as DispositionHistoryEntry,
      ],
    });
    renderPage();

    await screen.findByRole("heading", { level: 1, name: NAME });
    const main = document.querySelector<HTMLElement>(".hd-record__main")!;
    const parts = [
      within(main).getByTestId("photo-gallery"),
      within(main).getByRole("region", { name: "Details" }),
      within(main).getByRole("region", { name: "Value and acquisition" }),
      within(main).getByRole("region", { name: "Notes" }),
      within(main).getByTestId("documents"),
      within(main).getByRole("region", { name: "Disposition history" }),
    ];
    for (let i = 1; i < parts.length; i++) {
      expect(before(parts[i - 1], parts[i]), `part ${i}`).toBe(true);
    }
  });

  it("puts Coverage in the side column", async () => {
    renderPage();

    await screen.findByRole("heading", { level: 1, name: NAME });
    const side = document.querySelector<HTMLElement>(".hd-record__side")!;
    expect(within(side).getByRole("region", { name: "Coverage" })).toBeInTheDocument();
    expect(
      within(document.querySelector<HTMLElement>(".hd-record__main")!).queryByRole("region", {
        name: "Coverage",
      }),
    ).not.toBeInTheDocument();
  });

  it("gives the photo gallery and the documents the accessory as their owner (FR-007a)", async () => {
    renderPage();

    await screen.findByRole("heading", { level: 1, name: NAME });
    const owner = JSON.stringify({ kind: "accessory", id: 3 });
    expect(screen.getByTestId("photo-gallery")).toHaveAttribute("data-owner", owner);
    expect(screen.getByTestId("documents")).toHaveAttribute("data-owner", owner);
  });

  it("lists kind, make, model, serial number, caliber and cartridge under Details", async () => {
    renderPage();

    await screen.findByRole("heading", { level: 1, name: NAME });
    const details = screen.getByRole("region", { name: "Details" });
    const labels = Array.from(details.querySelectorAll("dt")).map((dt) => dt.textContent);
    expect(labels).toEqual(["Kind", "Make", "Model", "Serial number", "Caliber", "Cartridge"]);
    expect(fact(details, "Kind")).toHaveTextContent("Optic");
    expect(fact(details, "Make")).toHaveTextContent("Leupold");
    expect(fact(details, "Model")).toHaveTextContent("VX-5HD 3-15x44");
    expect(fact(details, "Serial number")).toHaveTextContent("L-5521");
    expect(fact(details, "Caliber")).toHaveTextContent("5.56mm");
    expect(fact(details, "Cartridge")).toHaveTextContent("5.56x45mm NATO");
  });

  it("shows the value and the acquisition under Value and acquisition", async () => {
    renderPage();

    await screen.findByRole("heading", { level: 1, name: NAME });
    const value = screen.getByRole("region", { name: "Value and acquisition" });
    expect(value).toHaveTextContent("$1,000");
    expect(value).toHaveTextContent("Optics Planet");
    expect(value).toHaveTextContent(formatDate("2025-03-01"));
    expect(value).toHaveTextContent("$900");
  });

  it("shows the notes, or says there are none", async () => {
    renderPage();
    await screen.findByRole("heading", { level: 1, name: NAME });
    expect(screen.getByRole("region", { name: "Notes" })).toHaveTextContent("Zeroed at 100 yards.");
  });

  it("says when no notes are recorded", async () => {
    getAccessory.mockResolvedValue({ ...optic, notes: null });
    renderPage();

    await screen.findByRole("heading", { level: 1, name: NAME });
    expect(screen.getByRole("region", { name: "Notes" })).toHaveTextContent("No notes recorded.");
  });

  it("lists earlier dispositions under Disposition history", async () => {
    getAccessory.mockResolvedValue({
      ...optic,
      dispositionHistory: [
        {
          id: 1,
          owner: { kind: "accessory", id: 3 },
          dispositionType: "sold",
          dispositionRecipient: "A buyer",
          dispositionDate: "2025-06-01",
          dispositionPrice: 800,
          reversedAt: "2025-07-01 00:00:00",
        } as unknown as DispositionHistoryEntry,
      ],
    });
    renderPage();

    await screen.findByRole("heading", { level: 1, name: NAME });
    const history = screen.getByRole("region", { name: "Disposition history" });
    expect(history).toHaveTextContent("A buyer");
    expect(history).toHaveTextContent("$800");
  });

  it("has no Mounted section yet: that is User Story 2's", async () => {
    renderPage();

    await screen.findByRole("heading", { level: 1, name: NAME });
    expect(screen.queryByRole("region", { name: "Mounted" })).not.toBeInTheDocument();
  });

  it("says nothing about a quantity", async () => {
    renderPage();

    await screen.findByRole("heading", { level: 1, name: NAME });
    expect(screen.queryByText(/quantity/i)).not.toBeInTheDocument();
  });
});

describe("AccessoryRecordPage dialogs", () => {
  it("opens the edit form, titled 'Edit {name}'", async () => {
    const user = userEvent.setup();
    renderPage();

    await user.click(await screen.findByRole("button", { name: "Edit" }));

    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    expect(openDialogTitle()).toBe(`Edit ${NAME}`);
    expect(await screen.findByLabelText(/^Make/)).toHaveValue("Leupold");
  });

  it("opens the dispose dialog from Mark disposed", async () => {
    const user = userEvent.setup();
    renderPage();

    await user.click(await screen.findByRole("button", { name: "Mark disposed" }));

    expect(await screen.findByRole("dialog")).toBeInTheDocument();
  });

  it("opens the restore dialog from Restore on a disposed accessory", async () => {
    const user = userEvent.setup();
    getAccessory.mockResolvedValue({
      ...optic,
      status: "disposed",
      dispositionType: "sold",
      dispositionRecipient: "A buyer",
      dispositionDate: "2025-06-01",
      dispositionPrice: 800,
    });
    renderPage();

    await screen.findByRole("heading", { level: 1, name: NAME });
    await user.click(within(actions()).getByRole("button", { name: /^Restore/ }));

    // The restore dialog is a confirmation, so an alert dialog, as the firearm's is.
    expect(await screen.findByRole("alertdialog")).toBeInTheDocument();
    expect(openDialogTitle()).toBe("Restore to the collection?");
  });
});

// The firearm's delete confirmation, worded for an accessory (§12).
describe("AccessoryRecordPage delete confirmation", () => {
  it("asks 'Delete {name}?' in the firearm's words, and says to mark it disposed instead", async () => {
    const user = userEvent.setup();
    renderPage();

    await user.click(await screen.findByRole("button", { name: "Delete" }));

    const dialog = await screen.findByRole("alertdialog");
    expect(openDialogTitle()).toBe(`Delete ${NAME}?`);
    expect(dialog).toHaveTextContent(
      "This erases the record entirely, including its photos and documents. It can't be undone. If you sold or transferred it, mark it disposed instead to keep its history.",
    );
    expect(within(dialog).getByRole("button", { name: "Delete accessory" })).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Cancel" })).toBeInTheDocument();
  });

  it("mentions the disposition history when the accessory is disposed", async () => {
    const user = userEvent.setup();
    getAccessory.mockResolvedValue({
      ...optic,
      status: "disposed",
      dispositionType: "sold",
      dispositionRecipient: "A buyer",
      dispositionDate: "2025-06-01",
      dispositionPrice: 800,
    });
    renderPage();

    await screen.findByRole("heading", { level: 1, name: NAME });
    await user.click(within(actions()).getByRole("button", { name: "Delete" }));

    expect(await screen.findByRole("alertdialog")).toHaveTextContent(
      "This erases the record entirely, including its photos, documents, and disposition history. It can't be undone.",
    );
  });

  it("deletes with confirmation, refreshes the collection and goes back", async () => {
    const user = userEvent.setup();
    renderPage();

    await user.click(await screen.findByRole("button", { name: "Delete" }));
    await user.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", {
        name: "Delete accessory",
      }),
    );

    await waitFor(() => expect(deleteAccessory).toHaveBeenCalledWith(3, true));
    await waitFor(() => expect(goBack).toHaveBeenCalledTimes(1));
    expect(refresh).toHaveBeenCalled();
  });

  it("deletes nothing on Cancel", async () => {
    const user = userEvent.setup();
    renderPage();

    await user.click(await screen.findByRole("button", { name: "Delete" }));
    await user.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Cancel" }),
    );

    expect(deleteAccessory).not.toHaveBeenCalled();
    expect(goBack).not.toHaveBeenCalled();
  });
});

describe("AccessoryRecordPage when the accessory can't be loaded", () => {
  it("says so, with a way back", async () => {
    getAccessory.mockRejectedValue(new Error("gone"));
    renderPage();

    expect(await screen.findByRole("alert")).toHaveTextContent(/couldn't be loaded/);
    expect(screen.getByRole("button", { name: /Accessories/ })).toBeInTheDocument();
  });
});
