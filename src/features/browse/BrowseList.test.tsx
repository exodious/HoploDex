import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { NavigationContext } from "../app/navigation";
import type { Navigation } from "../app/navigation";
import type { RecordLabel } from "../mounts/types";
import { BrowseList } from "./BrowseList";
import { GROUP_BY_OPTIONS } from "./types";
import type { FirearmSummary, GroupBy, VisibleGroup } from "./types";

// The thumbnail talks to Tauri; it isn't under test.
vi.mock("./FirearmThumbnail", () => ({ FirearmThumbnail: () => null }));

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
    mountedCount: 0,
    ...overrides,
  };
}

const LONG_CARTRIDGE =
  "Extremely Long Wildcat Cartridge Name Improved by a Custom Gunsmith in 1987";

const firearms = [
  summary({ id: 1, model: "17", cartridge: "9x19mm Parabellum", caliber: "9mm" }),
  summary({ id: 2, make: "Hawken", model: "Plains", cartridge: null, caliber: ".50" }),
  summary({ id: 3, make: "Custom", model: "Rifle", cartridge: LONG_CARTRIDGE, caliber: ".30" }),
];

function renderList(groupBy: GroupBy | undefined) {
  const groups: VisibleGroup[] = [{ key: "All", firearms, total: firearms.length }];
  render(<BrowseList groups={groups} groupBy={groupBy} onSelect={vi.fn()} />);
}

// specs/004-cartridges-action-types FR-027, US1-10, contracts/ui-entry.md §6.
describe("BrowseList cartridge and caliber", () => {
  it("shows the cartridge with its caliber, or the caliber alone", () => {
    renderList(undefined);

    const glock = screen.getByRole("row", { name: /Glock 17/ });
    expect(within(glock).getByText("9x19mm Parabellum (9mm)")).toBeInTheDocument();
    const hawken = screen.getByRole("row", { name: /Hawken Plains/ });
    expect(within(hawken).getByText(".50")).toBeInTheDocument();
    expect(within(hawken).queryByText(/\(/)).not.toBeInTheDocument();
  });

  it("truncates a long cartridge in its cell but keeps the full text in the row's name", () => {
    renderList(undefined);

    const cell = screen.getByText(`${LONG_CARTRIDGE} (.30)`);
    expect(cell).toHaveClass("hd-cell-truncate");
    expect(cell).toHaveAttribute("title", `${LONG_CARTRIDGE} (.30)`);
    expect(
      screen.getByRole("row", { name: new RegExp(`${LONG_CARTRIDGE} \\(\\.30\\)`) }),
    ).toBeInTheDocument();
  });

  it.each([["caliber"], ["cartridge"]] as const)(
    "hides the Caliber column when grouped by %s",
    (groupBy) => {
      renderList(groupBy);
      expect(screen.queryByRole("columnheader", { name: "Caliber" })).not.toBeInTheDocument();
      expect(screen.queryByText("9x19mm Parabellum (9mm)")).not.toBeInTheDocument();
    },
  );

  it("shows the Caliber column otherwise", () => {
    renderList("make");
    expect(screen.getByRole("columnheader", { name: "Caliber" })).toBeInTheDocument();
  });

  it("offers Cartridge in the Group by control, after Caliber", () => {
    const values = GROUP_BY_OPTIONS.map((option) => option.value);
    expect(values).toContain("cartridge");
    expect(values.indexOf("cartridge")).toBe(values.indexOf("caliber") + 1);
    expect(GROUP_BY_OPTIONS.find((option) => option.value === "cartridge")?.label).toBe(
      "Cartridge",
    );
  });
});

// specs/004-cartridges-action-types US3-8, contracts/ui-entry.md §6.
describe("BrowseList action", () => {
  const withActions = [
    summary({ id: 1, model: "17", actionTypeName: "Semi-automatic" }),
    summary({ id: 2, make: "Hawken", model: "Plains", actionTypeName: null }),
  ];

  function renderActions(groupBy: GroupBy | undefined) {
    const groups: VisibleGroup[] = [{ key: "All", firearms: withActions, total: 2 }];
    render(<BrowseList groups={groups} groupBy={groupBy} onSelect={vi.fn()} />);
  }

  it("shows an Action column after Type, with the name or blank", () => {
    renderActions(undefined);

    const headers = screen.getAllByRole("columnheader").map((header) => header.textContent);
    expect(headers.indexOf("Action")).toBe(headers.indexOf("Type") + 1);
    const glock = screen.getByRole("row", { name: /Glock 17/ });
    expect(within(glock).getByText("Semi-automatic")).toBeInTheDocument();
    const hawken = screen.getByRole("row", { name: /Hawken Plains/ });
    const cells = within(hawken).getAllByRole("cell");
    expect(cells[headers.indexOf("Action")]).toHaveTextContent("");
  });

  it("hides the Action column when grouped by action type, and only then", () => {
    renderActions("action_type");
    expect(screen.queryByRole("columnheader", { name: "Action" })).not.toBeInTheDocument();
    expect(screen.queryByText("Semi-automatic")).not.toBeInTheDocument();
  });

  it("keeps the Type column when grouped by action type", () => {
    renderActions("action_type");
    expect(screen.getByRole("columnheader", { name: "Type" })).toBeInTheDocument();
  });

  it("offers Group by in the order Type, Action, Caliber, Cartridge, Make, Origin, Registered as, Registered to", () => {
    expect(GROUP_BY_OPTIONS.map((option) => option.label)).toEqual([
      "Type",
      "Action",
      "Caliber",
      "Cartridge",
      "Make",
      "Origin",
      "Registered as",
      "Registered to",
    ]);
    expect(GROUP_BY_OPTIONS.find((option) => option.label === "Action")?.value).toBe("action_type");
  });
});

// specs/006-accessory-links US2-11, FR-016a, contracts/ui-accessories.md §9:
// a mounted firearm names its host under its name; a firearm with records
// mounted on it counts them.
describe("BrowseList mount details (US2-11)", () => {
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
        <BrowseList groups={groups} groupBy={undefined} onSelect={onSelect} />
      </NavigationContext.Provider>,
    );
  }

  const mounted = summary({ id: 1, make: "Gemtech", model: "GM-45", mountedOn: deerRifle });
  const carrying = summary({ id: 2, make: "Ruger", model: "10/22", mountedCount: 3 });
  const both = summary({
    id: 3,
    make: "Springfield",
    model: "Saint",
    mountedOn: upper,
    mountedCount: 2,
  });
  const neither = summary({ id: 4, make: "Hawken", model: "Plains" });

  /** The cell holding the firearm's name. */
  const nameCell = (name: RegExp) =>
    within(screen.getByRole("row", { name })).getAllByRole("cell")[0];

  beforeEach(() => {
    open.mockReset();
    onSelect.mockReset();
  });

  it("shows 'Mounted on {host's name}' under a mounted firearm's name, the host a link", async () => {
    const user = userEvent.setup();
    renderMounts([mounted, carrying, both, neither]);

    const cell = nameCell(/Gemtech GM-45/);
    expect(cell).toHaveTextContent(/Mounted on\s*Winchester Model 70 “Deer rifle”/);
    const link =
      within(cell).queryByRole("link", { name: "Winchester Model 70 “Deer rifle”" }) ??
      within(cell).getByRole("button", { name: "Winchester Model 70 “Deer rifle”" });

    await user.click(link);

    expect(open).toHaveBeenCalledWith(expect.objectContaining({ page: "firearm", id: 7 }));
    // Following the host's link does not also open this row's firearm.
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("names an accessory host by its name and kind, and links it to the accessory", async () => {
    const user = userEvent.setup();
    renderMounts([both]);

    const cell = nameCell(/Springfield Saint/);
    expect(cell).toHaveTextContent(/Mounted on\s*BCM upper · Upper receiver/);
    const link =
      within(cell).queryByRole("link", { name: "BCM upper · Upper receiver" }) ??
      within(cell).getByRole("button", { name: "BCM upper · Upper receiver" });
    await user.click(link);

    expect(open).toHaveBeenCalledWith(expect.objectContaining({ page: "accessory", id: 11 }));
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("shows '{n} mounted' under a firearm with records on it, not linked", () => {
    renderMounts([carrying]);

    const cell = nameCell(/Ruger 10\/22/);
    const text = within(cell).getByText("3 mounted");
    expect(text.closest("a, button")).toBeNull();
    expect(cell).not.toHaveTextContent(/Mounted on/);
  });

  it("shows both lines, 'Mounted on …' first, for a firearm that is mounted and carrying", () => {
    renderMounts([both]);

    const cell = nameCell(/Springfield Saint/);
    const hostLine = within(cell).getByText(/Mounted on/);
    const countLine = within(cell).getByText("2 mounted");
    expect(
      Boolean(hostLine.compareDocumentPosition(countLine) & Node.DOCUMENT_POSITION_FOLLOWING),
    ).toBe(true);
  });

  it("shows neither line for a firearm that is neither", () => {
    renderMounts([neither]);

    const cell = nameCell(/Hawken Plains/);
    expect(cell).not.toHaveTextContent(/Mounted on|\bmounted\b/);
  });

  it("puts the lines under the name, with the serial number still shown", () => {
    renderMounts([both]);

    const cell = nameCell(/Springfield Saint/);
    const name = within(cell).getByRole("button", { name: /Springfield Saint/ });
    const hostLine = within(cell).getByText(/Mounted on/);
    expect(Boolean(name.compareDocumentPosition(hostLine) & Node.DOCUMENT_POSITION_FOLLOWING)).toBe(
      true,
    );
    expect(cell).toHaveTextContent("G-1");
  });
});
