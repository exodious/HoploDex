import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { render, screen, within, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { OriginGuide } from "./OriginGuide";

function Wrapper() {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button onClick={() => setOpen(true)}>How do I record this?</button>
      <OriginGuide open={open} onOpenChange={setOpen} />
    </>
  );
}

// specs/002-firearm-identification contracts/ui-identification.md §8, FR-015, SC-007
describe("OriginGuide", () => {
  it("shows the title and the FR-006 disclaimer at the top", () => {
    render(<OriginGuide open onOpenChange={vi.fn()} />);

    expect(
      screen.getByRole("dialog", { name: "How to record where a firearm came from" }),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        "Record what is stamped on the firearm and what your paperwork says. The app does not check it against any rules.",
      ),
    ).toBeInTheDocument();
  });

  it("introduces the worked examples as examples, not rules", () => {
    render(<OriginGuide open onOpenChange={vi.fn()} />);

    expect(screen.getByRole("dialog")).toHaveAccessibleDescription(
      "Six examples, each showing what you might see on a firearm and how to record it. Yours may not match any of them exactly: use the closest as a guide.",
    );
    const list = screen.getByRole("list", { name: "Examples" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(6);
  });

  it("renders all six required worked examples with a title, What you see, and How to record it", () => {
    render(<OriginGuide open onOpenChange={vi.fn()} />);

    const titles = [
      "Re-imported M1 Carbine",
      "Importer adopted the maker's marks",
      "Importer assigned its own serial number",
      "Older surplus import with only the maker's marks",
      "Same serial from two wartime makers",
      "Pre-1968 domestic firearms whose maker restarted numbering",
    ];
    for (const title of titles) {
      expect(screen.getByRole("heading", { name: title })).toBeInTheDocument();
    }
    expect(screen.getAllByText("What you see:")).toHaveLength(titles.length);
    expect(screen.getAllByText("How to record it:")).toHaveLength(titles.length);
  });

  it("closes with Escape and returns focus to the opening button", async () => {
    const user = userEvent.setup();
    render(<Wrapper />);

    const opener = screen.getByRole("button", { name: "How do I record this?" });
    await user.click(opener);
    expect(screen.getByRole("dialog")).toBeInTheDocument();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await waitFor(() => expect(opener).toHaveFocus());
  });

  it("closes with the Close button and returns focus to the opening button", async () => {
    const user = userEvent.setup();
    render(<Wrapper />);

    const opener = screen.getByRole("button", { name: "How do I record this?" });
    await user.click(opener);
    await user.click(screen.getByRole("button", { name: "Close" }));

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await waitFor(() => expect(opener).toHaveFocus());
  });
});
