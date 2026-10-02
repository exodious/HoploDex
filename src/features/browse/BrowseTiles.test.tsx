import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { NavigationContext } from "../app/navigation";
import type { Navigation } from "../app/navigation";
import type { RecordLabel } from "../mounts/types";
import { BrowseTiles } from "./BrowseTiles";
import type { FirearmSummary, VisibleGroup } from "./types";

// The thumbnail talks to Tauri; it isn't under test. A firearm with no photo
// shows its type's drawing, as the real one does.
vi.mock("./FirearmThumbnail", async () => {
  const { TypeDrawing } = await import("./TypeDrawing");
  return {
    FirearmThumbnail: ({ genericThumbnailKey }: { genericThumbnailKey: string }) => (
      <TypeDrawing typeKey={genericThumbnailKey} />
    ),
  };
});

function summary(overrides: Partial<FirearmSummary>): FirearmSummary {
  return {
    id: 1,
    make: "Glock",
    model: "17",
    nickname: null,
    serialNumber: "G-1",
    caliber: "9mm",
    cartridge: null,
    firearmTypeName: "Handgun",
    actionTypeName: null,
    registeredAs: null,
    status: "active",
    thumbnailPhotoId: null,
    genericThumbnailKey: "handgun",
    estimatedValue: 500,
    insuranceWarning: "none",
    insurancePolicyId: null,
    scheduledCoverageAmount: null,
    // specs/006-accessory-links US2: not mounted, nothing mounted on it.
    mountedOn: null,
    mountedCounts: { firearms: 0, accessories: 0 },
    ...overrides,
  };
}

// specs/004-cartridges-action-types FR-027 / US1-10: the tile's caliber line
// reads as the list's Caliber cell.
describe("BrowseTiles caliber line", () => {
  it("shows the cartridge with its caliber, or the caliber alone", () => {
    const firearms = [
      summary({ id: 1, cartridge: "9x19mm Parabellum", caliber: "9mm" }),
      summary({ id: 2, make: "Hawken", model: "Plains", serialNumber: null, caliber: ".50" }),
    ];
    const groups: VisibleGroup[] = [{ key: "All", firearms, total: 2 }];
    render(<BrowseTiles groups={groups} grouped={false} onSelect={vi.fn()} />);

    const glock = screen.getByRole("button", { name: /Glock 17/ });
    expect(within(glock).getByText("9x19mm Parabellum (9mm)")).toBeInTheDocument();
    const hawken = screen.getByRole("button", { name: /Hawken Plains/ });
    expect(within(hawken).getByText(".50")).toBeInTheDocument();
    expect(within(hawken).queryByText(/\(/)).not.toBeInTheDocument();
  });

  // specs/004-cartridges-action-types US3-8: tiles do not show the action.
  it("does not show the action", () => {
    const firearms = [summary({ id: 1, actionTypeName: "Semi-automatic" })];
    const groups: VisibleGroup[] = [{ key: "All", firearms, total: 1 }];
    render(<BrowseTiles groups={groups} grouped={false} onSelect={vi.fn()} />);

    expect(screen.queryByText("Semi-automatic")).not.toBeInTheDocument();
  });
});

// specs/005-regulated-item-types US1-4, FR-001: a Suppressor with no photo
// shows its own drawing, not the generic "other" one.
describe("BrowseTiles suppressor", () => {
  it("shows the suppressor drawing for a Suppressor with no photo", () => {
    const firearms = [
      summary({
        id: 1,
        make: "SilencerCo",
        model: "Omega 300",
        caliber: ".30",
        firearmTypeName: "Suppressor",
        genericThumbnailKey: "suppressor",
      }),
    ];
    const groups: VisibleGroup[] = [{ key: "All", firearms, total: 1 }];
    const { container } = render(
      <BrowseTiles groups={groups} grouped={false} onSelect={vi.fn()} />,
    );

    expect(container.querySelector('[data-drawing="suppressor"]')).not.toBeNull();
    expect(container.querySelector('[data-drawing="other"]')).toBeNull();
  });
});

// specs/006-accessory-links US2-11, FR-016a, contracts/ui-accessories.md §9:
// the same lines as the list, under the tile's name.
describe("BrowseTiles mount details (US2-11)", () => {
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

  const open = vi.fn();
  const onSelect = vi.fn();

  function renderMounts(items: FirearmSummary[]) {
    const navigation: Navigation = {
      route: { page: "collection" },
      navigate: () => {},
      open,
      back: null,
      openDialog: () => {},
    };
    const groups: VisibleGroup[] = [{ key: "All", firearms: items, total: items.length }];
    render(
      <NavigationContext.Provider value={navigation}>
        <BrowseTiles groups={groups} grouped={false} onSelect={onSelect} />
      </NavigationContext.Provider>,
    );
  }

  const mounted = summary({ id: 1, make: "Gemtech", model: "GM-45", mountedOn: deerRifle });
  const carrying = summary({
    id: 2,
    make: "Ruger",
    model: "10/22",
    mountedCounts: { firearms: 1, accessories: 2 },
  });
  const both = summary({
    id: 3,
    make: "Springfield",
    model: "Saint",
    mountedOn: upper,
    mountedCounts: { firearms: 0, accessories: 2 },
  });
  const neither = summary({ id: 4, make: "Hawken", model: "Plains" });

  /** The tile (list item) holding the firearm named by `text`. */
  const tile = (text: string) => screen.getByText(text).closest("li")!;

  beforeEach(() => {
    open.mockReset();
    onSelect.mockReset();
  });

  it("shows 'Mounted on {host's name}' under a mounted firearm's name, the host a link", async () => {
    const user = userEvent.setup();
    renderMounts([mounted, carrying, both, neither]);

    const item = tile("Gemtech GM-45");
    expect(item).toHaveTextContent(/Mounted on\s*Winchester Model 70 “Deer rifle”/);
    const link =
      within(item).queryByRole("link", { name: "Winchester Model 70 “Deer rifle”" }) ??
      within(item).getByRole("button", { name: "Winchester Model 70 “Deer rifle”" });

    await user.click(link);

    expect(open).toHaveBeenCalledWith(expect.objectContaining({ page: "firearm", id: 7 }));
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("links an accessory host to the accessory", async () => {
    const user = userEvent.setup();
    renderMounts([both]);

    const item = tile("Springfield Saint");
    const link =
      within(item).queryByRole("link", { name: "BCM upper · Upper receiver" }) ??
      within(item).getByRole("button", { name: "BCM upper · Upper receiver" });
    await user.click(link);

    expect(open).toHaveBeenCalledWith(expect.objectContaining({ page: "accessory", id: 11 }));
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("shows what is mounted on a firearm by kind ('1 firearm and 2 accessories mounted'), not linked", () => {
    renderMounts([carrying]);

    const item = tile("Ruger 10/22");
    const text = within(item).getByText("1 firearm and 2 accessories mounted");
    // Not a control of its own: no link, and no button other than the tile.
    expect(text.closest("a")).toBeNull();
    expect(within(item).queryByRole("link")).not.toBeInTheDocument();
    expect(text.closest("button")?.classList.contains("hd-tile") ?? true).toBe(true);
    expect(item).not.toHaveTextContent(/Mounted on/);
  });

  it("shows both lines, 'Mounted on …' first, for a firearm that is mounted and carrying", () => {
    renderMounts([both]);

    const item = tile("Springfield Saint");
    const hostLine = within(item).getByText(/Mounted on/);
    const countLine = within(item).getByText("2 accessories mounted");
    expect(
      Boolean(hostLine.compareDocumentPosition(countLine) & Node.DOCUMENT_POSITION_FOLLOWING),
    ).toBe(true);
  });

  it("shows neither line for a firearm that is neither", () => {
    renderMounts([neither]);

    expect(tile("Hawken Plains")).not.toHaveTextContent(/Mounted on|\bmounted\b/);
  });
});
