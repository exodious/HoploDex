import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render as renderBase, screen, waitFor, within } from "@testing-library/react";
import type { ReactElement } from "react";
import userEvent from "@testing-library/user-event";
import { ACCESSORY_KINDS } from "../../test/collectionFixtures";
import { CollectionContext } from "../app/collectionStore";
import type { CollectionState } from "../app/collectionStore";
import { CommandFailure } from "../../services/tauriClient";
import { currentDraft, getDirtyForm, setResumedDraft } from "../session/usePendingDraft";
import type { DerivedCaliber } from "../firearms/types";
import { AccessoryForm, FORM_VERSION } from "./AccessoryForm";
import type { Accessory, AccessoryDetail, AccessoryKind } from "./types";
import type { RecordLabel } from "../mounts/types";

// specs/006-accessory-links User Story 1 (contracts/ui-accessories.md §3,
// FR-001 to FR-005, FR-027). The form is a bare form like `FirearmForm`: the
// record page and the shell put it in a large `Dialog` titled "Add accessory"
// or "Edit {name}" (tested with them). The Mounted on row is User Story 2's,
// tested at the end.

// The kinds come from the collection store (`useAccessoryKinds`), as the
// firearm types do; each render gets the seeded twelve unless a test says.
function collectionWith(kinds: AccessoryKind[]): CollectionState {
  return {
    accessoryKinds: { kinds },
    accessoryKindsFailed: false,
  } as unknown as CollectionState;
}

function render(ui: ReactElement, kinds: AccessoryKind[] = ACCESSORY_KINDS) {
  return renderBase(ui, {
    wrapper: ({ children }) => (
      <CollectionContext.Provider value={collectionWith(kinds)}>
        {children}
      </CollectionContext.Provider>
    ),
  });
}

// 004's make, model, cartridge and caliber fields settle and suggest through
// the entries commands, which accessories share (FR-003): the same
// stand-ins as FirearmForm.test.tsx.
const settleEntry = vi.fn();
const suggestEntries = vi.fn();
vi.mock("../firearms/firearmsService", () => ({
  settleEntry: (field: string, text: string) => settleEntry(field, text),
  suggestEntries: (field: string, text: string, make?: string | null) =>
    suggestEntries(field, text, make),
}));

// specs/006-accessory-links US2: the Mounted on chooser searches the mount
// candidates through the mounts service.
const listMountCandidates = vi.fn();
vi.mock("../mounts/mountsService", () => ({
  listMountCandidates: (input: unknown) => listMountCandidates(input),
}));

const DERIVED: Record<string, DerivedCaliber> = {
  "5.56x45mm NATO": { caliber: "5.56mm", source: "catalog" },
};

beforeEach(() => {
  listMountCandidates.mockReset();
  listMountCandidates.mockResolvedValue({ candidates: [] });
  suggestEntries.mockReset();
  suggestEntries.mockResolvedValue([]);
  settleEntry.mockReset();
  settleEntry.mockImplementation(async (field: string, text: string) => ({
    value: text.trim(),
    changedBy: null,
    derivedCaliber: field === "cartridge" ? (DERIVED[text.trim()] ?? null) : null,
  }));
});

afterEach(() => setResumedDraft(null));

const kindField = () => screen.getByRole("combobox", { name: /^Kind/ });

async function chooseKind(user: ReturnType<typeof userEvent.setup>, name: string) {
  await user.click(kindField());
  await user.click(await screen.findByRole("option", { name }));
}

/** Types the required make and model (FR-001), for a test about the rest. */
async function nameIt(user: ReturnType<typeof userEvent.setup>) {
  await user.type(screen.getByLabelText(/^Make/), "Magpul");
  await user.type(screen.getByLabelText(/^Model/), "MS1");
}

const saved: Accessory = {
  id: 3,
  accessoryKindId: 1,
  make: "Leupold",
  model: "VX-5HD 3-15x44",
  serialNumber: "L-5521",
  caliber: null,
  cartridge: null,
  notes: "Zeroed at 100 yards.",
  status: "active",
  estimatedValue: 1000,
  acquisitionSource: "Optics Planet",
  acquisitionDate: "2025-03-01",
  acquisitionPrice: 900,
  dispositionType: null,
  dispositionRecipient: null,
  dispositionDate: null,
  dispositionPrice: null,
  insurancePolicyId: null,
  scheduledCoverageAmount: null,
  mountedOn: null,
  thumbnailPhotoId: null,
  createdAt: "2025-01-01 00:00:00",
  updatedAt: "2025-01-01 00:00:00",
};

describe("AccessoryForm layout (contracts/ui-accessories.md §3)", () => {
  it("lays out Kind, Make and Model, Cartridge and Caliber, Serial number, Estimated value, Acquired, Notes", () => {
    render(<AccessoryForm onSubmit={vi.fn()} />);

    const rows: HTMLElement[] = [
      kindField(),
      screen.getByLabelText(/^Make/),
      screen.getByLabelText(/^Model/),
      screen.getByLabelText(/^Cartridge/),
      screen.getByLabelText(/^Caliber/),
      screen.getByLabelText(/^Serial number/),
      screen.getByLabelText(/^Estimated value/),
      screen.getByLabelText(/^Acquired from/),
      screen.getByLabelText(/^Date acquired/),
      screen.getByLabelText(/^Price paid/),
      screen.getByLabelText(/^Notes/),
    ];
    for (let i = 1; i < rows.length; i++) {
      expect(
        rows[i - 1].compareDocumentPosition(rows[i]) & Node.DOCUMENT_POSITION_FOLLOWING,
        `${i}`,
      ).toBeTruthy();
    }
  });

  it("puts Make and Model together, and Cartridge and Caliber together, as the firearm form does", () => {
    render(<AccessoryForm onSubmit={vi.fn()} />);

    const makeRow = screen.getByLabelText(/^Make/).closest(".hd-form-grid--2");
    expect(makeRow).not.toBeNull();
    expect(makeRow).toContainElement(screen.getByLabelText(/^Model/));
    const cartridgeRow = screen.getByLabelText(/^Cartridge/).closest(".hd-form-grid--2");
    expect(cartridgeRow).not.toBeNull();
    expect(cartridgeRow).toContainElement(screen.getByLabelText(/^Caliber/));
  });

  it("gives Serial number a third and Estimated value a quarter of the row", () => {
    render(<AccessoryForm onSubmit={vi.fn()} />);

    expect(screen.getByLabelText(/^Serial number/).closest(".hd-field")).toHaveClass(
      "hd-field--third",
    );
    expect(screen.getByLabelText(/^Estimated value/).closest(".hd-field")).toHaveClass(
      "hd-field--quarter",
    );
  });

  it("has no quantity field, and no 'no serial number' box (US1-4, FR-004)", () => {
    render(<AccessoryForm onSubmit={vi.fn()} />);

    expect(screen.queryByLabelText(/quantity|count|how many/i)).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/no serial number/i)).not.toBeInTheDocument();
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
    expect(screen.getByLabelText(/^Serial number/)).toBeEnabled();
    expect(screen.getByLabelText(/^Serial number/)).not.toBeRequired();
  });

  it("offers Save and Cancel", async () => {
    const user = userEvent.setup();
    const onCancel = vi.fn();
    render(<AccessoryForm onSubmit={vi.fn()} onCancel={onCancel} />);

    expect(screen.getByRole("button", { name: "Save" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });
});

describe("AccessoryForm hints (FR-002, FR-005)", () => {
  it("says under Kind that a suppressor is recorded as a firearm", () => {
    render(<AccessoryForm onSubmit={vi.fn()} />);
    expect(kindField()).toHaveAccessibleDescription(
      expect.stringContaining("A suppressor is recorded as a firearm."),
    );
  });

  it("explains that a value covers the record as a whole", () => {
    render(<AccessoryForm onSubmit={vi.fn()} />);
    expect(screen.getByLabelText(/^Estimated value/)).toHaveAccessibleDescription(
      expect.stringContaining(
        "The value of this accessory as a whole, everything it describes included. Leave out any firearm or accessory recorded separately, such as one mounted on it.",
      ),
    );
  });
});

describe("AccessoryForm kind (US1, FR-001, FR-002)", () => {
  it("is a required choice, with nothing chosen on a new record", () => {
    render(<AccessoryForm onSubmit={vi.fn()} />);

    expect(screen.getByText("Kind", { selector: "label" })).toHaveAttribute("data-required");
    expect(kindField()).not.toHaveTextContent("Optic");
  });

  it("offers the offered kinds in list order", async () => {
    const user = userEvent.setup();
    render(<AccessoryForm onSubmit={vi.fn()} />);

    await user.click(kindField());

    const options = (await screen.findAllByRole("option")).map((o) => o.textContent);
    expect(options).toEqual(ACCESSORY_KINDS.map((kind) => kind.name));
    expect(options[0]).toBe("Optic");
    expect(options[options.length - 1]).toBe("Other");
  });

  it("does not offer a kind that is no longer offered, to a new record", async () => {
    const user = userEvent.setup();
    const kinds = ACCESSORY_KINDS.map((kind) =>
      kind.name === "Conversion kit" ? { ...kind, offered: false } : kind,
    );
    render(<AccessoryForm onSubmit={vi.fn()} />, kinds);

    await user.click(kindField());

    const options = (await screen.findAllByRole("option")).map((o) => o.textContent);
    expect(options).not.toContain("Conversion kit");
    expect(options).toHaveLength(ACCESSORY_KINDS.length - 1);
  });

  it("shows a saved record's kind as selected even when it is no longer offered", async () => {
    const user = userEvent.setup();
    const kinds = ACCESSORY_KINDS.map((kind) =>
      kind.name === "Conversion kit" ? { ...kind, offered: false } : kind,
    );
    const conversion = kinds.find((kind) => kind.name === "Conversion kit")!;
    render(
      <AccessoryForm
        initialValues={{ ...saved, accessoryKindId: conversion.id }}
        onSubmit={vi.fn()}
      />,
      kinds,
    );

    expect(kindField()).toHaveTextContent("Conversion kit");

    await user.click(kindField());
    const options = await screen.findAllByRole("option");
    const selected = options.find((o) => o.getAttribute("aria-selected") === "true");
    expect(selected).toHaveTextContent("Conversion kit");
  });

  it("submits the chosen kind's id", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<AccessoryForm onSubmit={onSubmit} />);

    await chooseKind(user, "Magazine");
    await nameIt(user);
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit.mock.calls[0][0].accessoryKindId).toBe(
      ACCESSORY_KINDS.find((kind) => kind.name === "Magazine")!.id,
    );
  });

  it("focuses Kind and says 'Choose a kind.' on a save with none chosen", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<AccessoryForm onSubmit={onSubmit} />);

    await user.type(screen.getByLabelText(/^Make/), "Walther");
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(screen.getByText("Choose a kind.")).toBeInTheDocument();
    await waitFor(() => expect(kindField()).toHaveFocus());
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("clears the message once a kind is chosen", async () => {
    const user = userEvent.setup();
    render(<AccessoryForm onSubmit={vi.fn().mockResolvedValue(undefined)} />);

    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(screen.getByText("Choose a kind.")).toBeInTheDocument();

    await chooseKind(user, "Sling");
    expect(screen.queryByText("Choose a kind.")).not.toBeInTheDocument();
  });
});

describe("AccessoryForm saving (US1-1, US1-2)", () => {
  it("saves with only a kind, make and model: every other field is optional", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<AccessoryForm onSubmit={onSubmit} />);

    await chooseKind(user, "Sling");
    await user.type(screen.getByLabelText(/^Make/), "Magpul");
    await user.type(screen.getByLabelText(/^Model/), "MS1");
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
    expect(onSubmit.mock.calls[0][0]).toMatchObject({
      make: "Magpul",
      model: "MS1",
      serialNumber: null,
      caliber: null,
      cartridge: null,
      notes: null,
      status: "active",
      estimatedValue: null,
      acquisitionSource: null,
      acquisitionDate: null,
      acquisitionPrice: null,
      insurancePolicyId: null,
      scheduledCoverageAmount: null,
      mountedOn: null,
    });
  });

  it("requires a make and a model, as a firearm's form does (FR-001)", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    render(<AccessoryForm onSubmit={onSubmit} />);

    expect(screen.getByLabelText(/^Make/)).toBeRequired();
    expect(screen.getByLabelText(/^Model/)).toBeRequired();

    await chooseKind(user, "Sling");
    await user.type(screen.getByLabelText(/^Model/), "   ");
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(onSubmit).not.toHaveBeenCalled();
    expect(screen.getByText("Enter the make.")).toBeInTheDocument();
    expect(screen.getByText("Enter the model.")).toBeInTheDocument();
    expect(screen.getByLabelText(/^Make/)).toHaveFocus();
  });

  it("submits trimmed text and whole dollars", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<AccessoryForm onSubmit={onSubmit} />);

    await chooseKind(user, "Optic");
    await user.type(screen.getByLabelText(/^Make/), " Leupold ");
    await user.type(screen.getByLabelText(/^Model/), "VX-5HD 3-15x44");
    await user.type(screen.getByLabelText(/^Serial number/), " L-5521 ");
    await user.type(screen.getByLabelText(/^Estimated value/), "1000");
    await user.type(screen.getByLabelText(/^Acquired from/), "Optics Planet");
    await user.type(screen.getByLabelText(/^Price paid/), "900");
    await user.type(screen.getByLabelText(/^Notes/), "Zeroed at 100 yards.");
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit.mock.calls[0][0]).toMatchObject({
      accessoryKindId: ACCESSORY_KINDS.find((kind) => kind.name === "Optic")!.id,
      make: "Leupold",
      model: "VX-5HD 3-15x44",
      serialNumber: "L-5521",
      estimatedValue: 1000,
      acquisitionSource: "Optics Planet",
      acquisitionPrice: 900,
      notes: "Zeroed at 100 yards.",
    });
  });

  it("refuses an amount above $99,999,999 before sending, and accepts exactly that (#67)", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<AccessoryForm onSubmit={onSubmit} />);

    await chooseKind(user, "Sling");
    await user.type(screen.getByLabelText(/^Make/), "Vortex");
    await user.type(screen.getByLabelText(/^Model/), "Padded");
    await user.type(screen.getByLabelText(/^Estimated value/), "100000000");
    await user.type(screen.getByLabelText(/^Price paid/), "100000000");
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(screen.getAllByText("Enter an amount of $99,999,999 or less.")).toHaveLength(2);
    expect(onSubmit).not.toHaveBeenCalled();

    for (const label of [/^Estimated value/, /^Price paid/]) {
      const field = screen.getByLabelText(label);
      await user.clear(field);
      await user.type(field, "99999999");
    }
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onSubmit.mock.calls[0][0]).toMatchObject({
      estimatedValue: 99_999_999,
      acquisitionPrice: 99_999_999,
    });
  });

  it("prefills every field when editing", () => {
    render(<AccessoryForm initialValues={saved} onSubmit={vi.fn()} />);

    expect(kindField()).toHaveTextContent("Optic");
    expect(screen.getByLabelText(/^Make/)).toHaveValue("Leupold");
    expect(screen.getByLabelText(/^Model/)).toHaveValue("VX-5HD 3-15x44");
    expect(screen.getByLabelText(/^Serial number/)).toHaveValue("L-5521");
    expect(screen.getByLabelText(/^Estimated value/)).toHaveValue("1000");
    expect(screen.getByLabelText(/^Acquired from/)).toHaveValue("Optics Planet");
    expect(screen.getByLabelText(/^Price paid/)).toHaveValue("900");
    expect(screen.getByLabelText(/^Notes/)).toHaveValue("Zeroed at 100 yards.");
  });

  it("blocks a future acquisition date, as the firearm form does", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<AccessoryForm onSubmit={onSubmit} />);

    await chooseKind(user, "Sling");
    const next = new Date();
    next.setUTCFullYear(next.getUTCFullYear() + 1);
    await user.type(screen.getByLabelText(/^Date acquired/), next.toISOString().slice(0, 10));
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(screen.getByText("Acquisition date can't be in the future.")).toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("shows the backend's field errors on the fields, as the firearm form does", async () => {
    const user = userEvent.setup();
    const message = "Estimated value must be 0 or more.";
    const onSubmit = vi.fn().mockRejectedValue(
      new CommandFailure({
        code: "VALIDATION_ERROR",
        message,
        fieldErrors: { estimatedValue: message },
      }),
    );
    render(<AccessoryForm onSubmit={onSubmit} />);

    await chooseKind(user, "Sling");
    await nameIt(user);
    await user.click(screen.getByRole("button", { name: "Save" }));

    const error = await screen.findByText(message);
    expect(error).toBeInTheDocument();
    expect(screen.getByLabelText(/^Estimated value/)).toHaveAccessibleDescription(
      expect.stringContaining(message),
    );
  });
});

// specs/004-cartridges-action-types, reused by specs/006 US1-6: a blank
// caliber is filled from the cartridge, with 004's hints.
describe("AccessoryForm cartridge and caliber (US1-6)", () => {
  const cartridgeField = () => screen.getByLabelText(/^Cartridge/);
  const caliberField = () => screen.getByLabelText(/^Caliber/);

  it("fills an empty Caliber from the cartridge '5.56x45mm NATO' with 004's hint", async () => {
    const user = userEvent.setup();
    render(<AccessoryForm onSubmit={vi.fn()} />);

    await user.type(cartridgeField(), "5.56x45mm NATO");
    await user.tab();

    await waitFor(() => expect(caliberField()).toHaveValue("5.56mm"));
    expect(settleEntry).toHaveBeenCalledWith("cartridge", "5.56x45mm NATO");
    expect(screen.getByText("From the cartridge.")).toBeInTheDocument();
  });

  it("leaves a caliber the user entered alone", async () => {
    const user = userEvent.setup();
    render(<AccessoryForm onSubmit={vi.fn()} />);

    await user.type(caliberField(), ".223");
    await user.type(cartridgeField(), "5.56x45mm NATO");
    await user.tab();

    await waitFor(() => expect(settleEntry).toHaveBeenCalledWith("cartridge", "5.56x45mm NATO"));
    expect(caliberField()).toHaveValue(".223");
    expect(screen.queryByText("From the cartridge.")).not.toBeInTheDocument();
  });

  it("suggests make, model, cartridge and caliber through the shared entry commands (FR-003)", async () => {
    const user = userEvent.setup();
    render(<AccessoryForm onSubmit={vi.fn()} />);

    await user.type(screen.getByLabelText(/^Make/), "Leu");

    await waitFor(() => expect(suggestEntries).toHaveBeenCalledWith("make", "Leu", undefined));
  });

  it("submits the derived caliber and the cartridge", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<AccessoryForm onSubmit={onSubmit} />);

    await chooseKind(user, "Magazine");
    await nameIt(user);
    await user.type(cartridgeField(), "5.56x45mm NATO");
    await user.tab();
    await waitFor(() => expect(caliberField()).toHaveValue("5.56mm"));
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(onSubmit.mock.calls[0][0]).toMatchObject({
      cartridge: "5.56x45mm NATO",
      caliber: "5.56mm",
    });
  });
});

// specs/003 FR-010 and FR-039 through the shared pending-draft hook: closing
// with changes asks save, discard or cancel (the session layer asks, using
// the label and submit the form registers), and a lock keeps the input as
// pending changes labelled with the accessory's name (FR-027).
describe("AccessoryForm unsaved changes (FR-027)", () => {
  it("is not dirty until something changes", () => {
    render(<AccessoryForm initialValues={saved} onSubmit={vi.fn()} />);
    expect(getDirtyForm()).toBeNull();
    expect(currentDraft()).toBeNull();
  });

  it("registers a new accessory's unsaved input as 'New accessory'", async () => {
    const user = userEvent.setup();
    render(<AccessoryForm onSubmit={vi.fn()} />);

    await user.type(screen.getByLabelText(/^Make/), "Walther");

    expect(getDirtyForm()?.label).toBe("New accessory");
    expect(currentDraft()).toMatchObject({
      formVersion: FORM_VERSION,
      kind: "accessory",
      mode: "add",
      targetId: null,
      label: "New accessory",
    });
  });

  it("labels an edit by the accessory's name", async () => {
    const user = userEvent.setup();
    render(<AccessoryForm initialValues={saved} onSubmit={vi.fn()} />);

    await user.type(screen.getByLabelText(/^Notes/), " More.");

    expect(getDirtyForm()?.label).toBe("Leupold VX-5HD 3-15x44 · Optic (edit)");
    expect(currentDraft()).toMatchObject({
      formVersion: FORM_VERSION,
      kind: "accessory",
      mode: "edit",
      targetId: 3,
      label: "Leupold VX-5HD 3-15x44 · Optic (edit)",
    });
  });

  it("keeps the draft's values, so a lock can write them as pending changes", async () => {
    const user = userEvent.setup();
    render(<AccessoryForm onSubmit={vi.fn()} />);

    await user.type(screen.getByLabelText(/^Make/), "Walther");

    const values = currentDraft()?.values as Record<string, unknown>;
    expect(values.make).toBe("Walther");
  });

  it("saves through the form's own submit when the question is answered Save", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<AccessoryForm onSubmit={onSubmit} />);

    await chooseKind(user, "Sling");
    await nameIt(user);
    expect(getDirtyForm()).not.toBeNull();

    await expect(getDirtyForm()!.submit()).resolves.toBe(true);
    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit.mock.calls[0][0].accessoryKindId).toBe(10);
  });

  it("answers false from submit when the form can't be saved, leaving it showing why", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    render(<AccessoryForm onSubmit={onSubmit} />);

    await user.type(screen.getByLabelText(/^Make/), "Walther");

    await expect(getDirtyForm()!.submit()).resolves.toBe(false);
    expect(onSubmit).not.toHaveBeenCalled();
    expect(await screen.findByText("Choose a kind.")).toBeInTheDocument();
  });

  it("opens with resumed pending changes as its unsaved input", () => {
    setResumedDraft({
      formVersion: FORM_VERSION,
      kind: "accessory",
      mode: "edit",
      targetId: 3,
      label: "Leupold VX-5HD 3-15x44 · Optic (edit)",
      values: { make: "Leupold", notes: "Swapped the mount; half typed" },
    });

    render(<AccessoryForm initialValues={saved} onSubmit={vi.fn()} />);

    expect(screen.getByLabelText(/^Notes/)).toHaveValue("Swapped the mount; half typed");
    // A field the draft doesn't hold takes the record's value.
    expect(screen.getByLabelText(/^Model/)).toHaveValue("VX-5HD 3-15x44");
    expect(getDirtyForm()?.label).toBe("Leupold VX-5HD 3-15x44 · Optic (edit)");
  });

  it("ignores a draft of another form version or another record", () => {
    setResumedDraft({
      formVersion: FORM_VERSION + 1,
      kind: "accessory",
      mode: "edit",
      targetId: 3,
      label: "Leupold (edit)",
      values: { notes: "From a newer form" },
    });

    render(<AccessoryForm initialValues={saved} onSubmit={vi.fn()} />);

    expect(screen.getByLabelText(/^Notes/)).toHaveValue("Zeroed at 100 yards.");
  });

  it("is the first version of its drafts", () => {
    expect(FORM_VERSION).toBe(1);
  });
});

describe("AccessoryForm structure", () => {
  it("is a form named by its fields, with the kind first", () => {
    render(<AccessoryForm onSubmit={vi.fn()} />);

    const form = kindField().closest("form");
    expect(form).not.toBeNull();
    const first = within(form!).getAllByRole("combobox")[0];
    expect(first).toBe(kindField());
  });
});

// specs/006-accessory-links User Story 2 (contracts/ui-accessories.md §3
// row 5, §4; FR-010, FR-012, US2-4). A saved accessory's current host arrives
// with its detail (`mount.chain[0]`), which is what the record page passes as
// `initialValues`; `presetMountedOn` (a `RecordLabel`) is the record whose
// "Mount > New accessory…" opened the form. The form sends only the host's
// `RecordRef` as `mountedOn`.
describe("AccessoryForm Mounted on (US2)", () => {
  const HINT = "The firearm or accessory it is on now, if any.";

  const rifle: RecordLabel = {
    record: { kind: "firearm", id: 9 },
    make: "LaRue",
    model: "PredatAR",
    nickname: null,
    typeName: "Rifle",
    serialNumber: "L-9",
    status: "active",
  };
  const upper: RecordLabel = {
    record: { kind: "accessory", id: 11 },
    make: "BCM",
    model: "upper",
    nickname: null,
    typeName: "Upper receiver",
    serialNumber: null,
    status: "active",
  };

  const mountedOnField = () => screen.getByRole("combobox", { name: /^Mounted on/ });

  const mountedOptic: AccessoryDetail = {
    ...saved,
    dispositionHistory: [],
    mountedOn: upper.record,
    mount: { chain: [upper, rifle], mounted: [] },
  };

  it("is row 5, after Serial number and before Estimated value", () => {
    render(<AccessoryForm onSubmit={vi.fn()} />);

    const follows = (a: HTMLElement, b: HTMLElement) =>
      Boolean(a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING);
    expect(follows(screen.getByLabelText(/^Serial number/), mountedOnField())).toBe(true);
    expect(follows(mountedOnField(), screen.getByLabelText(/^Estimated value/))).toBe(true);
  });

  it("gives the hint 'The firearm or accessory it is on now, if any.'", () => {
    render(<AccessoryForm onSubmit={vi.fn()} />);
    expect(mountedOnField()).toHaveAccessibleDescription(expect.stringContaining(HINT));
  });

  it("reads 'Not mounted' on a new accessory and submits mountedOn null", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<AccessoryForm onSubmit={onSubmit} />);

    expect(mountedOnField()).toHaveAttribute("placeholder", "Not mounted");
    await chooseKind(user, "Sling");
    await nameIt(user);
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(onSubmit.mock.calls[0][0].mountedOn).toBeNull();
  });

  it("searches the hosts, leaving out this accessory when editing", async () => {
    const user = userEvent.setup();
    render(<AccessoryForm initialValues={saved} onSubmit={vi.fn()} />);

    await user.click(mountedOnField());

    await waitFor(() =>
      expect(listMountCandidates).toHaveBeenLastCalledWith(
        expect.objectContaining({ role: "host", record: { kind: "accessory", id: 3 } }),
      ),
    );
  });

  it("sends the chosen host's RecordRef as mountedOn on save", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    listMountCandidates.mockResolvedValue({ candidates: [{ label: rifle, mountedOn: null }] });
    render(<AccessoryForm onSubmit={onSubmit} />);

    await chooseKind(user, "Optic");
    await nameIt(user);
    await user.click(mountedOnField());
    await user.click(await screen.findByRole("option", { name: /LaRue PredatAR/ }));
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(onSubmit.mock.calls[0][0].mountedOn).toEqual({ kind: "firearm", id: 9 });
  });

  it("shows the current host when editing and keeps it on save", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<AccessoryForm initialValues={mountedOptic} onSubmit={onSubmit} />);

    expect(mountedOnField()).toHaveDisplayValue(/BCM upper · Upper receiver/);
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(onSubmit.mock.calls[0][0].mountedOn).toEqual({ kind: "accessory", id: 11 });
  });

  it("sends mountedOn null when the host is cleared", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<AccessoryForm initialValues={mountedOptic} onSubmit={onSubmit} />);

    await user.click(screen.getByRole("button", { name: /clear|×/i }));
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(onSubmit.mock.calls[0][0].mountedOn).toBeNull();
  });

  it("presets Mounted on to the record it was opened from, and sends it (US2-4)", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<AccessoryForm presetMountedOn={rifle} onSubmit={onSubmit} />);

    expect(mountedOnField()).toHaveDisplayValue(/LaRue PredatAR/);
    await chooseKind(user, "Optic");
    await nameIt(user);
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(onSubmit.mock.calls[0][0].mountedOn).toEqual({ kind: "firearm", id: 9 });
  });

  it("lets the preset be changed or cleared", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    listMountCandidates.mockResolvedValue({ candidates: [{ label: upper, mountedOn: null }] });
    render(<AccessoryForm presetMountedOn={rifle} onSubmit={onSubmit} />);

    await chooseKind(user, "Optic");
    await nameIt(user);
    await user.click(mountedOnField());
    await user.click(await screen.findByRole("option", { name: /BCM upper/ }));
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onSubmit.mock.calls[0][0].mountedOn).toEqual({ kind: "accessory", id: 11 });

    await user.click(screen.getByRole("button", { name: /clear|×/i }));
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onSubmit.mock.calls[1][0].mountedOn).toBeNull();
  });

  it("shows a mountedOn field error on the Mounted on field", async () => {
    const user = userEvent.setup();
    const message = "An accessory can't be mounted on itself, or on something mounted on it.";
    const onSubmit = vi.fn().mockRejectedValue(
      new CommandFailure({
        code: "VALIDATION_ERROR",
        message,
        fieldErrors: { mountedOn: message },
      }),
    );
    render(<AccessoryForm initialValues={mountedOptic} onSubmit={onSubmit} />);

    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(await screen.findByText(message)).toBeInTheDocument();
    expect(mountedOnField()).toHaveAccessibleDescription(expect.stringContaining(message));
  });
});
