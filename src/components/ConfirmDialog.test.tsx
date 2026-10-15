import { describe, expect, it, vi } from "vitest";
import { useState } from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Checkbox } from "./Checkbox";
import { ChoiceCards } from "./ChoiceCards";
import { ConfirmDialog } from "./ConfirmDialog";
import { Select } from "./Select";
import { SegmentedControl } from "./SegmentedControl";
import { TextArea } from "./TextArea";
import { TextField } from "./TextField";

describe("ConfirmDialog", () => {
  it("calls onConfirm and closes when the confirm button is activated", async () => {
    const user = userEvent.setup();
    const onConfirm = vi.fn();
    const onOpenChange = vi.fn();

    render(
      <ConfirmDialog
        open
        onOpenChange={onOpenChange}
        title="Delete firearm?"
        description="This permanently removes the record."
        confirmLabel="Delete"
        onConfirm={onConfirm}
      />,
    );

    expect(screen.getByRole("alertdialog", { name: "Delete firearm?" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Delete" }));

    expect(onConfirm).toHaveBeenCalledTimes(1);
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  // 001 US1 Scenario 5: cancelling a firearm's delete confirmation deletes nothing.
  it("does not call onConfirm when cancelled", async () => {
    const user = userEvent.setup();
    const onConfirm = vi.fn();
    const onOpenChange = vi.fn();

    render(
      <ConfirmDialog
        open
        onOpenChange={onOpenChange}
        title="Delete firearm?"
        description="This permanently removes the record."
        onConfirm={onConfirm}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Cancel" }));

    expect(onConfirm).not.toHaveBeenCalled();
  });

  it("stays open when onConfirm rejects, so the caller can show what went wrong", async () => {
    const user = userEvent.setup();
    const onConfirm = vi.fn().mockRejectedValue(new Error("clash"));
    const onOpenChange = vi.fn();

    render(
      <ConfirmDialog
        open
        onOpenChange={onOpenChange}
        title="Restore firearm?"
        description="Back to active."
        confirmLabel="Restore"
        destructive={false}
        onConfirm={onConfirm}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Restore" }));

    expect(onConfirm).toHaveBeenCalledTimes(1);
    expect(onOpenChange).not.toHaveBeenCalledWith(false);
    expect(screen.getByRole("button", { name: "Restore" })).toBeEnabled();
  });

  it("keeps confirm disabled until the caller says it may proceed", () => {
    render(
      <ConfirmDialog
        open
        onOpenChange={vi.fn()}
        title="Restore firearm?"
        description="Back to active."
        confirmLabel="Restore"
        confirmDisabled
        onConfirm={vi.fn()}
      />,
    );

    expect(screen.getByRole("button", { name: "Restore" })).toBeDisabled();
  });

  describe("with an alternative action (specs/003 contracts/ui-databases.md §0)", () => {
    function renderThreeWay() {
      const handlers = { onConfirm: vi.fn(), onAlternative: vi.fn(), onOpenChange: vi.fn() };
      render(
        <ConfirmDialog
          open
          onOpenChange={handlers.onOpenChange}
          title="Save changes to Glock 19 (edit)?"
          description="Your changes haven't been saved."
          confirmLabel="Save changes"
          destructive={false}
          alternativeLabel="Discard changes"
          onAlternative={handlers.onAlternative}
          onConfirm={handlers.onConfirm}
        />,
      );
      return handlers;
    }

    it("shows a third button in destructive style", () => {
      renderThreeWay();

      const buttons = screen.getAllByRole("button").map((button) => button.textContent);
      expect(buttons).toEqual(["Cancel", "Discard changes", "Save changes"]);
      expect(screen.getByRole("button", { name: "Discard changes" })).toHaveClass(
        "hd-button--danger",
      );
      expect(screen.getByRole("button", { name: "Save changes" })).toHaveClass(
        "hd-button--primary",
      );
    });

    it("the confirm button calls only onConfirm", async () => {
      const user = userEvent.setup();
      const handlers = renderThreeWay();

      await user.click(screen.getByRole("button", { name: "Save changes" }));

      expect(handlers.onConfirm).toHaveBeenCalledTimes(1);
      expect(handlers.onAlternative).not.toHaveBeenCalled();
      expect(handlers.onOpenChange).toHaveBeenCalledWith(false);
    });

    it("the alternative button calls only onAlternative", async () => {
      const user = userEvent.setup();
      const handlers = renderThreeWay();

      await user.click(screen.getByRole("button", { name: "Discard changes" }));

      expect(handlers.onAlternative).toHaveBeenCalledTimes(1);
      expect(handlers.onConfirm).not.toHaveBeenCalled();
      expect(handlers.onOpenChange).toHaveBeenCalledWith(false);
    });

    it("cancel calls neither", async () => {
      const user = userEvent.setup();
      const handlers = renderThreeWay();

      await user.click(screen.getByRole("button", { name: "Cancel" }));

      expect(handlers.onConfirm).not.toHaveBeenCalled();
      expect(handlers.onAlternative).not.toHaveBeenCalled();
      expect(handlers.onOpenChange).toHaveBeenCalledWith(false);
    });

    it("has no third button without an alternative", () => {
      render(
        <ConfirmDialog
          open
          onOpenChange={vi.fn()}
          title="Delete firearm?"
          description="This permanently removes the record."
          onConfirm={vi.fn()}
        />,
      );

      expect(screen.getAllByRole("button")).toHaveLength(2);
    });
  });

  // Issue #44: the app's Enter rule (specs/003 contracts/ui-databases.md §8).
  describe("Enter", () => {
    function renderWithField(
      props: Partial<React.ComponentProps<typeof ConfirmDialog>> = {},
      extra?: React.ReactNode,
    ) {
      const handlers = { onConfirm: vi.fn(), onOpenChange: vi.fn() };
      render(
        <ConfirmDialog
          open
          onOpenChange={handlers.onOpenChange}
          title="Take over?"
          description="Enter it again."
          confirmLabel="Take over"
          onConfirm={handlers.onConfirm}
          {...props}
        >
          <TextField label="Passphrase" />
          {extra}
        </ConfirmDialog>,
      );
      return handlers;
    }

    it("in a field of the body confirms", async () => {
      const user = userEvent.setup();
      const handlers = renderWithField();

      await user.type(screen.getByLabelText("Passphrase"), "secret{Enter}");

      expect(handlers.onConfirm).toHaveBeenCalledTimes(1);
      expect(handlers.onOpenChange).toHaveBeenCalledWith(false);
    });

    it("in a field does nothing while confirm is disabled", async () => {
      const user = userEvent.setup();
      const handlers = renderWithField({ confirmDisabled: true });

      await user.type(screen.getByLabelText("Passphrase"), "secret{Enter}");

      expect(handlers.onConfirm).not.toHaveBeenCalled();
      expect(handlers.onOpenChange).not.toHaveBeenCalled();
    });

    it("in a field does not confirm a second time while the first is pending", async () => {
      const user = userEvent.setup();
      let settle: () => void = () => {};
      const onConfirm = vi.fn(() => new Promise<void>((resolve) => (settle = resolve)));
      renderWithField({ onConfirm });
      await user.type(screen.getByLabelText("Passphrase"), "secret{Enter}{Enter}");

      expect(onConfirm).toHaveBeenCalledTimes(1);
      settle();
      await waitFor(() => expect(screen.getByRole("button", { name: "Take over" })).toBeEnabled());
    });

    it("on Cancel cancels and does not confirm", async () => {
      const user = userEvent.setup();
      const handlers = renderWithField();
      screen.getByRole("button", { name: "Cancel" }).focus();

      await user.keyboard("{Enter}");

      expect(handlers.onConfirm).not.toHaveBeenCalled();
      expect(handlers.onOpenChange).toHaveBeenCalledWith(false);
    });

    it("on the alternative runs only the alternative", async () => {
      const user = userEvent.setup();
      const onAlternative = vi.fn();
      const handlers = renderWithField({ alternativeLabel: "Discard", onAlternative });
      screen.getByRole("button", { name: "Discard" }).focus();

      await user.keyboard("{Enter}");

      expect(onAlternative).toHaveBeenCalledTimes(1);
      expect(handlers.onConfirm).not.toHaveBeenCalled();
    });

    it("in a textarea adds a line and does not confirm", async () => {
      const user = userEvent.setup();
      const handlers = renderWithField({}, <TextArea label="Notes" />);

      await user.type(screen.getByLabelText("Notes"), "a{Enter}b");

      expect(screen.getByLabelText("Notes")).toHaveValue("a\nb");
      expect(handlers.onConfirm).not.toHaveBeenCalled();
    });

    it("on a radio card or a segmented radio only operates it and does not confirm", async () => {
      const user = userEvent.setup();
      const handlers = renderWithField(
        {},
        <>
          <ChoiceCards
            label="Choice"
            value=""
            onChange={vi.fn()}
            options={[{ value: "a", label: "Alpha" }]}
          />
          <SegmentedControl
            label="Mode"
            value=""
            onChange={vi.fn()}
            options={[{ value: "b", label: "Beta" }]}
          />
        </>,
      );

      for (const name of ["Alpha", "Beta"]) {
        screen.getByRole("radio", { name }).focus();
        await user.keyboard("{Enter}");
      }

      expect(handlers.onConfirm).not.toHaveBeenCalled();
      expect(handlers.onOpenChange).not.toHaveBeenCalled();
    });

    it("on a checkbox (does nothing) or a select (opens it) does not confirm and does not confirm", async () => {
      const user = userEvent.setup();
      function Controls() {
        const [checked, setChecked] = useState(false);
        return (
          <>
            <Checkbox label="Also" checked={checked} onCheckedChange={setChecked} />
            <Select
              label="Policy"
              value={undefined}
              onValueChange={vi.fn()}
              options={[{ value: "p", label: "Policy P" }]}
            />
          </>
        );
      }
      const handlers = renderWithField({}, <Controls />);

      screen.getByRole("checkbox", { name: "Also" }).focus();
      await user.keyboard("{Enter}");
      expect(screen.getByRole("checkbox", { name: "Also" })).not.toBeChecked();
      screen.getByRole("combobox", { name: "Policy" }).focus();
      await user.keyboard("{Enter}");

      expect(handlers.onConfirm).not.toHaveBeenCalled();
    });

    it("that ends an IME composition is the input method's: nothing is confirmed", () => {
      const handlers = renderWithField();
      const field = screen.getByLabelText("Passphrase");

      expect(fireEvent.keyDown(field, { key: "Enter", isComposing: true })).toBe(false);
      expect(fireEvent.keyDown(field, { key: "Enter", keyCode: 229 })).toBe(false);
      expect(handlers.onConfirm).not.toHaveBeenCalled();
      // A plain Enter is left to the form.
      expect(fireEvent.keyDown(field, { key: "Enter" })).toBe(true);
    });

    it("does not submit a form the dialog is rendered inside (the portal's events bubble)", async () => {
      const user = userEvent.setup();
      const outer = vi.fn((event: React.FormEvent) => event.preventDefault());
      const onConfirm = vi.fn();
      render(
        <form onSubmit={outer}>
          <ConfirmDialog
            open
            onOpenChange={vi.fn()}
            title="Restore?"
            description="Replace it."
            confirmLabel="Restore"
            onConfirm={onConfirm}
          >
            <TextField label="Passphrase" />
          </ConfirmDialog>
        </form>,
      );

      await user.type(screen.getByLabelText("Passphrase"), "x{Enter}");
      await user.click(screen.getByRole("button", { name: "Restore" }));

      expect(onConfirm).toHaveBeenCalledTimes(2);
      expect(outer).not.toHaveBeenCalled();
    });
  });

  describe("where focus starts", () => {
    const focused = () => document.activeElement;

    it("is the confirm button in a non-destructive question with no field, so Enter confirms", async () => {
      const user = userEvent.setup();
      const onConfirm = vi.fn();
      render(
        <ConfirmDialog
          open
          onOpenChange={vi.fn()}
          title="Move it?"
          description="Along with what is mounted on it."
          confirmLabel="Move"
          destructive={false}
          onConfirm={onConfirm}
        />,
      );

      await waitFor(() => expect(focused()).toBe(screen.getByRole("button", { name: "Move" })));
      await user.keyboard("{Enter}");

      expect(onConfirm).toHaveBeenCalledTimes(1);
    });

    it("is Cancel in a destructive question, so Enter never destroys", async () => {
      const user = userEvent.setup();
      const onConfirm = vi.fn();
      const onOpenChange = vi.fn();
      render(
        <ConfirmDialog
          open
          onOpenChange={onOpenChange}
          title="Delete it?"
          description="For good."
          confirmLabel="Delete"
          onConfirm={onConfirm}
        />,
      );

      await waitFor(() => expect(focused()).toBe(screen.getByRole("button", { name: "Cancel" })));
      await user.keyboard("{Enter}");

      expect(onConfirm).not.toHaveBeenCalled();
      expect(onOpenChange).toHaveBeenCalledWith(false);
    });

    it("is the first field when there is one, even in a non-destructive question", async () => {
      render(
        <ConfirmDialog
          open
          onOpenChange={vi.fn()}
          title="Remember?"
          description="Keyring."
          confirmLabel="Remember"
          destructive={false}
          onConfirm={vi.fn()}
        >
          <TextField label="Passphrase" />
        </ConfirmDialog>,
      );

      await waitFor(() => expect(focused()).toBe(screen.getByLabelText("Passphrase")));
    });

    it("stays on a field the content focused itself", async () => {
      render(
        <ConfirmDialog
          open
          onOpenChange={vi.fn()}
          title="Take over?"
          description="Enter it again."
          confirmLabel="Take over"
          onConfirm={vi.fn()}
        >
          <TextField label="First" />
          <TextField label="Second" autoFocus />
        </ConfirmDialog>,
      );

      await waitFor(() => expect(focused()).toBe(screen.getByLabelText("Second")));
    });

    it("is Cancel in a non-destructive question whose confirm is held back", async () => {
      render(
        <ConfirmDialog
          open
          onOpenChange={vi.fn()}
          title="Continue?"
          description="Later."
          confirmLabel="Continue"
          destructive={false}
          confirmDisabled
          onConfirm={vi.fn()}
        />,
      );

      await waitFor(() => expect(focused()).toBe(screen.getByRole("button", { name: "Cancel" })));
    });
  });
});
