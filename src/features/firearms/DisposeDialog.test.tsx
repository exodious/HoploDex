import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ACCESSORY_KINDS } from "../../test/collectionFixtures";
import type { Accessory } from "../accessories/types";
import { CollectionContext } from "../app/collectionStore";
import type { CollectionState } from "../app/collectionStore";
import { currentDraft, getDirtyForm } from "../session/usePendingDraft";
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
      expect.objectContaining({ dispositionType: "sold", recipient: "Jane Doe", price: 1200 }),
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

// specs/006-accessory-links FR-006, FR-009, FR-027, contracts/ui-accessories.md
// §7: the dialog serves a record of either kind. Given an accessory (in
// place of the firearm) it names it by `RecordName`, hands the same input to
// `onDispose`, which the accessory's page sends as `dispose_accessory`, and
// keeps its draft as `kind: "accessory"`.
describe("DisposeDialog for an accessory", () => {
  const accessory = {
    id: 3,
    accessoryKindId: 1,
    make: "Leupold",
    model: "VX-5HD 3-15x44",
    status: "active",
    acquisitionDate: "2025-03-01",
  } as Accessory;
  const NAME = "Leupold VX-5HD 3-15x44 · Optic";

  const collection = {
    firearms: [],
    firearmsById: new Map(),
    accessories: [],
    accessoriesById: new Map(),
    policies: [],
    policiesById: new Map(),
    summary: null,
    accessoryKinds: { kinds: ACCESSORY_KINDS },
    accessoryKindsFailed: false,
    loaded: true,
    error: null,
    revision: 1,
    refresh: async () => {},
  } as unknown as CollectionState;

  function renderDialog(onDispose = vi.fn().mockResolvedValue(undefined)) {
    render(
      <CollectionContext.Provider value={collection}>
        <DisposeDialog open onOpenChange={vi.fn()} accessory={accessory} onDispose={onDispose} />
      </CollectionContext.Provider>,
    );
    return onDispose;
  }

  it("names the accessory by its make, model and kind", () => {
    renderDialog();

    expect(screen.getByRole("dialog")).toHaveTextContent(NAME);
    expect(screen.getByRole("dialog")).not.toHaveTextContent(/firearm/i);
  });

  it("hands the same input as a firearm's to onDispose", async () => {
    const user = userEvent.setup();
    const onDispose = renderDialog();

    await user.click(screen.getByRole("radio", { name: "Sold" }));
    await user.type(screen.getByLabelText("Transferred to"), "Jane Doe");
    await user.type(screen.getByLabelText("Price received"), "800");
    await user.click(screen.getByRole("button", { name: "Mark as disposed" }));

    expect(onDispose).toHaveBeenCalledWith(
      expect.objectContaining({ dispositionType: "sold", recipient: "Jane Doe", price: 800 }),
    );
  });

  it("blocks a disposition date before the accessory was acquired", async () => {
    const user = userEvent.setup();
    const onDispose = renderDialog();

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

  it("says 'accessory', not 'firearm', when the save fails", async () => {
    const user = userEvent.setup();
    renderDialog(vi.fn().mockRejectedValue(new Error("boom")));

    await user.click(screen.getByRole("radio", { name: "Sold" }));
    await user.type(screen.getByLabelText("Transferred to"), "Jane Doe");
    await user.type(screen.getByLabelText("Price received"), "100");
    await user.click(screen.getByRole("button", { name: "Mark as disposed" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "The accessory couldn't be marked disposed.",
    );
  });

  it("keeps its unsaved input as a draft of the accessory (FR-027)", async () => {
    const user = userEvent.setup();
    renderDialog();
    expect(getDirtyForm()).toBeNull();

    await user.type(screen.getByLabelText("Transferred to"), "Jane");

    expect(getDirtyForm()?.label).toBe(`${NAME} (disposal)`);
    expect(currentDraft()).toMatchObject({
      formVersion: 1,
      kind: "accessory",
      mode: "dispose",
      targetId: 3,
      label: `${NAME} (disposal)`,
      values: { recipient: "Jane" },
    });
  });
});
