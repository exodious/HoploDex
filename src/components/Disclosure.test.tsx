import { describe, expect, it } from "vitest";
import { useState } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Disclosure } from "./Disclosure";

function Harness({ initiallyOpen = false }: { initiallyOpen?: boolean }) {
  const [open, setOpen] = useState(initiallyOpen);
  return (
    <Disclosure title="Extras" summary="Two recorded." open={open} onOpenChange={setOpen}>
      <label>
        Inside <input />
      </label>
    </Disclosure>
  );
}

describe("Disclosure", () => {
  it("is a heading holding a button that reports and toggles its state", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    const button = screen.getByRole("button", { name: "Extras Two recorded." });
    expect(screen.getByRole("heading", { level: 4 })).toContainElement(button);
    expect(button).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByLabelText("Inside")).not.toBeInTheDocument();

    await user.click(button);

    expect(button).toHaveAttribute("aria-expanded", "true");
    const panel = document.getElementById(button.getAttribute("aria-controls")!);
    expect(panel).toContainElement(screen.getByLabelText("Inside"));
  });

  it("removes its contents when closed, so they are never focusable", async () => {
    const user = userEvent.setup();
    render(<Harness initiallyOpen />);

    expect(screen.getByLabelText("Inside")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /^Extras/ }));
    expect(screen.queryByLabelText("Inside")).not.toBeInTheDocument();
  });
});
