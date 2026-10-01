import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { ACCESSORY_KINDS } from "../../test/collectionFixtures";
import { scrollAnchorTo, stubIntersectionObserver } from "../../test/intersectionObserver";
import { CollectionContext } from "../app/collectionStore";
import type { CollectionState } from "../app/collectionStore";
import { NavigationContext } from "../app/navigation";
import type { Navigation } from "../app/navigation";
import { AccessoriesPage } from "./AccessoriesPage";
import type { RecordLabel } from "../mounts/types";
import type {
  AccessoryGroup,
  AccessoryGroupBy,
  AccessorySummary,
  ListAccessoriesInput,
} from "./types";

// specs/006-accessory-links User Story 1, contracts/ui-accessories.md §2.
// This is the plain list and tiles, then the Mounted on column and tile line
// (User Story 2, §2), then search and grouping (User Story 4, §2).

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
  groupBy: AccessoryGroupBy | undefined;
  includeDisposed: boolean;
  view: "list" | "tile";
};

const open = vi.fn();
const openDialog = vi.fn();

/** Renders the page with its remembered state held here, as the shell holds
 * it, so a change to it shows. */
function renderPage(
  initial: Partial<Browse> = {},
  {
    all = ALL,
    active = ALL.filter((a) => a.status === "active"),
    respond,
  }: {
    all?: AccessorySummary[];
    active?: AccessorySummary[];
    /** Answers `list_accessories` itself, for a test of search or grouping. */
    respond?: (input: ListAccessoriesInput) => { groups: AccessoryGroup[] };
  } = {},
) {
  listAccessories.mockImplementation(async (input: ListAccessoriesInput) =>
    respond ? respond(input) : groupOf(input.includeDisposed ? all : active),
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

// specs/006-accessory-links User Story 4 (contracts/ui-accessories.md §2,
// FR-016 to FR-018): the collection page's search bar and "Group by" menu.
describe("AccessoriesPage search and grouping (US4)", () => {
  const deerRifle: RecordLabel = {
    record: { kind: "firearm", id: 7 },
    make: "Winchester",
    model: "Model 70",
    nickname: "Deer rifle",
    typeName: "Rifle",
    serialNumber: "W-70",
    status: "active",
  };
  const upper: RecordLabel = {
    record: { kind: "accessory", id: 11 },
    make: "BCM",
    model: "upper",
    nickname: null,
    typeName: "Upper receiver",
    serialNumber: "U-1",
    status: "active",
  };
  const sameNameA: RecordLabel = {
    record: { kind: "firearm", id: 21 },
    make: "Colt",
    model: "Python",
    nickname: null,
    typeName: "Revolver",
    serialNumber: "P-1",
    status: "active",
  };
  const sameNameB: RecordLabel = {
    ...sameNameA,
    record: { kind: "firearm", id: 22 },
    serialNumber: "P-2",
  };

  const hostGroups: AccessoryGroup[] = [
    {
      key: "Winchester Model 70 “Deer rifle”",
      host: deerRifle,
      accessories: [summary({ id: 1, mountedOn: deerRifle })],
    },
    { key: "Not mounted", host: null, accessories: [magazine, sling] },
  ];

  /** The menu's items, once opened. */
  async function openMenu(user: ReturnType<typeof userEvent.setup>, current = "None") {
    await user.click(await screen.findByRole("button", { name: `Group by, ${current}` }));
    return within(await screen.findByRole("menu"));
  }

  it("has a search bar named 'Search accessories' with the 'Search accessories…' placeholder", async () => {
    renderPage();

    const box = await screen.findByRole("searchbox", { name: "Search accessories" });
    expect(box).toHaveAttribute("placeholder", "Search accessories…");
  });

  it("sends the typed text to list_accessories, once typing pauses, and remembers it", async () => {
    const user = userEvent.setup();
    const changes = renderPage();
    await screen.findByText(NAMES.optic);

    await user.type(screen.getByRole("searchbox", { name: "Search accessories" }), "vx");

    expect(changes.at(-1)?.query).toBe("vx");
    await waitFor(() =>
      expect(listAccessories).toHaveBeenLastCalledWith(expect.objectContaining({ query: "vx" })),
    );
  });

  it("sends no query when the box is blank", async () => {
    renderPage();

    await screen.findByText(NAMES.optic);
    const input = listAccessories.mock.calls.at(-1)![0] as ListAccessoriesInput;
    expect(input.query).toBeUndefined();
    expect(input.groupBy).toBeUndefined();
  });

  it("clears the search with Escape", async () => {
    const user = userEvent.setup();
    const changes = renderPage({ query: "vx" });
    await screen.findByText(NAMES.optic);

    await user.type(screen.getByRole("searchbox", { name: "Search accessories" }), "{Escape}");

    expect(changes.at(-1)?.query).toBe("");
  });

  it('reads No accessories match "{query}". when nothing is found, with a way to clear it', async () => {
    const user = userEvent.setup();
    const changes = renderPage({ query: "zzz" }, { respond: () => ({ groups: [] }) });

    const note = await screen.findByText(/No accessories match/);
    expect(note.closest("p")).toHaveTextContent("No accessories match “zzz”.");
    expect(screen.queryByText("No accessories recorded yet.")).not.toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();

    // The search box has an icon button of that name too; this is the note's link.
    await user.click(screen.getByText("Clear search"));
    expect(changes.at(-1)?.query).toBe("");
  });

  it("counts the matches while a search is on", async () => {
    renderPage({ query: "vx" }, { respond: () => groupOf([optic, magazine]) });

    expect(await screen.findByText(/2 accessories match/)).toHaveTextContent(
      "2 accessories match “vx”.",
    );
  });

  it("reads Group by None when ungrouped and names the grouping when grouped", async () => {
    renderPage();
    const trigger = await screen.findByRole("button", { name: "Group by, None" });
    expect(trigger).toHaveAttribute("aria-haspopup", "menu");
    expect(trigger).toHaveTextContent("Group byNone");
  });

  it("offers No grouping, Kind, Make, Caliber, Cartridge and Mounted on as radio items", async () => {
    const user = userEvent.setup();
    renderPage({ groupBy: "make" });

    const menu = await openMenu(user, "Make");

    const items = menu.getAllByRole("menuitemradio");
    expect(items.map((item) => item.textContent)).toEqual([
      "No grouping",
      "Kind",
      "Make",
      "Caliber",
      "Cartridge",
      "Mounted on",
    ]);
    expect(
      items
        .filter((item) => item.getAttribute("aria-checked") === "true")
        .map((i) => i.textContent),
    ).toEqual(["Make"]);
  });

  it.each([
    ["Kind", "kind"],
    ["Make", "make"],
    ["Caliber", "caliber"],
    ["Cartridge", "cartridge"],
    ["Mounted on", "mounted_on"],
  ])("regroups by %s and asks list_accessories for %s", async (label, groupBy) => {
    const user = userEvent.setup();
    const changes = renderPage();
    await screen.findByText(NAMES.optic);

    await user.click(await screen.findByRole("button", { name: "Group by, None" }));
    await user.click(await screen.findByRole("menuitemradio", { name: label }));

    expect(changes.at(-1)?.groupBy).toBe(groupBy);
    await waitFor(() =>
      expect(listAccessories).toHaveBeenLastCalledWith(expect.objectContaining({ groupBy })),
    );
  });

  it("chooses No grouping to ungroup", async () => {
    const user = userEvent.setup();
    const changes = renderPage({ groupBy: "kind" });
    await screen.findByText(NAMES.optic);

    await user.click(await screen.findByRole("button", { name: "Group by, Kind" }));
    await user.click(await screen.findByRole("menuitemradio", { name: "No grouping" }));

    expect(changes.at(-1)?.groupBy).toBeUndefined();
  });

  it("shows a heading with a count for each group when grouped, and none when not", async () => {
    renderPage(
      { groupBy: "kind" },
      {
        respond: () => ({
          groups: [
            { key: "Optic", host: null, accessories: [optic] },
            { key: "Magazine", host: null, accessories: [magazine] },
            { key: "Sling", host: null, accessories: [sling, sling] },
          ],
        }),
      },
    );

    const headings = await screen.findAllByRole("heading", { level: 2 });
    expect(headings.map((h) => h.textContent)).toEqual(["Optic1", "Magazine1", "Sling2"]);
  });

  it("shows no group heading when ungrouped", async () => {
    renderPage();

    await screen.findByText(NAMES.optic);
    expect(screen.queryByRole("heading", { level: 2 })).not.toBeInTheDocument();
  });

  it("keeps every column when grouped by kind, make, caliber or cartridge", async () => {
    renderPage(
      { groupBy: "kind" },
      { respond: () => ({ groups: [{ key: "Optic", host: null, accessories: [optic] }] }) },
    );

    await screen.findByText(NAMES.optic);
    expect(screen.getAllByRole("columnheader").map((h) => h.textContent)).toEqual([
      "Accessory",
      "Mounted on",
      "Value",
      "Coverage",
    ]);
  });

  describe("grouped by Mounted on", () => {
    it("heads each host's group with its name as a link to the host", async () => {
      const user = userEvent.setup();
      renderPage({ groupBy: "mounted_on" }, { respond: () => ({ groups: hostGroups }) });

      const headings = await screen.findAllByRole("heading", { level: 2 });
      expect(headings[0]).toHaveTextContent("Winchester Model 70 “Deer rifle”");
      expect(headings[1]).toHaveTextContent("Not mounted");
      const hostLink = within(headings[0]).getByRole("button", {
        name: "Winchester Model 70 “Deer rifle”",
      });

      await user.click(hostLink);

      expect(open).toHaveBeenLastCalledWith({ page: "firearm", id: 7, from: "accessories" });
    });

    it("heads an accessory host's group with its RecordName, and follows it to the accessory", async () => {
      const user = userEvent.setup();
      renderPage(
        { groupBy: "mounted_on" },
        {
          respond: () => ({
            groups: [
              {
                key: "BCM upper · Upper receiver",
                host: upper,
                accessories: [summary({ id: 5, make: "Aimpoint", model: "T-2", mountedOn: upper })],
              },
            ],
          }),
        },
      );

      const heading = await screen.findByRole("heading", { level: 2 });
      await user.click(within(heading).getByRole("button", { name: "BCM upper · Upper receiver" }));

      expect(open).toHaveBeenLastCalledWith({ page: "accessory", id: 11, from: "accessories" });
    });

    it("does not make 'Not mounted' a link", async () => {
      renderPage({ groupBy: "mounted_on" }, { respond: () => ({ groups: hostGroups }) });

      const headings = await screen.findAllByRole("heading", { level: 2 });
      expect(within(headings[1]).queryByRole("button")).not.toBeInTheDocument();
    });

    it("follows the host's name with its serial number only when two hosts share the name", async () => {
      renderPage(
        { groupBy: "mounted_on" },
        {
          respond: () => ({
            groups: [
              {
                key: "Colt Python",
                host: sameNameA,
                accessories: [summary({ id: 31, mountedOn: sameNameA })],
              },
              {
                key: "Colt Python",
                host: sameNameB,
                accessories: [summary({ id: 32, mountedOn: sameNameB })],
              },
              {
                key: "Winchester Model 70 “Deer rifle”",
                host: deerRifle,
                accessories: [summary({ id: 33, mountedOn: deerRifle })],
              },
              { key: "Not mounted", host: null, accessories: [sling] },
            ],
          }),
        },
      );

      const headings = await screen.findAllByRole("heading", { level: 2 });
      expect(headings.map((h) => h.textContent)).toEqual([
        "Colt PythonP-11",
        "Colt PythonP-21",
        "Winchester Model 70 “Deer rifle”1",
        "Not mounted1",
      ]);
      // Muted: its own element, not part of the link.
      expect(within(headings[0]).getByText("P-1")).not.toHaveRole("button");
      expect(within(headings[0]).getByRole("button", { name: "Colt Python" })).toBeInTheDocument();
    });

    it("leaves out the Mounted on column, which the heading already says", async () => {
      renderPage({ groupBy: "mounted_on" }, { respond: () => ({ groups: [hostGroups[1]] }) });

      await screen.findByText(NAMES.magazine);
      expect(screen.getAllByRole("columnheader").map((h) => h.textContent)).toEqual([
        "Accessory",
        "Value",
        "Coverage",
      ]);
    });

    it("shows each group as its own section named by its heading", async () => {
      renderPage({ groupBy: "mounted_on" }, { respond: () => ({ groups: hostGroups }) });

      await screen.findByText(NAMES.magazine);
      expect(screen.getByRole("region", { name: "Not mounted" })).toBeInTheDocument();
    });

    it("groups the tiles the same way", async () => {
      renderPage(
        { groupBy: "mounted_on", view: "tile" },
        { respond: () => ({ groups: hostGroups }) },
      );

      const headings = await screen.findAllByRole("heading", { level: 2 });
      expect(headings.map((h) => h.textContent)).toEqual([
        "Winchester Model 70 “Deer rifle”1",
        "Not mounted2",
      ]);
    });
  });

  it("remembers the search and the grouping for the session (they are the shell's state)", async () => {
    const user = userEvent.setup();
    const changes = renderPage({ query: "vx", groupBy: "caliber" });
    await screen.findByText(NAMES.optic);

    expect(screen.getByRole("searchbox", { name: "Search accessories" })).toHaveValue("vx");
    expect(screen.getByRole("button", { name: "Group by, Caliber" })).toHaveTextContent("Caliber");
    // A change keeps the rest of the remembered state.
    await user.click(screen.getByRole("checkbox", { name: "Show disposed" }));
    expect(changes.at(-1)).toMatchObject({
      query: "vx",
      groupBy: "caliber",
      includeDisposed: true,
    });
  });
  // constitution IV, research.md §22: as the collection page, only the first
  // rows mount, the rest as the end of the list nears.
  describe("with thousands of accessories", () => {
    beforeEach(() => stubIntersectionObserver());
    afterEach(() => vi.unstubAllGlobals());

    const many = Array.from({ length: 400 }, (_, i) =>
      summary({ id: 100 + i, make: "Make", model: `Model ${i}` }),
    );

    it.each(["list", "tile"] as const)(
      "mounts a batch of rows at a time in the %s view, and the count is still the whole group",
      async (view) => {
        renderPage({ view, groupBy: "kind" }, { all: many, active: many });
        await screen.findByText("Make Model 0 · Optic");

        const rows = () =>
          view === "list"
            ? screen.getAllByRole("row").length - 1
            : screen.getAllByRole("listitem").length;
        expect(rows()).toBe(150);
        expect(screen.getByRole("heading", { level: 2 })).toHaveTextContent("All400");

        scrollAnchorTo(500);
        await waitFor(() => expect(rows()).toBe(300));
        scrollAnchorTo(500);
        scrollAnchorTo(500);
        await waitFor(() => expect(rows()).toBe(400));
        expect(screen.getByText("Make Model 399 · Optic")).toBeInTheDocument();
      },
    );
  });
});
