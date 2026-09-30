import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { render, screen, within, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { IdentificationGuide } from "./IdentificationGuide";
import type { GuidePart } from "./IdentificationGuide";

function Wrapper({ part }: { part?: GuidePart }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button onClick={() => setOpen(true)}>How do I record this?</button>
      <IdentificationGuide open={open} onOpenChange={setOpen} part={part} />
    </>
  );
}

// specs/002-firearm-identification contracts/ui-identification.md §8, FR-015, SC-007;
// specs/005-regulated-item-types contracts/ui-registration.md §7, FR-014
describe("IdentificationGuide", () => {
  it("shows the title and the disclaimer at the top", () => {
    render(<IdentificationGuide open onOpenChange={vi.fn()} />);

    expect(
      screen.getByRole("dialog", {
        name: "How to record where a firearm came from and how it's registered",
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        "Record what is stamped on the firearm and what your paperwork says. HoploDex doesn't check it against any rules or decide what is regulated.",
      ),
    ).toBeInTheDocument();
  });

  it("introduces the worked examples as examples, not rules", () => {
    render(<IdentificationGuide open onOpenChange={vi.fn()} />);

    expect(screen.getByRole("dialog")).toHaveAccessibleDescription(
      "Worked examples, each showing what you might have and how to record it. Yours may not match any of them exactly: use the closest as a guide.",
    );
  });

  it("keeps the six origin examples under Where it came from", () => {
    render(<IdentificationGuide open onOpenChange={vi.fn()} />);

    expect(screen.getByRole("heading", { name: "Where it came from" })).toBeInTheDocument();
    const list = screen.getByRole("list", { name: "Where it came from" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(6);
    const titles = [
      "Re-imported M1 Carbine",
      "Importer adopted the maker's marks",
      "Importer assigned its own serial number",
      "Older surplus import with only the maker's marks",
      "Same serial from two wartime makers",
      "Pre-1968 domestic firearms whose maker restarted numbering",
    ];
    for (const title of titles) {
      expect(within(list).getByRole("heading", { name: title })).toBeInTheDocument();
    }
    expect(within(list).getAllByText("What you see:")).toHaveLength(6);
    expect(within(list).getAllByText("How to record it:")).toHaveLength(6);
  });

  it("adds Registered items with two What you have / How to record it examples", () => {
    render(<IdentificationGuide open onOpenChange={vi.fn()} />);

    expect(screen.getByRole("heading", { name: "Registered items" })).toBeInTheDocument();
    const list = screen.getByRole("list", { name: "Registered items" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(2);
    expect(
      within(list).getByRole("heading", {
        name: "Suppressor bought on a Form 4, registered to a trust",
      }),
    ).toBeInTheDocument();
    expect(
      within(list).getByRole("heading", {
        name: "Rifle made into a short-barreled rifle on a Form 1",
      }),
    ).toBeInTheDocument();
    expect(within(list).getAllByText("What you have:")).toHaveLength(2);
    expect(within(list).getAllByText("How to record it:")).toHaveLength(2);
    expect(within(list).getByText(/list them in Notes/)).toBeInTheDocument();
  });

  it("opens at the top for the origin", async () => {
    const user = userEvent.setup();
    render(<Wrapper />);
    await user.click(screen.getByRole("button", { name: "How do I record this?" }));

    const close = await screen.findByRole("button", { name: "Close" });
    expect(screen.getByRole("heading", { name: "Registered items" })).not.toHaveFocus();
    expect(close).toBeInTheDocument();
  });

  it("opens at Registered items, scrolled into view and focused, for the registration", async () => {
    const scrollIntoView = vi.fn();
    Element.prototype.scrollIntoView = scrollIntoView;
    const user = userEvent.setup();
    render(<Wrapper part="registration" />);
    await user.click(screen.getByRole("button", { name: "How do I record this?" }));

    const heading = await screen.findByRole("heading", { name: "Registered items" });
    await waitFor(() => expect(heading).toHaveFocus());
    expect(scrollIntoView).toHaveBeenCalled();
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
