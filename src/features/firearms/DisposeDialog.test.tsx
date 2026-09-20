import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DisposeDialog } from "./DisposeDialog";
import type { DisposeFirearmInput, Firearm } from "./types";

const firearm = {
  id: 1,
  make: "Colt",
  model: "Python",
  status: "active",
  acquisitionDate: "2025-03-01",
} as Firearm;

function Harness({ onDispose }: { onDispose: (input: DisposeFirearmInput) => Promise<void> }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button type="button" onClick={() => setOpen(true)}>
        Open
      </button>
      <DisposeDialog open={open} onOpenChange={setOpen} firearm={firearm} onDispose={onDispose} />
    </>
  );
}

describe("DisposeDialog", () => {
  it("starts from a blank form every time it opens", async () => {
    // Regression: values typed before cancelling reappeared on reopening.
    const user = userEvent.setup();
    render(<Harness onDispose={vi.fn()} />);

    await user.click(screen.getByRole("button", { name: "Open" }));
    await user.type(screen.getByLabelText("Transferred to"), "Jane Doe");
    await user.click(screen.getByRole("button", { name: "Cancel" }));

    await user.click(screen.getByRole("button", { name: "Open" }));
    expect(screen.getByLabelText("Transferred to")).toHaveValue("");
  });

  it("requires a price instead of silently recording $0", async () => {
    const user = userEvent.setup();
    const onDispose = vi.fn().mockResolvedValue(undefined);
    render(<Harness onDispose={onDispose} />);

    await user.click(screen.getByRole("button", { name: "Open" }));
    await user.click(screen.getByRole("radio", { name: "Sold" }));
    await user.type(screen.getByLabelText("Transferred to"), "Jane Doe");
    await user.click(screen.getByRole("button", { name: "Mark as disposed" }));

    expect(screen.getByText("Enter the price, or 0 if nothing was received.")).toBeInTheDocument();
    expect(onDispose).not.toHaveBeenCalled();

    await user.type(screen.getByLabelText("Price received"), "1,200");
    await user.click(screen.getByRole("button", { name: "Mark as disposed" }));
    expect(onDispose).toHaveBeenCalledWith(
      expect.objectContaining({ dispositionType: "sold", recipient: "Jane Doe", price: 120000 }),
    );
  });

  it("blocks a disposition date before the acquisition date", async () => {
    const user = userEvent.setup();
    const onDispose = vi.fn().mockResolvedValue(undefined);
    render(<Harness onDispose={onDispose} />);

    await user.click(screen.getByRole("button", { name: "Open" }));
    await user.click(screen.getByRole("radio", { name: "Sold" }));
    await user.type(screen.getByLabelText("Transferred to"), "Jane Doe");
    await user.type(screen.getByLabelText("Price received"), "100");
    const date = screen.getByLabelText("Date");
    await user.clear(date);
    await user.type(date, "2025-02-28");
    await user.click(screen.getByRole("button", { name: "Mark as disposed" }));

    expect(
      screen.getByText("Disposition date can't be earlier than the acquisition date."),
    ).toBeInTheDocument();
    expect(onDispose).not.toHaveBeenCalled();
  });

  it("blocks a future disposition date", async () => {
    const user = userEvent.setup();
    const onDispose = vi.fn().mockResolvedValue(undefined);
    render(<Harness onDispose={onDispose} />);

    await user.click(screen.getByRole("button", { name: "Open" }));
    await user.click(screen.getByRole("radio", { name: "Sold" }));
    await user.type(screen.getByLabelText("Transferred to"), "Jane Doe");
    await user.type(screen.getByLabelText("Price received"), "100");
    const date = screen.getByLabelText("Date");
    await user.clear(date);
    await user.type(date, "2999-01-01");
    await user.click(screen.getByRole("button", { name: "Mark as disposed" }));

    expect(screen.getByText("Disposition date can't be in the future.")).toBeInTheDocument();
    expect(onDispose).not.toHaveBeenCalled();
  });
});
