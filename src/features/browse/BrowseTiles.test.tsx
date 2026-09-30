import { describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import { BrowseTiles } from "./BrowseTiles";
import type { FirearmSummary, VisibleGroup } from "./types";

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
});
