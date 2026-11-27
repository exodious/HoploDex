// 014: the firearm edit form's changed markers, per-field revert and Save
// state (014 US1, US2, US3).
import { beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactElement } from "react";
import {
  listMountCandidates,
  originGroupButton,
  resetServiceMocks,
} from "./firearmFormTestSupport";
import { FirearmForm } from "./FirearmForm";
import type { Firearm } from "./types";
import type { RecordLabel } from "../mounts/types";
import { getDirtyForm } from "../session/usePendingDraft";
import { CollectionContext } from "../app/collectionStore";
import type { CollectionState } from "../app/collectionStore";
import { ACTION_TYPES, FIREARM_TYPES, REGISTRATION_CLASSES } from "../../test/collectionFixtures";
import { render as renderBase } from "../../test/sessionScope";

beforeEach(resetServiceMocks);

const collection = {
  firearmTypes: { types: FIREARM_TYPES },
  actionTypes: { actions: ACTION_TYPES, allowedByFirearmType: {} },
  registrationClasses: { classes: REGISTRATION_CLASSES },
} as unknown as CollectionState;

function render(ui: ReactElement) {
  return renderBase(
    <CollectionContext.Provider value={collection}>{ui}</CollectionContext.Provider>,
  );
}

const glock = {
  id: 3,
  make: "Glock",
  model: "19",
  serialNumber: "ABC123",
  noSerialAttested: false,
  caliber: "9mm",
  firearmTypeId: 1,
  status: "active",
  estimatedValue: 1250,
  weightTenthsOz: 405,
  notes: "Range gun",
} as Firearm;

const rifle = {
  id: 7,
  make: "Ruger",
  model: "American",
  serialNumber: "R7",
  noSerialAttested: false,
  caliber: ".308",
  firearmTypeId: 2,
  actionTypeId: 3,
  barrelLengthHundredths: 2000,
  capacity: 5,
  status: "active",
} as Firearm;

const save = () => screen.getByRole("button", { name: "Save changes" });
const changedTags = () => screen.queryAllByText("Changed", { selector: ".hd-changed__tag" });

describe("014 US1 - See Which Fields I Have Changed", () => {
  it("marks nothing at first, and only the field changed; typed back, nothing (014 US1-1 to US1-3)", async () => {
    const user = userEvent.setup();
    render(<FirearmForm initialValues={glock} onSubmit={vi.fn()} />);
    expect(changedTags()).toHaveLength(0);
    const make = screen.getByRole("combobox", { name: /^Make/ });
    await user.type(make, " Ges.m.b.H.");
    expect(changedTags()).toHaveLength(1);
    expect(screen.getByRole("button", { name: "Revert Make to “Glock”" })).toBeInTheDocument();
    expect(make).toHaveAccessibleDescription(/^Changed/);
    await user.clear(make);
    await user.type(make, "Glock");
    expect(changedTags()).toHaveLength(0);
  });

  it("compares as a save stores: spaces, leading zeros and one weight (014 US1-4 to US1-6)", async () => {
    const user = userEvent.setup();
    render(<FirearmForm initialValues={glock} onSubmit={vi.fn()} />);
    const make = screen.getByRole("combobox", { name: /^Make/ });
    await user.type(make, " ");
    expect(changedTags()).toHaveLength(0);
    await user.clear(make);
    await user.type(make, "glock");
    expect(changedTags()).toHaveLength(1);
    await user.clear(make);
    await user.type(make, "Glock");

    const value = screen.getByLabelText(/^Estimated replacement value/);
    await user.clear(value);
    await user.type(value, "01250");
    expect(changedTags()).toHaveLength(0);
  });

  it("says a folded section holds a change (014 US1-7)", async () => {
    const user = userEvent.setup();
    render(<FirearmForm initialValues={glock} onSubmit={vi.fn()} />);
    const group = () => screen.getByRole("button", { name: /^Physical details/ });
    if (group().getAttribute("aria-expanded") === "false") await user.click(group());
    const pounds = screen.getByLabelText(/^Weight \(lb\)/);
    await user.clear(pounds);
    await user.type(pounds, "3");
    expect(changedTags()).toHaveLength(1);
    await user.click(group());
    expect(group()).toHaveAccessibleName(/· Changed$/);
  });

  it("doesn't count a photo or document, which the record page saves at once (014 FR-022)", () => {
    // The form holds no attachments: opening it marks nothing and keeps no draft.
    render(<FirearmForm initialValues={glock} onSubmit={vi.fn()} />);
    expect(changedTags()).toHaveLength(0);
    expect(getDirtyForm()).toBeNull();
  });
});

describe("014 US2 - Put One Field Back to Its Saved Value", () => {
  it("reverts one field only, without saving, and focuses it (014 US2-1, US2-3)", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    render(<FirearmForm initialValues={glock} onSubmit={onSubmit} />);
    await user.type(screen.getByRole("combobox", { name: /^Make/ }), "x");
    const notes = screen.getByLabelText(/^Notes/);
    await user.type(notes, " and carry");
    await user.click(screen.getByRole("button", { name: "Revert Notes to “Range gun”" }));
    expect(notes).toHaveValue("Range gun");
    expect(screen.getByRole("combobox", { name: /^Make/ })).toHaveValue("Glockx");
    await waitFor(() => expect(notes).toHaveFocus());
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("names an empty saved value Not recorded (014 US2-2)", async () => {
    const user = userEvent.setup();
    render(<FirearmForm initialValues={glock} onSubmit={vi.fn()} />);
    await user.type(screen.getByLabelText(/^Nickname/), "Range");
    expect(
      screen.getByRole("button", { name: "Revert Nickname to “Not recorded”" }),
    ).toBeInTheDocument();
  });

  it("reverting Type brings back the action it cleared (014 US2-4, US2-5)", async () => {
    const user = userEvent.setup();
    render(<FirearmForm initialValues={rifle} onSubmit={vi.fn()} />);
    await user.click(screen.getByRole("radio", { name: "Handgun" }));
    await user.click(screen.getByRole("button", { name: "Revert Type to “Rifle”" }));
    expect(screen.getByRole("radio", { name: "Rifle" })).toBeChecked();
    expect(changedTags()).toHaveLength(0);
  });

  it("clears an error from the edit with it (014 US2-9)", async () => {
    const user = userEvent.setup();
    render(<FirearmForm initialValues={glock} onSubmit={vi.fn()} />);
    await user.type(screen.getByLabelText(/^Date acquired|^Acquisition date/), "2999-01-01");
    await user.click(save());
    expect(screen.getByRole("alert")).toBeInTheDocument();
    await user.click(
      screen.getByRole("button", { name: /^Revert (Date acquired|Acquisition date)/ }),
    );
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("marks and reverts an Origin that discarded nothing (014 FR-009, FR-011)", async () => {
    const user = userEvent.setup();
    render(<FirearmForm initialValues={glock} onSubmit={vi.fn()} />);
    if (originGroupButton().getAttribute("aria-expanded") === "false") {
      await user.click(originGroupButton());
    }
    await user.click(screen.getByRole("radio", { name: /^Domestic/ }));
    expect(changedTags()).toHaveLength(1);
    expect(save()).not.toHaveAttribute("aria-disabled");
    expect(getDirtyForm()).not.toBeNull();
    await user.click(screen.getByRole("button", { name: "Revert Origin to “Unspecified”" }));
    expect(screen.getByRole("radio", { name: /^Unspecified/ })).toBeChecked();
    expect(save()).toHaveAttribute("aria-disabled", "true");
  });

  it("reverting Origin brings back the import details it discarded (014 US2-6)", async () => {
    const user = userEvent.setup();
    const imported = {
      ...glock,
      origin: "imported",
      countryOfManufacture: "Austria",
      importerName: "Glock Inc.",
    } as Firearm;
    render(<FirearmForm initialValues={imported} onSubmit={vi.fn()} />);
    if (originGroupButton().getAttribute("aria-expanded") === "false") {
      await user.click(originGroupButton());
    }
    await user.click(screen.getByRole("radio", { name: /^Domestic/ }));
    await user.click(screen.getByRole("button", { name: "Discard and change" }));
    expect(screen.queryByLabelText("Importer")).not.toBeInTheDocument();
    expect(changedTags()).toHaveLength(1);
    await user.click(screen.getByRole("button", { name: "Revert Origin to “Imported”" }));
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(screen.getByRole("radio", { name: /^Imported/ })).toBeChecked();
    expect(screen.getByLabelText("Country of manufacture")).toHaveValue("Austria");
    expect(screen.getByLabelText("Importer")).toHaveValue("Glock Inc.");
    expect(changedTags()).toHaveLength(0);
    expect(save()).toHaveAttribute("aria-disabled", "true");
  });

  it("keeps a saved value the list no longer offers until another is picked (014 Edge Cases)", () => {
    render(<FirearmForm initialValues={{ ...rifle, actionTypeId: 13 }} onSubmit={vi.fn()} />);
    expect(changedTags()).toHaveLength(0);
  });
});

describe("014 US5 - Change What Is Mounted, and Save It with the Record", () => {
  it("doesn't offer as Mounted on a record the Mounted list will mount here (014 FR-026, Edge Cases)", async () => {
    const user = userEvent.setup();
    const optic = (id: number, make: string): RecordLabel => ({
      record: { kind: "accessory", id },
      make,
      model: "X",
      nickname: null,
      typeName: "Optic",
      serialNumber: null,
      status: "active",
    });
    listMountCandidates.mockResolvedValue({
      candidates: [
        { label: optic(21, "Leupold"), mountedOn: null },
        { label: optic(22, "Trijicon"), mountedOn: null },
      ],
    });
    render(<FirearmForm initialValues={rifle} onSubmit={vi.fn()} />);
    await user.click(screen.getByRole("combobox", { name: /^Add to Mounted/ }));
    await user.click(await screen.findByRole("option", { name: /Leupold/ }));
    await user.click(screen.getByRole("combobox", { name: /^Mounted on/ }));
    expect(await screen.findByRole("option", { name: /Trijicon/ })).toBeInTheDocument();
    expect(screen.queryByRole("option", { name: /Leupold/ })).not.toBeInTheDocument();
  });
});

describe("014 US3 - Save Only When There Is Something to Save", () => {
  it("says there is nothing to save, stays reachable, and does nothing (014 US3-1 to US3-4)", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    render(<FirearmForm initialValues={glock} onSubmit={onSubmit} />);
    expect(save()).toHaveAttribute("aria-disabled", "true");
    expect(save()).toHaveAccessibleDescription("No changes to save");
    await user.click(save());
    await user.type(screen.getByLabelText(/^Nickname/), "x{Enter}");
    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(save()).not.toHaveAttribute("aria-disabled");
    await user.click(screen.getByRole("button", { name: /^Revert Nickname/ }));
    expect(save()).toHaveAttribute("aria-disabled", "true");
  });

  it("stays enabled on a change with an error, and shows it without saving (014 US3-5)", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    render(<FirearmForm initialValues={glock} onSubmit={onSubmit} />);
    await user.type(screen.getByLabelText(/^Date acquired|^Acquisition date/), "2999-01-01");
    expect(save()).not.toHaveAttribute("aria-disabled");
    await user.click(save());
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("keeps the add form's Save enabled, showing what is missing (014 US3-8)", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    render(<FirearmForm onSubmit={onSubmit} />);
    const add = screen.getByRole("button", { name: "Add firearm" });
    expect(add).not.toHaveAttribute("aria-disabled");
    await user.click(add);
    expect(onSubmit).not.toHaveBeenCalled();
    expect(screen.getByText("Enter the make.")).toBeInTheDocument();
    expect(changedTags()).toHaveLength(0);
  });
});

describe("014 US4 - Unsaved Means the Same Thing Everywhere", () => {
  it("has no unsaved input once typed back, and has it while changed (014 US4-1, US4-2)", async () => {
    const user = userEvent.setup();
    render(<FirearmForm initialValues={glock} onSubmit={vi.fn()} />);
    const nickname = screen.getByLabelText(/^Nickname/);
    await user.type(nickname, "x");
    expect(getDirtyForm()).not.toBeNull();
    await user.clear(nickname);
    expect(getDirtyForm()).toBeNull();
  });

  it("an add form has unsaved input as soon as anything is entered (014 US4-6, US4-7)", async () => {
    const user = userEvent.setup();
    render(<FirearmForm onSubmit={vi.fn()} />);
    const make = screen.getByRole("combobox", { name: /^Make/ });
    await user.type(make, "Glock");
    expect(getDirtyForm()?.label).toBe("New firearm");
    await user.clear(make);
    expect(getDirtyForm()).toBeNull();
  });
});
