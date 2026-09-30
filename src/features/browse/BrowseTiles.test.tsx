import { describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
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
