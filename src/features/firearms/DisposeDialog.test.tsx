import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ACCESSORY_KINDS } from "../../test/collectionFixtures";
import type { Accessory } from "../accessories/types";
import { CollectionContext } from "../app/collectionStore";
import type { CollectionState } from "../app/collectionStore";
import { currentDraft, getDirtyForm, setResumedDraft } from "../session/usePendingDraft";
import { DisposeDialog, FORM_VERSION } from "./DisposeDialog";
import { CommandFailure } from "../../services/tauriClient";
import type { MountDetail, RecordLabel } from "../mounts/types";
import type { DisposeInput, Firearm } from "./types";

const firearm = {
  id: 1,
  make: "Colt",
  model: "Python",
  status: "active",
  acquisitionDate: "2025-03-01",
} as Firearm;

function Harness({ onDispose }: { onDispose: (input: DisposeInput) => Promise<void> }) {
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
      formVersion: 2,
      kind: "accessory",
      mode: "dispose",
      targetId: 3,
      label: `${NAME} (disposal)`,
      values: { recipient: "Jane" },
    });
  });
});

// specs/006-accessory-links FR-014, US3, contracts/ui-accessories.md §7: the
// dialog says what happens to the records mounted on the record, and to the
// record itself if it is mounted.
describe("DisposeDialog with mounted records (US3)", () => {
  const label = (
    id: number,
    kind: "firearm" | "accessory",
    make: string,
    model: string,
    typeName: string,
  ): RecordLabel => ({
    record: { kind, id },
    make,
    model,
    nickname: null,
    typeName,
    serialNumber: null,
    status: "active",
  });
  const rifle = label(9, "firearm", "LaRue", "PredatAR", "Rifle");
  const upper = label(11, "accessory", "BCM", "upper", "Upper receiver");
  const optic = label(12, "accessory", "Leupold", "Mark 5HD", "Optic");
  const light = label(14, "accessory", "SureFire", "M600", "Light or laser");
  const cap = label(15, "accessory", "Leupold", "Flip cap", "Other");
  const NAME = "Colt Python";
  const KEEP = "Kept records mounted on Colt Python will be unmounted.";
  const ORPHAN = "Kept records mounted on a record disposed with it will be unmounted.";
  const STAY = "Records kept with what they are mounted on stay mounted.";
  const host = { kind: "firearm", id: 1 } as const;

  /** An optic and a light on the firearm, and a cap on the optic. */
  const carrying: MountDetail = {
    chain: [],
    mounted: [
      { label: optic, host, depth: 1 },
      { label: cap, host: optic.record, depth: 2 },
      { label: light, host, depth: 1 },
    ],
  };

  function renderDialog(
    mount: MountDetail,
    onDispose = vi.fn().mockResolvedValue(undefined),
    onMountChanged = vi.fn(),
  ) {
    const collection = {
      accessoryKinds: { kinds: ACCESSORY_KINDS },
      accessoryKindsFailed: false,
    } as unknown as CollectionState;
    render(
      <CollectionContext.Provider value={collection}>
        <DisposeDialog
          open
          onOpenChange={vi.fn()}
          firearm={firearm}
          mount={mount}
          onMountChanged={onMountChanged}
          onDispose={onDispose}
        />
      </CollectionContext.Provider>,
    );
    return { onDispose, onMountChanged };
  }

  const group = () => screen.getByRole("group", { name: "Mounted" });
  const row = (name: string) => within(group()).getByRole("radiogroup", { name });
  const OPTIC = "Leupold Mark 5HD · Optic";
  const LIGHT = "SureFire M600 · Light or laser";
  const CAP = "Leupold Flip cap · Other";

  async function fillOwn(user: ReturnType<typeof userEvent.setup>) {
    await user.click(screen.getByRole("radio", { name: "Sold" }));
    await user.type(screen.getByLabelText("Transferred to"), "Jane Doe");
    await user.type(screen.getByLabelText("Price received"), "1200");
  }

  it("says the record will be unmounted from its host under the title (US3-3)", () => {
    renderDialog({ chain: [upper, rifle], mounted: [] });

    expect(screen.getByRole("dialog")).toHaveTextContent(
      `${NAME} will be unmounted from BCM upper · Upper receiver.`,
    );
  });

  it("says nothing of a host, and has no Mounted group, for a record with neither", () => {
    renderDialog({ chain: [], mounted: [] });

    expect(screen.getByRole("dialog")).not.toHaveTextContent(/will be unmounted/);
    expect(screen.queryByRole("group", { name: "Mounted" })).not.toBeInTheDocument();
  });

  it("shows a Mounted group after the record's own price, listing everything below it", () => {
    renderDialog(carrying);

    const list = within(group()).getByRole("list");
    expect(within(list).getAllByRole("listitem")).toHaveLength(3);
    expect(list).toHaveTextContent(OPTIC);
    expect(list).toHaveTextContent(`on ${OPTIC}`);
    // The names are not links: the dialog must not be left.
    expect(within(list).queryByRole("button")).not.toBeInTheDocument();
    const price = screen.getByLabelText("Price received");
    expect(Boolean(price.compareDocumentPosition(group()) & Node.DOCUMENT_POSITION_FOLLOWING)).toBe(
      true,
    );
  });

  it("reads Keep | Dispose with it on every row, Keep selected", () => {
    renderDialog(carrying);

    for (const name of [OPTIC, CAP, LIGHT]) {
      expect(within(row(name)).getByRole("radio", { name: "Keep" })).toBeChecked();
      expect(within(row(name)).getByRole("radio", { name: "Dispose with it" })).not.toBeChecked();
    }
    expect(screen.queryByLabelText(/^Price for /)).not.toBeInTheDocument();
  });

  it("is one tab stop per row, changed by arrow keys", async () => {
    const user = userEvent.setup();
    renderDialog(carrying);

    within(row(OPTIC)).getByRole("radio", { name: "Keep" }).focus();
    await user.keyboard("{ArrowRight}");
    expect(within(row(OPTIC)).getByRole("radio", { name: "Dispose with it" })).toBeChecked();
    expect(within(row(OPTIC)).getByRole("radio", { name: "Dispose with it" })).toHaveFocus();

    // Tab leaves the row's group, and lands on its price field first.
    await user.tab();
    expect(screen.getByLabelText(`Price for ${OPTIC}`)).toHaveFocus();
    await user.tab();
    expect(within(row(CAP)).getByRole("radio", { name: "Keep" })).toHaveFocus();
    await user.tab();
    expect(within(row(LIGHT)).getByRole("radio", { name: "Keep" })).toHaveFocus();
  });

  it("asks for an optional price on a record disposed with it", async () => {
    const user = userEvent.setup();
    renderDialog(carrying);

    await user.click(within(row(OPTIC)).getByRole("radio", { name: "Dispose with it" }));

    const field = screen.getByLabelText(`Price for ${OPTIC}`);
    expect(field).not.toBeRequired();
    expect(screen.getByText("Leave blank if none was received separately.")).toBeInTheDocument();
    expect(field.closest(".hd-field")).toHaveClass("hd-field--third");
    expect(screen.queryByLabelText(`Price for ${LIGHT}`)).not.toBeInTheDocument();

    await user.click(within(row(OPTIC)).getByRole("radio", { name: "Keep" }));
    expect(screen.queryByLabelText(`Price for ${OPTIC}`)).not.toBeInTheDocument();
  });

  it("states what happens to kept records as the choices make it true", async () => {
    const user = userEvent.setup();
    renderDialog(carrying);

    // Everything kept: the direct ones are unmounted; the cap stays on the optic.
    expect(screen.getByText(KEEP)).toBeInTheDocument();
    expect(screen.getByText(STAY)).toBeInTheDocument();
    expect(screen.queryByText(ORPHAN)).not.toBeInTheDocument();

    // The optic goes: the cap, kept, loses its host, and the light is still kept.
    await user.click(within(row(OPTIC)).getByRole("radio", { name: "Dispose with it" }));
    expect(screen.getByText(KEEP)).toBeInTheDocument();
    expect(screen.getByText(ORPHAN)).toBeInTheDocument();
    expect(screen.queryByText(STAY)).not.toBeInTheDocument();

    // The light goes too: only the cap is left, unmounted from the optic.
    await user.click(within(row(LIGHT)).getByRole("radio", { name: "Dispose with it" }));
    expect(screen.queryByText(KEEP)).not.toBeInTheDocument();
    expect(screen.getByText(ORPHAN)).toBeInTheDocument();
    expect(screen.queryByText(STAY)).not.toBeInTheDocument();

    // Back to keeping the optic with the cap: the cap stays on it.
    await user.click(within(row(OPTIC)).getByRole("radio", { name: "Keep" }));
    expect(screen.getByText(KEEP)).toBeInTheDocument();
    expect(screen.getByText(STAY)).toBeInTheDocument();
    expect(screen.queryByText(ORPHAN)).not.toBeInTheDocument();
  });

  it("sends one call with withMounted, a blank price as null", async () => {
    const user = userEvent.setup();
    const { onDispose } = renderDialog(carrying);

    await fillOwn(user);
    await user.click(within(row(OPTIC)).getByRole("radio", { name: "Dispose with it" }));
    await user.type(screen.getByLabelText(`Price for ${OPTIC}`), "150");
    await user.click(within(row(LIGHT)).getByRole("radio", { name: "Dispose with it" }));
    await user.click(screen.getByRole("button", { name: "Mark as disposed" }));

    await waitFor(() => expect(onDispose).toHaveBeenCalledTimes(1));
    expect(onDispose).toHaveBeenCalledWith({
      dispositionType: "sold",
      recipient: "Jane Doe",
      date: expect.any(String),
      price: 1200,
      withMounted: [
        { record: optic.record, price: 150 },
        { record: light.record, price: null },
      ],
    });
  });

  it("sends an empty withMounted when everything is kept", async () => {
    const user = userEvent.setup();
    const { onDispose } = renderDialog(carrying);

    await fillOwn(user);
    await user.click(screen.getByRole("button", { name: "Mark as disposed" }));

    await waitFor(() => expect(onDispose).toHaveBeenCalledTimes(1));
    expect(onDispose).toHaveBeenCalledWith(expect.objectContaining({ withMounted: [] }));
  });

  describe("a record disposed with it acquired after the disposition date (FR-014, US3/AC2)", () => {
    // Acquired long after any date the dialog accepts.
    const acquiredLater: MountDetail = {
      chain: [],
      mounted: [
        { label: optic, host, depth: 1, acquisitionDate: "2999-01-01" },
        { label: light, host, depth: 1, acquisitionDate: "2020-01-01" },
      ],
    };
    const ORDER = "Disposition date can't be earlier than the acquisition date.";

    it("names the record and blocks the save, with nothing sent", async () => {
      const user = userEvent.setup();
      const { onDispose } = renderDialog(acquiredLater);

      await fillOwn(user);
      await user.click(within(row(OPTIC)).getByRole("radio", { name: "Dispose with it" }));
      await user.click(within(row(LIGHT)).getByRole("radio", { name: "Dispose with it" }));
      await user.click(screen.getByRole("button", { name: "Mark as disposed" }));

      expect(await screen.findByRole("alert")).toHaveTextContent(`${OPTIC}: ${ORDER}`);
      // Only the record that fails says so.
      expect(screen.getAllByRole("alert")).toHaveLength(1);
      expect(onDispose).not.toHaveBeenCalled();
    });

    it("says nothing about a record that is kept, and saves", async () => {
      const user = userEvent.setup();
      const { onDispose } = renderDialog(acquiredLater);

      await fillOwn(user);
      await user.click(within(row(LIGHT)).getByRole("radio", { name: "Dispose with it" }));
      await user.click(screen.getByRole("button", { name: "Mark as disposed" }));

      await waitFor(() => expect(onDispose).toHaveBeenCalledTimes(1));
      expect(screen.queryByText(new RegExp(ORDER))).not.toBeInTheDocument();
    });

    it("shows the backend's own failure for a record the dialog could not judge", async () => {
      const user = userEvent.setup();
      const message = `${OPTIC}: ${ORDER}`;
      const onDispose = vi.fn().mockRejectedValue(
        new CommandFailure({
          code: "VALIDATION_ERROR",
          message,
          fieldErrors: { withMounted: message },
        }),
      );
      // The list carried no acquisition date, as an older payload would.
      renderDialog(carrying, onDispose);

      await fillOwn(user);
      await user.click(within(row(OPTIC)).getByRole("radio", { name: "Dispose with it" }));
      await user.click(screen.getByRole("button", { name: "Mark as disposed" }));

      expect(await screen.findByRole("alert")).toHaveTextContent(message);
    });
  });

  it("shows the stale-mount error and reloads the list", async () => {
    const user = userEvent.setup();
    const stale = "What is mounted has changed. Close the dialog and try again.";
    const onDispose = vi.fn().mockRejectedValue(
      new CommandFailure({
        code: "VALIDATION_ERROR",
        message: "The input isn't valid.",
        fieldErrors: { withMounted: stale },
      }),
    );
    const { onMountChanged } = renderDialog(carrying, onDispose);

    await fillOwn(user);
    await user.click(within(row(OPTIC)).getByRole("radio", { name: "Dispose with it" }));
    await user.click(screen.getByRole("button", { name: "Mark as disposed" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(stale);
    expect(onMountChanged).toHaveBeenCalledTimes(1);
  });

  it("keeps the choices and prices in its draft, and resumes them (FR-027)", async () => {
    const user = userEvent.setup();
    renderDialog(carrying);

    await user.click(within(row(OPTIC)).getByRole("radio", { name: "Dispose with it" }));
    await user.type(screen.getByLabelText(`Price for ${OPTIC}`), "150");

    expect(currentDraft()).toMatchObject({
      formVersion: 2,
      values: { disposeWith: { "accessory:12": "150" } },
    });
  });

  it("is version 2, and discards a version 1 draft", () => {
    expect(FORM_VERSION).toBe(2);
    setResumedDraft({
      formVersion: 1,
      kind: "firearm",
      mode: "dispose",
      targetId: 1,
      label: `${NAME} (disposal)`,
      values: { dispositionType: "sold", recipient: "Old", date: "2025-04-01", price: "5" },
    });
    renderDialog({ chain: [], mounted: [] });

    expect(screen.getByLabelText("Transferred to")).toHaveValue("");
    setResumedDraft(null);
  });

  it("starts from a resumed version 2 draft's choices", () => {
    setResumedDraft({
      formVersion: 2,
      kind: "firearm",
      mode: "dispose",
      targetId: 1,
      label: `${NAME} (disposal)`,
      values: {
        dispositionType: "sold",
        recipient: "Jane",
        date: "2025-04-01",
        price: "5",
        disposeWith: { "accessory:12": "150" },
      },
    });
    renderDialog(carrying);

    expect(within(row(OPTIC)).getByRole("radio", { name: "Dispose with it" })).toBeChecked();
    expect(screen.getByLabelText(`Price for ${OPTIC}`)).toHaveValue("150");
    expect(within(row(LIGHT)).getByRole("radio", { name: "Keep" })).toBeChecked();
    setResumedDraft(null);
  });
});
