import { describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
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
    status: "active",
    thumbnailPhotoId: null,
    genericThumbnailKey: "handgun",
    estimatedValue: 500,
    insuranceWarning: "none",
    insurancePolicyId: null,
    scheduledCoverageAmount: null,
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
