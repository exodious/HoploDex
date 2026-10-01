import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { ACCESSORY_KINDS } from "../../test/collectionFixtures";
import { CollectionContext } from "../app/collectionStore";
import type { CollectionState } from "../app/collectionStore";
import { NavigationContext } from "../app/navigation";
import type { Navigation } from "../app/navigation";
import { AccessoriesPage } from "./AccessoriesPage";
import type { RecordLabel } from "../mounts/types";
import type { AccessoryGroup, AccessorySummary, ListAccessoriesInput } from "./types";

// specs/006-accessory-links User Story 1, contracts/ui-accessories.md §2.
// Search and grouping are User Story 4's; this is the plain list and tiles,
// and at the end the Mounted on column and tile line (User Story 2, §2).

const listAccessories = vi.fn();

vi.mock("./accessoriesService", () => ({
  listAccessories: (input: ListAccessoriesInput) => listAccessories(input),
}));
// The thumbnail talks to Tauri; it isn't under test. A record with no photo
// shows its kind's drawing, as the real one does; one with a photo shows an
// image.
vi.mock("../browse/FirearmThumbnail", async () => {
  const { TypeDrawing } = await import("../browse/TypeDrawing");
  return {
    FirearmThumbnail: ({
      thumbnailPhotoId,
      genericThumbnailKey,
    }: {
      thumbnailPhotoId: number | null;
      genericThumbnailKey: string;
    }) =>
      thumbnailPhotoId == null ? (
        <TypeDrawing typeKey={genericThumbnailKey} />
      ) : (
        <img alt="" data-testid="photo-thumbnail" data-photo-id={thumbnailPhotoId} />
      ),
  };
});

function summary(overrides: Partial<AccessorySummary>): AccessorySummary {
  return {
    id: 1,
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
    estimatedValue: 1000,
    insuranceWarning: "none",
    insurancePolicyId: null,
    scheduledCoverageAmount: null,
    mountedOn: null,
    ...overrides,
  };
}

const optic = summary({ id: 1 });
const magazine = summary({
  id: 2,
  accessoryKindId: 3,
  kindName: "Magazine",
  genericThumbnailKey: "magazine",
  make: "Walther",
  model: null,
  estimatedValue: 180,
});
const sling = summary({
  id: 3,
  accessoryKindId: 10,
  kindName: "Sling",
  genericThumbnailKey: "sling",
  make: null,
  model: null,
  estimatedValue: null,
  insuranceWarning: "under_insured",
});
const disposedLight = summary({
  id: 4,
  accessoryKindId: 2,
  kindName: "Light or laser",
  genericThumbnailKey: "light",
  make: "SureFire",
  model: "M600",
  status: "disposed",
  estimatedValue: 250,
});

const ALL = [optic, magazine, sling, disposedLight];

function groupOf(accessories: AccessorySummary[]): { groups: AccessoryGroup[] } {
  return { groups: accessories.length ? [{ key: "All", host: null, accessories }] : [] };
}

function collectionWith(accessories: AccessorySummary[]): CollectionState {
  return {
    firearms: [],
    firearmsById: new Map(),
    accessories,
    accessoriesById: new Map(accessories.map((a) => [a.id, a])),
    summary: null,
    policies: [],
    policiesById: new Map(),
    accessoryKinds: { kinds: ACCESSORY_KINDS },
    accessoryKindsFailed: false,
    loaded: true,
    error: null,
    revision: 1,
    refresh: async () => {},
  } as unknown as CollectionState;
}

type Browse = {
  query: string;
  groupBy: undefined;
  includeDisposed: boolean;
  view: "list" | "tile";
};

const open = vi.fn();
const openDialog = vi.fn();

/** Renders the page with its remembered state held here, as the shell holds
 * it, so a change to it shows. */
function renderPage(
  initial: Partial<Browse> = {},
  { all = ALL, active = ALL.filter((a) => a.status === "active") } = {},
) {
  listAccessories.mockImplementation(async (input: ListAccessoriesInput) =>
    groupOf(input.includeDisposed ? all : active),
  );
  const changes: Browse[] = [];
  function Harness() {
    const [browse, setBrowse] = useState<Browse>({
      query: "",
      groupBy: undefined,
      includeDisposed: false,
      view: "list",
      ...initial,
    });
    const navigation: Navigation = {
      route: { page: "accessories" },
      navigate: () => {},
      open,
      back: null,
      openDialog,
    };
    return (
      <CollectionContext.Provider value={collectionWith(active)}>
        <NavigationContext.Provider value={navigation}>
          <AccessoriesPage
            browse={browse}
            onBrowseChange={(next: Browse) => {
              changes.push(next);
              setBrowse(next);
            }}
          />
        </NavigationContext.Provider>
      </CollectionContext.Provider>
    );
  }
  render(<Harness />);
  return changes;
}

const NAMES = {
  optic: "Leupold VX-5HD 3-15x44 · Optic",
  magazine: "Walther · Magazine",
  sling: "Sling",
  light: "SureFire M600 · Light or laser",
};

beforeEach(() => {
  listAccessories.mockReset();
  open.mockReset();
  openDialog.mockReset();
});

describe("AccessoriesPage controls (§2)", () => {
  it("has the collection page's List | Tiles switch, Show disposed box and an Add accessory button", async () => {
    renderPage();

    await screen.findByText(NAMES.optic);
    const view = screen.getByRole("radiogroup", { name: "View" });
    expect(
      within(view)
        .getAllByRole("radio")
        // A radio input has no text of its own; its label does.
        .map((r) => r.closest("label")?.textContent),
    ).toEqual(["List", "Tiles"]);
    expect(within(view).getByRole("radio", { name: "List" })).toBeChecked();
    expect(screen.getByRole("checkbox", { name: "Show disposed" })).not.toBeChecked();
    expect(screen.getByRole("button", { name: "Add accessory" })).toBeInTheDocument();
  });

  it("is titled Accessories", async () => {
    renderPage();
    expect(
      await screen.findByRole("heading", { level: 1, name: "Accessories" }),
    ).toBeInTheDocument();
  });

  it("opens the add-accessory dialog from the Add accessory button", async () => {
    const user = userEvent.setup();
    renderPage();

    await user.click(await screen.findByRole("button", { name: "Add accessory" }));

    expect(openDialog).toHaveBeenCalledWith("addAccessory");
  });

  it("switches to tiles through the shared state", async () => {
    const user = userEvent.setup();
    const changes = renderPage();

    await screen.findByText(NAMES.optic);
    await user.click(screen.getByRole("radio", { name: "Tiles" }));

    expect(changes.at(-1)?.view).toBe("tile");
  });

  it("asks for disposed accessories only with Show disposed", async () => {
    const user = userEvent.setup();
    const changes = renderPage();

    await screen.findByText(NAMES.optic);
    expect(listAccessories).toHaveBeenLastCalledWith(
      expect.objectContaining({ includeDisposed: false }),
    );
    expect(screen.queryByText(NAMES.light)).not.toBeInTheDocument();

    await user.click(screen.getByRole("checkbox", { name: "Show disposed" }));

    expect(changes.at(-1)?.includeDisposed).toBe(true);
    expect(await screen.findByText(NAMES.light)).toBeInTheDocument();
    expect(listAccessories).toHaveBeenLastCalledWith(
      expect.objectContaining({ includeDisposed: true }),
    );
  });

  it("marks a disposed accessory as the collection page does", async () => {
    renderPage({ includeDisposed: true });

    const row = (await screen.findByText(NAMES.light)).closest("tr")!;
    expect(row).toHaveClass("hd-row--disposed");
    expect(within(row).getByText("Disposed")).toBeInTheDocument();
  });
});

describe("AccessoriesPage list (§2)", () => {
  it("has the columns Accessory, Value and Coverage", async () => {
    renderPage();

    await screen.findByText(NAMES.optic);
    const headers = screen.getAllByRole("columnheader").map((h) => h.textContent);
    expect(headers).toEqual(expect.arrayContaining(["Accessory", "Value", "Coverage"]));
    expect(headers.indexOf("Accessory")).toBeLessThan(headers.indexOf("Value"));
    expect(headers.indexOf("Value")).toBeLessThan(headers.indexOf("Coverage"));
  });

  it("names each accessory by RecordName, as a link to its record", async () => {
    renderPage();

    expect(await screen.findByText(NAMES.optic)).toBeInTheDocument();
    expect(screen.getByText(NAMES.magazine)).toBeInTheDocument();
    expect(screen.getByText(NAMES.sling)).toBeInTheDocument();
    for (const name of [NAMES.optic, NAMES.magazine, NAMES.sling]) {
      expect(screen.getByText(name).closest("button, a")).not.toBeNull();
    }
  });

  it("opens an accessory's record through navigation.open, so Back returns here", async () => {
    const user = userEvent.setup();
    renderPage();

    await user.click((await screen.findByText(NAMES.magazine)).closest("button, a")!);

    expect(open).toHaveBeenCalledWith({ page: "accessory", id: 2, from: "accessories" });
  });

  it("shows each value in whole dollars", async () => {
    renderPage();

    const opticRow = (await screen.findByText(NAMES.optic)).closest("tr")!;
    expect(within(opticRow).getByText("$1,000")).toBeInTheDocument();
    const magazineRow = screen.getByText(NAMES.magazine).closest("tr")!;
    expect(within(magazineRow).getByText("$180")).toBeInTheDocument();
  });

  it("flags an under-insured accessory with the shared InsuranceWarningBadge", async () => {
    renderPage();

    const row = (await screen.findByText(NAMES.sling)).closest("tr")!;
    expect(within(row).getByText("Under-insured")).toBeInTheDocument();
    const quiet = screen.getByText(NAMES.optic).closest("tr")!;
    expect(within(quiet).queryByText("Under-insured")).not.toBeInTheDocument();
    expect(within(quiet).queryByText("Uninsured")).not.toBeInTheDocument();
  });

  it("flags an uninsured accessory", async () => {
    renderPage({}, { active: [summary({ id: 9, insuranceWarning: "uninsured" })] });

    const row = (await screen.findByText(NAMES.optic)).closest("tr")!;
    expect(within(row).getByText("Uninsured")).toBeInTheDocument();
  });

  it("shows a thumbnail beside each name: the photo, or the kind's drawing", async () => {
    renderPage(
      {},
      {
        active: [optic, summary({ id: 7, thumbnailPhotoId: 41, make: "Aimpoint", model: "T-2" })],
      },
    );

    const plain = (await screen.findByText(NAMES.optic)).closest("tr")!;
    expect(plain.querySelector('[data-drawing="optic"]')).not.toBeNull();
    const photo = screen.getByText("Aimpoint T-2 · Optic").closest("tr")!;
    expect(within(photo).getByTestId("photo-thumbnail")).toHaveAttribute("data-photo-id", "41");
    expect(photo.querySelector("[data-drawing]")).toBeNull();
  });
});

describe("AccessoriesPage tiles (§2, FR-007a)", () => {
  it("shows a tile with the kind's drawing when there is no photo, then the name and value", async () => {
    renderPage({ view: "tile" });

    const tile = (await screen.findByText(NAMES.magazine)).closest("button")!;
    expect(tile.querySelector('[data-drawing="magazine"]')).not.toBeNull();
    expect(within(tile).getByText("$180")).toBeInTheDocument();
  });

  it("shows the thumbnail photo when there is one", async () => {
    renderPage(
      { view: "tile" },
      { active: [summary({ id: 7, thumbnailPhotoId: 41, make: "Aimpoint", model: "T-2" })] },
    );

    const tile = (await screen.findByText("Aimpoint T-2 · Optic")).closest("button")!;
    expect(within(tile).getByTestId("photo-thumbnail")).toHaveAttribute("data-photo-id", "41");
    expect(tile.querySelector("[data-drawing]")).toBeNull();
  });

  it("falls back to the generic drawing for a kind without its own", async () => {
    renderPage(
      { view: "tile" },
      {
        active: [
          summary({
            id: 8,
            accessoryKindId: 12,
            kindName: "Other",
            genericThumbnailKey: "accessory",
            make: "Odd",
            model: "Thing",
          }),
        ],
      },
    );

    const tile = (await screen.findByText("Odd Thing · Other")).closest("button")!;
    expect(tile.querySelector('[data-drawing="accessory"]')).not.toBeNull();
  });

  it("opens an accessory's record from its tile through navigation.open", async () => {
    const user = userEvent.setup();
    renderPage({ view: "tile" });

    await user.click((await screen.findByText(NAMES.sling)).closest("button")!);

    expect(open).toHaveBeenCalledWith({ page: "accessory", id: 3, from: "accessories" });
  });

  it("flags an under-insured accessory on its tile", async () => {
    renderPage({ view: "tile" });

    const tile = (await screen.findByText(NAMES.sling)).closest("button")!;
    expect(within(tile).getByText("Under-insured")).toBeInTheDocument();
  });

  it("shows disposed accessories on tiles only with Show disposed", async () => {
    renderPage({ view: "tile" });
    await screen.findByText(NAMES.optic);
    expect(screen.queryByText(NAMES.light)).not.toBeInTheDocument();
  });
});

describe("AccessoriesPage empty state (§2)", () => {
  it("says 'No accessories recorded yet.' with the Add accessory button", async () => {
    renderPage({}, { all: [], active: [] });

    expect(await screen.findByText("No accessories recorded yet.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Add accessory" })).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  it("opens the add-accessory dialog from the empty state", async () => {
    const user = userEvent.setup();
    renderPage({}, { all: [], active: [] });

    await user.click(await screen.findByRole("button", { name: "Add accessory" }));

    expect(openDialog).toHaveBeenCalledWith("addAccessory");
  });

  it("does not say so while accessories exist", async () => {
    renderPage();

    await screen.findByText(NAMES.optic);
    expect(screen.queryByText("No accessories recorded yet.")).not.toBeInTheDocument();
  });
});

// specs/006-accessory-links User Story 2 (contracts/ui-accessories.md §2,
// FR-013, FR-016): the direct host, as a link, in the list's Mounted on
// column and under the tile's name.
describe("AccessoriesPage mounted on (US2)", () => {
  const upper: RecordLabel = {
    record: { kind: "accessory", id: 11 },
    make: "BCM",
    model: "upper",
    nickname: null,
    typeName: "Upper receiver",
    serialNumber: null,
    status: "active",
  };
  const deerRifle: RecordLabel = {
    record: { kind: "firearm", id: 7 },
    make: "Winchester",
    model: "Model 70",
    nickname: "Deer rifle",
    typeName: "Rifle",
    serialNumber: null,
    status: "active",
  };

  const onRifle = summary({ id: 1, mountedOn: deerRifle });
  const onUpper = summary({
    id: 5,
    make: "Aimpoint",
    model: "T-2",
    estimatedValue: 850,
    mountedOn: upper,
  });
  const loose = summary({
    id: 2,
    accessoryKindId: 3,
    kindName: "Magazine",
    genericThumbnailKey: "magazine",
    make: "Walther",
    model: null,
    estimatedValue: 180,
  });
  const mountedAndLoose = [onRifle, onUpper, loose];

  /** A link to a record, whether rendered as a button or an anchor. */
  const link = (name: string, scope: HTMLElement) =>
    within(scope).queryByRole("link", { name }) ?? within(scope).queryByRole("button", { name });

  it("has a Mounted on column between Accessory and Value", async () => {
    renderPage({}, { active: mountedAndLoose });

    await screen.findByText(NAMES.optic);
    const headers = screen.getAllByRole("columnheader").map((h) => h.textContent);
    expect(headers).toEqual(expect.arrayContaining(["Accessory", "Mounted on", "Value"]));
    expect(headers.indexOf("Accessory")).toBeLessThan(headers.indexOf("Mounted on"));
    expect(headers.indexOf("Mounted on")).toBeLessThan(headers.indexOf("Value"));
  });

  it("shows the direct host's name as a link, and '—' when not mounted", async () => {
    const user = userEvent.setup();
    renderPage({}, { active: mountedAndLoose });

    const headers = (await screen.findAllByRole("columnheader")).map((h) => h.textContent);
    const column = headers.indexOf("Mounted on");
    const cellOf = (name: string) =>
      within(screen.getByText(name).closest("tr")!).getAllByRole("cell")[column];

    expect(cellOf(NAMES.optic)).toHaveTextContent("Winchester Model 70 “Deer rifle”");
    expect(cellOf("Aimpoint T-2 · Optic")).toHaveTextContent("BCM upper · Upper receiver");
    expect(cellOf(NAMES.magazine)).toHaveTextContent("—");
    expect(link("BCM upper · Upper receiver", cellOf("Aimpoint T-2 · Optic"))).not.toBeNull();
    expect(within(cellOf(NAMES.magazine)).queryByRole("button")).not.toBeInTheDocument();
    expect(within(cellOf(NAMES.magazine)).queryByRole("link")).not.toBeInTheDocument();

    await user.click(link("Winchester Model 70 “Deer rifle”", cellOf(NAMES.optic))!);
    expect(open).toHaveBeenLastCalledWith({ page: "firearm", id: 7, from: "accessories" });
    await user.click(link("BCM upper · Upper receiver", cellOf("Aimpoint T-2 · Optic"))!);
    expect(open).toHaveBeenLastCalledWith({ page: "accessory", id: 11, from: "accessories" });
  });

  it("shows the tile's 'Mounted on {host}' as a link under the name, and nothing for an unmounted one", async () => {
    const user = userEvent.setup();
    renderPage({ view: "tile" }, { active: mountedAndLoose });

    const tile = (await screen.findByText(NAMES.optic)).closest("li")!;
    expect(tile).toHaveTextContent(/Mounted on\s*Winchester Model 70 “Deer rifle”/);
    const hostLink = link("Winchester Model 70 “Deer rifle”", tile)!;
    expect(hostLink).not.toBeNull();
    expect(screen.getByText(NAMES.magazine).closest("li")).not.toHaveTextContent(/Mounted on/);

    await user.click(hostLink);

    expect(open).toHaveBeenLastCalledWith({ page: "firearm", id: 7, from: "accessories" });
    // Following the host's link does not also open this accessory.
    expect(open).not.toHaveBeenCalledWith(expect.objectContaining({ page: "accessory", id: 1 }));
  });
});
