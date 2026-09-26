import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ConfirmDialog } from "./ConfirmDialog";

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
});
