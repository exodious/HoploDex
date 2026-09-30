import { useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Combobox } from "./Combobox";
import type { ComboboxOption } from "./Combobox";
import { Dialog } from "./Dialog";

// specs/004-cartridges-action-types contracts/ui-entry.md §1: the shared
// list-autocomplete combobox behind Make, Model, Cartridge and Caliber.

const OPTIONS: ComboboxOption[] = [
  { value: "9x19mm Parabellum", marker: "Built-in · 9mm" },
  { value: "9x18mm Makarov", marker: "3 in collection" },
  { value: "9mm Largo", marker: "Built-in · 9mm · 2 in collection" },
];

type Load = (text: string) => Promise<ComboboxOption[]>;

function Harness({
  load,
  onPick = () => {},
  onLeave = () => {},
  onSubmit = () => {},
  initial = "",
  error,
}: {
  load: Load;
  onPick?: (value: string) => void;
  onLeave?: () => void;
  onSubmit?: () => void;
  initial?: string;
  error?: string;
}) {
  const [value, setValue] = useState(initial);
  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        onSubmit();
      }}
    >
      <Combobox
        label="Cartridge"
        hint="Optional."
        error={error}
        value={value}
        onInputChange={setValue}
        loadOptions={load}
        onPick={(picked) => {
          setValue(picked);
          onPick(picked);
        }}
        onBlurSettle={onLeave}
      />
      <button type="submit">Save</button>
    </form>
  );
}

const input = () => screen.getByRole("combobox", { name: "Cartridge" });

let load: ReturnType<typeof vi.fn<Load>>;
beforeEach(() => {
  load = vi.fn<Load>(async (text) => (text === "zzz" ? [] : OPTIONS));
});
afterEach(() => {
  vi.useRealTimers();
});

describe("Combobox", () => {
  it("is a list-autocomplete combobox that keeps its label, hint and error", async () => {
    render(<Harness load={load} error="Too long." />);

    expect(input()).toHaveAttribute("aria-autocomplete", "list");
    expect(input()).toHaveAttribute("aria-expanded", "false");
    expect(input()).toHaveAttribute("aria-controls");
    expect(input()).toHaveAccessibleDescription("Optional. Too long.");
    expect(screen.getByRole("alert")).toHaveTextContent("Too long.");

    await userEvent.setup().click(input());
    const list = await screen.findByRole("listbox");
    expect(input()).toHaveAttribute("aria-expanded", "true");
    expect(input().getAttribute("aria-controls")).toBe(list.id);
  });

  it("opens on focus with the list for the current text, and names each row with its marker", async () => {
    const user = userEvent.setup();
    render(<Harness load={load} />);

    await user.click(input());

    expect(load).toHaveBeenCalledWith("");
    const rows = await screen.findAllByRole("option");
    expect(rows).toHaveLength(3);
    expect(screen.getByRole("option", { name: "9x19mm Parabellum Built-in · 9mm" })).toBeVisible();
    expect(screen.getByRole("option", { name: "9x18mm Makarov 3 in collection" })).toBeVisible();
    expect(
      screen.getByRole("option", { name: "9mm Largo Built-in · 9mm · 2 in collection" }),
    ).toBeVisible();
  });

  it("stays closed when focus is placed without the user's doing, until Down or typing", async () => {
    // A dialog focusing its first field as it opens is not asking for a list,
    // and Escape must still close the dialog.
    const user = userEvent.setup();
    render(<Harness load={load} />);

    act(() => input().focus());
    expect(load).not.toHaveBeenCalled();
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();

    await user.keyboard("{ArrowDown}");
    expect(await screen.findByRole("listbox")).toBeVisible();
  });

  it("stays closed when a dialog moves focus in from the button that opened it", async () => {
    // The button had focus, so the field's focus event names it as related,
    // yet nothing the user pressed asked for the list: Escape must close the
    // dialog.
    render(
      <div>
        <button type="button">Edit</button>
        <Harness load={load} />
      </div>,
    );
    screen.getByRole("button", { name: "Edit" }).focus();

    act(() => input().focus());

    expect(input()).toHaveFocus();
    expect(load).not.toHaveBeenCalled();
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  it("opens when Tab arrives from another control", async () => {
    const user = userEvent.setup();
    render(
      <div>
        <button type="button">Before</button>
        <Harness load={load} />
      </div>,
    );
    screen.getByRole("button", { name: "Before" }).focus();

    await user.tab();

    expect(input()).toHaveFocus();
    expect(await screen.findByRole("listbox")).toBeVisible();
  });

  it("asks for the list again on every keystroke, and closes with no matches (no Add row)", async () => {
    const user = userEvent.setup();
    render(<Harness load={load} />);

    await user.type(input(), "9mm");
    expect(load).toHaveBeenLastCalledWith("9mm");
    await screen.findAllByRole("option");

    await user.clear(input());
    await user.type(input(), "zzz");
    await waitFor(() => expect(screen.queryByRole("listbox")).not.toBeInTheDocument());
    expect(screen.queryByText(/^Add /)).not.toBeInTheDocument();
    expect(input()).toHaveAttribute("aria-expanded", "false");
    expect(input()).toHaveValue("zzz");
  });

  it("moves the highlight with Down and Up, wrapping, without changing the text", async () => {
    const user = userEvent.setup();
    render(<Harness load={load} initial="9" />);
    await user.click(input());
    await screen.findAllByRole("option");
    const active = () => input().getAttribute("aria-activedescendant");
    const rows = () => screen.getAllByRole("option");

    expect(active()).toBeNull();
    await user.keyboard("{ArrowDown}");
    expect(active()).toBe(rows()[0].id);
    expect(rows()[0]).toHaveAttribute("aria-selected", "true");
    await user.keyboard("{ArrowDown}{ArrowDown}");
    expect(active()).toBe(rows()[2].id);
    await user.keyboard("{ArrowDown}");
    expect(active()).toBe(rows()[0].id);
    await user.keyboard("{ArrowUp}");
    expect(active()).toBe(rows()[2].id);
    expect(input()).toHaveValue("9");
  });

  it("opens a closed list with Down", async () => {
    const user = userEvent.setup();
    render(<Harness load={load} />);
    await user.click(input());
    await screen.findByRole("listbox");
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();

    await user.keyboard("{ArrowDown}");
    expect(await screen.findByRole("listbox")).toBeVisible();
  });

  it("picks the highlighted row with Enter, closes the list and keeps focus in the field", async () => {
    const user = userEvent.setup();
    const onPick = vi.fn();
    const onSubmit = vi.fn();
    render(<Harness load={load} onPick={onPick} onSubmit={onSubmit} />);
    await user.type(input(), "9x");
    await screen.findAllByRole("option");

    await user.keyboard("{ArrowDown}{ArrowDown}{Enter}");

    expect(onPick).toHaveBeenCalledWith("9x18mm Makarov");
    expect(input()).toHaveValue("9x18mm Makarov");
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(input()).toHaveFocus();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("lets the form's Enter through when nothing is highlighted", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    const onPick = vi.fn();
    render(<Harness load={load} onSubmit={onSubmit} onPick={onPick} />);
    await user.type(input(), "9x");
    await screen.findAllByRole("option");

    await user.keyboard("{Enter}");

    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onPick).not.toHaveBeenCalled();
    expect(input()).toHaveValue("9x");
  });

  it("closes an open list on Escape keeping the text, and lets Escape through once closed", async () => {
    const user = userEvent.setup();
    const onOpenChange = vi.fn();
    render(
      <Dialog open onOpenChange={onOpenChange} title="Add firearm">
        <Harness load={load} />
      </Dialog>,
    );
    await user.click(input());
    await screen.findByRole("listbox");
    await user.keyboard("9");

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(input()).toHaveValue("9");
    expect(onOpenChange).not.toHaveBeenCalled();

    await user.keyboard("{Escape}");
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  it("lets the wheel scroll the list inside a modal dialog", async () => {
    // A modal dialog's scroll lock cancels wheel and touch moves aimed outside
    // the dialog, and the list is portaled outside it.
    const user = userEvent.setup();
    render(
      <Dialog open onOpenChange={() => {}} title="Add firearm">
        <Harness load={load} />
      </Dialog>,
    );
    await user.click(input());
    const list = await screen.findByRole("listbox");
    for (const type of ["wheel", "touchmove"]) {
      const event = new Event(type, { bubbles: true, cancelable: true });
      list.querySelector("li")!.dispatchEvent(event);
      expect(event.defaultPrevented, type).toBe(false);
    }
  });

  it("closes on Tab keeping the text, and reports that the field was left", async () => {
    const user = userEvent.setup();
    const onLeave = vi.fn();
    render(<Harness load={load} onLeave={onLeave} />);
    await user.type(input(), "9x");
    await screen.findAllByRole("option");

    await user.tab();

    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(input()).toHaveValue("9x");
    expect(onLeave).toHaveBeenCalledTimes(1);
  });

  it("picks a row when it is clicked, keeping focus in the field", async () => {
    const user = userEvent.setup();
    const onPick = vi.fn();
    const onLeave = vi.fn();
    render(<Harness load={load} onPick={onPick} onLeave={onLeave} />);
    await user.click(input());

    await user.click(await screen.findByRole("option", { name: /^9x18mm Makarov/ }));

    expect(onPick).toHaveBeenCalledWith("9x18mm Makarov");
    expect(input()).toHaveValue("9x18mm Makarov");
    expect(input()).toHaveFocus();
    expect(onLeave).not.toHaveBeenCalled();
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  it("closes when the user clicks outside, keeping the text", async () => {
    const user = userEvent.setup();
    render(
      <div>
        <Harness load={load} initial="9" />
        <p>Elsewhere</p>
      </div>,
    );
    await user.click(input());
    await screen.findByRole("listbox");

    await user.click(screen.getByText("Elsewhere"));

    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
    expect(input()).toHaveValue("9");
  });

  it("drops a response for text other than the current text", async () => {
    // The slower answers for "" and "9" must not replace the answer for "9x".
    const user = userEvent.setup();
    const resolvers: Record<string, (options: ComboboxOption[]) => void> = {};
    const slow = vi.fn<Load>(
      (text) =>
        new Promise((resolve) => {
          resolvers[text] = resolve;
        }),
    );
    render(<Harness load={slow} />);
    await user.click(input());
    await user.type(input(), "9x");

    await act(async () => {
      resolvers["9x"]([{ value: "9x19mm Parabellum" }]);
    });
    expect(await screen.findAllByRole("option")).toHaveLength(1);

    await act(async () => {
      resolvers["9"]([{ value: "9mm Largo" }, { value: "9mm Steyr" }]);
      resolvers[""]([{ value: "Everything" }]);
    });
    expect(screen.getAllByRole("option")).toHaveLength(1);
    expect(screen.getByRole("option", { name: "9x19mm Parabellum" })).toBeVisible();
  });

  it("announces the number of suggestions politely once typing pauses", async () => {
    const user = userEvent.setup();
    render(<Harness load={load} />);
    const status = () => screen.getByRole("status");

    await user.type(input(), "9");
    await screen.findAllByRole("option");
    expect(status()).toHaveAttribute("aria-live", "polite");
    expect(status()).toBeEmptyDOMElement();

    await waitFor(() => expect(status()).toHaveTextContent("3 suggestions"), { timeout: 2000 });
  });

  it("announces a single suggestion in the singular", async () => {
    const one = vi.fn<Load>(async () => [OPTIONS[0]]);
    const user = userEvent.setup();
    render(<Harness load={one} />);
    await user.click(input());
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("1 suggestion"), {
      timeout: 2000,
    });
    expect(screen.getByRole("status")).not.toHaveTextContent("1 suggestions");
  });

  it("shows a note under the field and announces it", () => {
    const note = "Changed to “Smith & Wesson”, as already in your collection.";
    render(
      <Combobox
        label="Make"
        value="Smith & Wesson"
        onInputChange={() => {}}
        loadOptions={async () => []}
        onPick={() => {}}
        note={note}
      />,
    );
    expect(screen.getByRole("combobox", { name: "Make" })).toHaveAccessibleDescription(note);
    expect(screen.getByRole("status")).toHaveTextContent(note);
  });
});
