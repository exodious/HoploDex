import { describe, expect, it, vi } from "vitest";
import { useState } from "react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Button } from "./Button";
import { Checkbox } from "./Checkbox";
import { ChoiceCards } from "./ChoiceCards";
import { Dialog } from "./Dialog";
import { Disclosure } from "./Disclosure";
import { TextField } from "./TextField";
import { UNREADABLE, changedFields, savedText } from "./fieldChanges";
import type { FieldChange } from "./fieldChanges";
import { useCloseGuard } from "./closeGuard";

describe("changedFields (014 research.md §1)", () => {
  const stored = (field: "a" | "b", state: { a: string; b: string }) =>
    field === "a" ? state.a.trim() : state.b === "?" ? UNREADABLE : state.b;

  it("marks only the fields whose stored value differs (014 FR-002, FR-004)", () => {
    const saved = { a: "Glock", b: "1" };
    expect(changedFields(saved, { a: "Glock ", b: "1" }, ["a", "b"], stored).size).toBe(0);
    expect([...changedFields(saved, { a: "glock", b: "1" }, ["a", "b"], stored)]).toEqual(["a"]);
  });

  it("counts a value it can't read yet as changed (014 FR-003)", () => {
    const saved = { a: "", b: "?" };
    expect([...changedFields(saved, saved, ["a", "b"], stored)]).toEqual(["b"]);
  });

  it("compares a form's fields in far less than 100 ms, whatever the collection holds (014 SC-001)", () => {
    // The comparison reads only the form: about 40 fields, never the records.
    const fields = Array.from({ length: 40 }, (_, i) => `f${i}`);
    const saved = Object.fromEntries(fields.map((field) => [field, "value"]));
    const current = { ...saved, f7: "other" };
    const start = performance.now();
    for (let i = 0; i < 100; i++) {
      changedFields(saved, current, fields, (field, state) => state[field].trim());
    }
    expect((performance.now() - start) / 100).toBeLessThan(10);
  });

  it("names an empty saved value and shortens a long one (014 FR-010)", () => {
    expect(savedText(null)).toBe("Not recorded");
    expect(savedText("  ")).toBe("Not recorded");
    expect(savedText("x".repeat(50))).toBe(`${"x".repeat(40)}…`);
  });
});

describe("the changed marker (014 FR-006, FR-010, FR-013)", () => {
  function Make({ saved = "Glock" }: { saved?: string }) {
    const [value, setValue] = useState(saved);
    const change: FieldChange | undefined =
      value.trim() !== saved
        ? { saved: savedText(saved), onRevert: () => setValue(saved) }
        : undefined;
    return (
      <TextField
        label="Make"
        hint="The maker"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        change={change}
      />
    );
  }

  it("shows a Changed tag and a named revert, read before the hint, and nothing when unchanged", async () => {
    const user = userEvent.setup();
    render(<Make />);
    expect(screen.queryByText("Changed")).not.toBeInTheDocument();
    const input = screen.getByLabelText("Make");
    await user.type(input, " Ges.m.b.H.");
    expect(screen.getByText("Changed")).toBeInTheDocument();
    const revert = screen.getByRole("button", { name: "Revert Make to “Glock”" });
    expect(revert).toHaveAttribute("title", "Revert to “Glock”");
    expect(input).toHaveAccessibleDescription("Changed The maker");
    await user.clear(input);
    await user.type(input, "Glock");
    expect(screen.queryByText("Changed")).not.toBeInTheDocument();
  });

  it("reverts by keyboard and puts focus back in the field", async () => {
    const user = userEvent.setup();
    render(<Make />);
    const input = screen.getByLabelText("Make");
    await user.type(input, "x");
    // The revert sits after the label, before the input: Shift+Tab reaches it.
    await user.tab({ shift: true });
    expect(screen.getByRole("button", { name: /Revert Make/ })).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(input).toHaveValue("Glock");
    await waitFor(() => expect(input).toHaveFocus());
  });

  it("marks a checkbox and a choice group the same way (014 FR-009)", async () => {
    const user = userEvent.setup();
    const onRevert = vi.fn();
    render(
      <>
        <Checkbox
          label="Remember"
          checked
          onCheckedChange={() => {}}
          change={{ saved: "Off", onRevert }}
        />
        <ChoiceCards
          label="Type"
          value="b"
          onChange={() => {}}
          options={[
            { value: "a", label: "Rifle" },
            { value: "b", label: "Suppressor" },
          ]}
          change={{ saved: "Rifle", onRevert }}
        />
      </>,
    );
    expect(screen.getByRole("checkbox")).toHaveAccessibleDescription("Changed");
    expect(screen.getByRole("radiogroup")).toHaveAccessibleDescription("Changed");
    await user.click(screen.getByRole("button", { name: "Revert Type to “Rifle”" }));
    expect(onRevert).toHaveBeenCalledTimes(1);
  });
});

describe("Save with nothing to save (014 FR-016)", () => {
  it("stays focusable, says why, and neither clicks nor submits", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn((e: React.FormEvent) => e.preventDefault());
    const onClick = vi.fn();
    render(
      <form onSubmit={onSubmit}>
        <input aria-label="Field" />
        <Button type="submit" unavailable="No changes to save" onClick={onClick}>
          Save
        </Button>
      </form>,
    );
    const save = screen.getByRole("button", { name: "Save" });
    expect(save).toHaveAttribute("aria-disabled", "true");
    expect(save).not.toBeDisabled();
    expect(save).toHaveAttribute("title", "No changes to save");
    expect(save).toHaveAccessibleDescription("No changes to save");
    await user.click(save);
    await user.type(screen.getByLabelText("Field"), "x{Enter}");
    expect(onClick).not.toHaveBeenCalled();
    expect(onSubmit).not.toHaveBeenCalled();
    await user.tab();
    expect(save).toHaveFocus();
  });
});

describe("Disclosure with a change inside (014 FR-007)", () => {
  it("says so in its folded summary only", () => {
    const { rerender } = render(
      <Disclosure
        title="Physical details"
        summary="20 in"
        open={false}
        onOpenChange={() => {}}
        changed
      >
        <p>inside</p>
      </Disclosure>,
    );
    expect(
      screen.getByRole("button", { name: /Physical details 20 in · Changed/ }),
    ).toBeInTheDocument();
    rerender(
      <Disclosure title="Physical details" summary="20 in" open onOpenChange={() => {}} changed>
        <p>inside</p>
      </Disclosure>,
    );
    expect(screen.queryByText(/Changed/)).not.toBeInTheDocument();
  });
});

describe("closing a dialog with changes (014 FR-021a)", () => {
  function Host({ dirty, onClosed }: { dirty: boolean; onClosed: () => void }) {
    const [open, setOpen] = useState(true);
    return (
      <Dialog
        open={open}
        onOpenChange={(next) => {
          setOpen(next);
          if (!next) onClosed();
        }}
        title="Edit"
        bare
      >
        <Form
          dirty={dirty}
          onCancel={() => {
            setOpen(false);
            onClosed();
          }}
        />
      </Dialog>
    );
  }
  function Form({ dirty, onCancel }: { dirty: boolean; onCancel: () => void }) {
    const requestClose = useCloseGuard(dirty, onCancel);
    return (
      <form className="hd-dialog__form">
        <div className="hd-dialog__body">
          <input aria-label="Notes" />
        </div>
        <Button onClick={requestClose}>Cancel</Button>
      </form>
    );
  }

  it("asks on Escape, Cancel and the close button, and Keep editing returns to the field", async () => {
    const user = userEvent.setup();
    const onClosed = vi.fn();
    render(<Host dirty onClosed={onClosed} />);
    const notes = screen.getByLabelText("Notes");
    notes.focus();
    await user.keyboard("{Escape}");
    const question = await screen.findByRole("alertdialog", { name: "Discard changes?" });
    expect(screen.getByRole("button", { name: "Keep editing" })).toHaveFocus();
    await user.click(screen.getByRole("button", { name: "Keep editing" }));
    await waitFor(() => expect(question).not.toBeInTheDocument());
    await waitFor(() => expect(notes).toHaveFocus());
    expect(onClosed).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Cancel" }));
    await user.click(await screen.findByRole("button", { name: "Discard" }));
    expect(onClosed).toHaveBeenCalledTimes(1);
  });

  it("asks from the close button too", async () => {
    const user = userEvent.setup();
    const onClosed = vi.fn();
    render(<Host dirty onClosed={onClosed} />);
    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(
      await screen.findByRole("alertdialog", { name: "Discard changes?" }),
    ).toBeInTheDocument();
    expect(onClosed).not.toHaveBeenCalled();
  });

  it("closes at once with nothing unsaved", async () => {
    const user = userEvent.setup();
    const onClosed = vi.fn();
    render(<Host dirty={false} onClosed={onClosed} />);
    await user.keyboard("{Escape}");
    expect(onClosed).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });
});
