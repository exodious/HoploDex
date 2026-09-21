import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { DispositionHistoryList } from "./DispositionHistoryList";

describe("DispositionHistoryList (FR-033)", () => {
  it("renders nothing when no dispositions were retained", () => {
    const { container } = render(<DispositionHistoryList entries={[]} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("lists each retained disposition as it was recorded", () => {
    render(
      <DispositionHistoryList
        entries={[
          {
            id: 2,
            firearmId: 1,
            dispositionType: "traded",
            dispositionRecipient: "Second Buyer",
            dispositionDate: "2025-02-20",
            dispositionPrice: null,
            reversedAt: "2025-03-01 10:00:00",
          },
          {
            id: 1,
            firearmId: 1,
            dispositionType: "sold",
            dispositionRecipient: "First Buyer",
            dispositionDate: "2024-01-10",
            dispositionPrice: 400,
            reversedAt: "2024-02-01 10:00:00",
          },
        ]}
      />,
    );

    expect(screen.getByRole("heading", { name: "Earlier dispositions" })).toBeInTheDocument();
    const items = screen.getAllByRole("listitem");
    expect(items).toHaveLength(2);
    expect(items[0]).toHaveTextContent("Traded");
    expect(items[0]).toHaveTextContent("Second Buyer");
    expect(items[1]).toHaveTextContent("Sold");
    expect(items[1]).toHaveTextContent("First Buyer");
    expect(items[1]).toHaveTextContent("$400");
  });
});
