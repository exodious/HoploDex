import { describe, expect, it } from "vitest";
import { render } from "@testing-library/react";
import { ACCESSORY_KINDS, FIREARM_TYPES } from "../../test/collectionFixtures";
import { TypeDrawing } from "./TypeDrawing";
import { DRAWINGS } from "./typeDrawings";

// specs/005-regulated-item-types FR-001: every firearm type has its own
// drawing, the suppressor's included.
describe("TypeDrawing", () => {
  it("has a drawing for every seeded type's key", () => {
    for (const type of FIREARM_TYPES) {
      expect(DRAWINGS[type.genericThumbnailKey], type.name).toBeDefined();
    }
  });

  it("renders the suppressor drawing, part for part", () => {
    const { container } = render(<TypeDrawing typeKey="suppressor" />);

    const svg = container.querySelector("svg");
    expect(svg).toHaveAttribute("data-drawing", "suppressor");
    const parts = container.querySelectorAll("[class^='hd-drawing__']:not(.hd-drawing__axis)");
    expect(parts).toHaveLength(DRAWINGS.suppressor.parts.length);
    expect(container.querySelector(".hd-drawing__axis")).not.toBeNull();
  });

  // specs/006-accessory-links FR-007a: an accessory with no photo shows its
  // kind's drawing, so every kind has one.
  it("has a drawing for every accessory kind's key (FR-007a)", () => {
    for (const kind of ACCESSORY_KINDS) {
      expect(DRAWINGS[kind.genericThumbnailKey], kind.name).toBeDefined();
    }
  });

  it("gives the generic accessory drawing to the kind 'Other'", () => {
    const other = ACCESSORY_KINDS.find((kind) => kind.name === "Other");
    expect(other?.genericThumbnailKey).toBe("accessory");
    expect(DRAWINGS.accessory).toBeDefined();
  });

  it("renders the optic drawing, part for part (FR-007a)", () => {
    const { container } = render(<TypeDrawing typeKey="optic" />);

    const svg = container.querySelector("svg");
    expect(svg).toHaveAttribute("data-drawing", "optic");
    const parts = container.querySelectorAll("[class^='hd-drawing__']:not(.hd-drawing__axis)");
    expect(parts).toHaveLength(DRAWINGS.optic.parts.length);
    expect(container.querySelector(".hd-drawing__axis")).not.toBeNull();
  });

  it("draws each of the twelve kinds as its own drawing", () => {
    for (const kind of ACCESSORY_KINDS) {
      const { container, unmount } = render(<TypeDrawing typeKey={kind.genericThumbnailKey} />);
      expect(container.querySelector("svg"), kind.name).toHaveAttribute(
        "data-drawing",
        kind.genericThumbnailKey,
      );
      unmount();
    }
  });

  it("falls back to the other drawing for an unknown key", () => {
    const { container } = render(<TypeDrawing typeKey="mystery" />);

    expect(container.querySelector("svg")).toHaveAttribute("data-drawing", "other");
  });
});
