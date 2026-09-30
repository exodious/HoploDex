import { describe, expect, it } from "vitest";
import { render } from "@testing-library/react";
import { FIREARM_TYPES } from "../../test/collectionFixtures";
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

  it("falls back to the other drawing for an unknown key", () => {
    const { container } = render(<TypeDrawing typeKey="mystery" />);

    expect(container.querySelector("svg")).toHaveAttribute("data-drawing", "other");
  });
});
