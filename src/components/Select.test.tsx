import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Select } from "./Select";

// Radix Select measures and scrolls its list, which jsdom doesn't implement.
Element.prototype.hasPointerCapture ??= () => false;
Element.prototype.releasePointerCapture ??= () => undefined;
Element.prototype.scrollIntoView ??= () => undefined;

// Regression (#114): focus returned to the trigger with a plain focus(), which
// the engine may ring even after a mouse choice.
describe("Select returns focus to its trigger (#114)", () => {
  afterEach(() => vi.restoreAllMocks());

  it("without the ring after a mouse choice, with it after Escape", async () => {
    const user = userEvent.setup();
    const focus = vi.spyOn(HTMLElement.prototype, "focus");
    render(
      <Select
        label="Policy"
        value="a"
        onValueChange={() => undefined}
        options={[
          { value: "a", label: "Alpha" },
          { value: "b", label: "Bravo" },
        ]}
      />,
    );
    const trigger = screen.getByRole("combobox", { name: "Policy" });
    const lastFocusOnTrigger = () => focus.mock.calls[focus.mock.contexts.lastIndexOf(trigger)];

    fireEvent.pointerDown(trigger, { button: 0, ctrlKey: false, pointerType: "mouse" });
    const option = await screen.findByRole("option", { name: "Bravo" });
    fireEvent.pointerDown(option);
    fireEvent.click(option);
    await waitFor(() => expect(screen.queryByRole("listbox")).not.toBeInTheDocument());
    expect(trigger).toHaveFocus();
    expect(lastFocusOnTrigger()).toEqual([{ focusVisible: false }]);

    await user.keyboard("{Enter}");
    await screen.findByRole("listbox");
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("listbox")).not.toBeInTheDocument());
    expect(trigger).toHaveFocus();
    expect(lastFocusOnTrigger()).toEqual([{ focusVisible: true }]);
  });
});
