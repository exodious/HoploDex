import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { BrowseList } from "../browse/BrowseList";
import { BrowseTiles } from "../browse/BrowseTiles";
import type { FirearmSummary, VisibleGroup } from "../browse/types";
import { firearmName } from "./collectionStore";
import { FirearmName } from "./FirearmName";

describe("firearmName (FR-031)", () => {
  it("is make and model alone when there is no nickname", () => {
    expect(firearmName({ make: "Glock", model: "19" })).toBe("Glock 19");
    expect(firearmName({ make: "Glock", model: "19", nickname: null })).toBe("Glock 19");
  });

  it("shows the nickname alongside make and model", () => {
    expect(firearmName({ make: "Glock", model: "19", nickname: "Old Faithful" })).toBe(
      "Glock 19 “Old Faithful”",
    );
  });
});

describe("FirearmName", () => {
  it("renders make, model and nickname as one readable name", () => {
    const { container } = render(
      <FirearmName firearm={{ make: "Glock", model: "19", nickname: "Old Faithful" }} />,
    );
    expect(container).toHaveTextContent("Glock 19 “Old Faithful”");
  });

  it("omits the nickname when there is none", () => {
    const { container } = render(<FirearmName firearm={{ make: "Glock", model: "19" }} />);
    expect(container).toHaveTextContent(/^Glock 19$/);
  });
});

function summary(id: number, nickname: string | null): FirearmSummary {
  return {
    id,
    make: "Glock",
    model: "19",
    nickname,
    serialNumber: `SN${id}`,
    caliber: "9mm",
    cartridge: null,
    firearmTypeName: "Handgun",
    status: "active",
    thumbnailPhotoId: null,
    genericThumbnailKey: "handgun",
    estimatedValue: null,
    insuranceWarning: "none",
    insurancePolicyId: null,
    scheduledCoverageAmount: null,
  };
}

const groups: VisibleGroup[] = [
  { key: "All", total: 2, firearms: [summary(1, "Range gun"), summary(2, null)] },
];

describe("browse views (US1 Scenario 8)", () => {
  it("list view tells identical firearms apart by nickname", () => {
    render(<BrowseList groups={groups} groupBy={undefined} onSelect={() => {}} />);

    expect(screen.getByRole("button", { name: "Glock 19 “Range gun”" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Glock 19" })).toBeInTheDocument();
  });

  it("tile view tells identical firearms apart by nickname", () => {
    render(<BrowseTiles groups={groups} grouped={false} onSelect={() => {}} />);

    expect(screen.getByText("“Range gun”")).toBeInTheDocument();
    expect(screen.getAllByText(/^Glock 19/)).toHaveLength(2);
  });
});
