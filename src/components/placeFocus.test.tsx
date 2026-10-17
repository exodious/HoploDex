import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { Button } from "./Button";
import { ChoiceCards } from "./ChoiceCards";
import { ConfirmDialog } from "./ConfirmDialog";
import { Dialog } from "./Dialog";
import { placeFocus } from "./placeFocus";

// Regression: a control the app focused itself (a dialog's first control, the
// chooser's Open) showed the keyboard focus ring after a mouse click.
describe("placeFocus", () => {
  afterEach(() => vi.restoreAllMocks());

  function focusSpy() {
    return vi.spyOn(HTMLElement.prototype, "focus");
  }

  it("focuses without the ring after a pointer, with it after a key", () => {
    render(<button>Target</button>);
    const target = screen.getByRole("button");
    const focus = focusSpy();

    fireEvent.pointerDown(document.body);
    placeFocus(target);
    expect(focus).toHaveBeenLastCalledWith({ focusVisible: false });

    fireEvent.keyDown(document.body, { key: "Enter" });
    placeFocus(target);
    expect(focus).toHaveBeenLastCalledWith({ focusVisible: true });
  });

  it("is how a Button's autoFocus focuses it", () => {
    fireEvent.pointerDown(document.body);
    const focus = focusSpy();
    render(<Button autoFocus>Open</Button>);

    const button = screen.getByRole("button", { name: "Open" });
    expect(button).toHaveFocus();
    expect(focus.mock.contexts).toContain(button);
    expect(focus).toHaveBeenLastCalledWith({ focusVisible: false });
  });

  it("is how a dialog focuses its first control", () => {
    fireEvent.pointerDown(document.body);
    const focus = focusSpy();
    render(
      <Dialog open onOpenChange={() => {}} title="Restore from a backup">
        <ChoiceCards
          label="Choose a backup"
          value="a"
          onChange={() => {}}
          options={[{ value: "a", label: "The newest" }]}
        />
      </Dialog>,
    );

    const radio = screen.getByRole("radio", { name: "The newest" });
    expect(radio).toHaveFocus();
    expect(focus).toHaveBeenLastCalledWith({ focusVisible: false });
  });

  it("is how a confirmation focuses its first control", () => {
    fireEvent.pointerDown(document.body);
    const focus = focusSpy();
    render(
      <ConfirmDialog
        open
        onOpenChange={() => {}}
        title="Delete firearm?"
        description="This permanently removes the record."
        onConfirm={() => {}}
      />,
    );

    expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus();
    expect(focus).toHaveBeenLastCalledWith({ focusVisible: false });
  });
});

// Regression (#114): a closing dialog gave focus back to its opener with a
// plain focus(), which WebKitGTK rings after a mouse click.
describe("a closing dialog returns focus to its opener", () => {
  afterEach(() => vi.restoreAllMocks());

  function Opener({ confirm, open }: { confirm?: boolean; open: boolean }) {
    return (
      <>
        <button>Opener</button>
        {confirm ? (
          <ConfirmDialog
            open={open}
            onOpenChange={() => {}}
            title="Delete firearm?"
            description="This permanently removes the record."
            onConfirm={() => {}}
          />
        ) : (
          <Dialog open={open} onOpenChange={() => {}} title="Restore from a backup">
            <input aria-label="Field" />
          </Dialog>
        )}
      </>
    );
  }

  it.each([
    ["a Dialog", false],
    ["a ConfirmDialog", true],
  ])("%s: with the ring only after a key", async (_name, confirm) => {
    const { rerender } = render(<Opener confirm={confirm} open={false} />);
    const opener = screen.getByRole("button", { name: "Opener" });
    const focus = vi.spyOn(HTMLElement.prototype, "focus");
    const lastFocusOnOpener = () => focus.mock.calls[focus.mock.contexts.lastIndexOf(opener)];

    for (const [press, ring] of [
      [() => fireEvent.pointerDown(opener), false],
      [() => fireEvent.keyDown(opener, { key: "Enter" }), true],
    ] as const) {
      press();
      opener.focus();
      rerender(<Opener confirm={confirm} open />);
      await screen.findByRole(confirm ? "alertdialog" : "dialog");
      rerender(<Opener confirm={confirm} open={false} />);
      await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
      expect(opener).toHaveFocus();
      expect(lastFocusOnOpener()).toEqual([{ focusVisible: ring }]);
    }
  });
});

// Regression: a <fieldset>'s legend was counted twice by WebKitGTK when a
// dialog first sized itself, so the choices are a labelled radiogroup.
describe("ChoiceCards", () => {
  it("is a radiogroup named by its label", () => {
    render(
      <ChoiceCards
        label="Choose a backup"
        value=""
        onChange={() => {}}
        required
        options={[{ value: "a", label: "The newest" }]}
      />,
    );

    const group = screen.getByRole("radiogroup", { name: "Choose a backup" });
    expect(group).toHaveAttribute("aria-required", "true");
    expect(group.querySelector("fieldset, legend")).toBeNull();
  });
});
