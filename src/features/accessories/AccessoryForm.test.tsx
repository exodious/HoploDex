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
import type { Accessory, AccessoryKind } from "./types";

// specs/006-accessory-links User Story 1 (contracts/ui-accessories.md §3,
// FR-001 to FR-005, FR-027). The form is a bare form like `FirearmForm`: the
// record page and the shell put it in a large `Dialog` titled "Add accessory"
// or "Edit {name}" (tested with them). The Mounted on row is User Story 2's.

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

const DERIVED: Record<string, DerivedCaliber> = {
  "5.56x45mm NATO": { caliber: "5.56mm", source: "catalog" },
};

beforeEach(() => {
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

  it("has no Mounted on row yet: that is User Story 2's", () => {
    render(<AccessoryForm onSubmit={vi.fn()} />);
    expect(screen.queryByLabelText(/^Mounted on/)).not.toBeInTheDocument();
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
        "The value of this record as a whole, everything it describes included. Value each record on its own.",
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
  it("saves with only a kind: every other field is optional", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<AccessoryForm onSubmit={onSubmit} />);

    await chooseKind(user, "Sling");
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit.mock.calls[0][0]).toMatchObject({
      make: null,
      model: null,
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

  it("names an edit of a record with neither make nor model by its kind", async () => {
    const user = userEvent.setup();
    render(
      <AccessoryForm
        initialValues={{ ...saved, make: null, model: null, accessoryKindId: 10 }}
        onSubmit={vi.fn()}
      />,
    );

    await user.type(screen.getByLabelText(/^Notes/), " More.");

    expect(getDirtyForm()?.label).toBe("Sling (edit)");
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
