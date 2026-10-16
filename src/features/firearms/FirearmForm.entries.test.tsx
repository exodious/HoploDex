import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import type { ReactElement } from "react";
import userEvent from "@testing-library/user-event";
import { ACTION_TYPES, FIREARM_TYPES } from "../../test/collectionFixtures";
import {
  fillRequired,
  render,
  resetServiceMocks,
  selectFirearmType,
  settleEntry,
  suggestEntries,
} from "./firearmFormTestSupport";
import { CollectionContext } from "../app/collectionStore";
import type { CollectionState } from "../app/collectionStore";
import { FORM_VERSION, FirearmForm } from "./FirearmForm";
import { CommandFailure } from "../../services/tauriClient";
import type { Firearm } from "./types";
import { render as renderBase, setResumedDraft } from "../../test/sessionScope";

beforeEach(resetServiceMocks);

// 004 US1: the cartridge, and the
// caliber it fills in (004 contracts/ui-entry.md §3, research.md §8).
describe("FirearmForm cartridge and caliber (004 US1)", () => {
  const cartridgeField = () => screen.getByLabelText("Cartridge");
  const caliberField = () => screen.getByLabelText("Caliber");

  async function enterCartridge(user: ReturnType<typeof userEvent.setup>, text: string) {
    await user.clear(cartridgeField());
    await user.type(cartridgeField(), text);
    await user.tab();
  }

  const saved = {
    id: 7,
    make: "Glock",
    model: "17",
    serialNumber: "G17-1",
    noSerialAttested: false,
    caliber: "9mm",
    cartridge: "9x19mm Parabellum",
    firearmTypeId: 1,
    actionTypeId: null,
    status: "active",
  } as Firearm;

  it("puts Cartridge left of Caliber in one row, then Serial number on its own at half width", () => {
    render(<FirearmForm onSubmit={vi.fn()} />);

    const cartridge = cartridgeField();
    expect(cartridge).not.toBeRequired();
    expect(cartridge).toHaveAttribute("placeholder", "e.g. 9x19mm Parabellum");
    expect(cartridge).toHaveAccessibleDescription(
      "Optional. The exact round it's chambered for, e.g. 9x19mm Parabellum.",
    );
    expect(caliberField()).toBeRequired();
    expect(caliberField()).toHaveAttribute("placeholder", "e.g. 9mm");

    const row = cartridge.closest(".hd-form-grid--2");
    expect(row).not.toBeNull();
    expect(row).toContainElement(caliberField());
    expect(
      cartridge.compareDocumentPosition(caliberField()) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();

    const serial = screen.getByLabelText("Serial number");
    expect(row).not.toContainElement(serial);
    expect(serial.closest(".hd-field")).toHaveClass("hd-field--half");
  });

  it("fills Caliber from a built-in cartridge when Cartridge is left (004 US1-1)", async () => {
    const user = userEvent.setup();
    render(<FirearmForm onSubmit={vi.fn()} />);

    await user.type(cartridgeField(), "9x19mm Parabellum");
    await user.tab();

    await waitFor(() => expect(caliberField()).toHaveValue("9mm"));
    expect(settleEntry).toHaveBeenCalledWith("cartridge", "9x19mm Parabellum");
    expect(screen.getByText("From the cartridge.")).toBeInTheDocument();
    expect(screen.queryByText("Guess")).not.toBeInTheDocument();
  });

  it("marks a guessed caliber with a Guess tag described by its hint (004 US1-2)", async () => {
    const user = userEvent.setup();
    render(<FirearmForm onSubmit={vi.fn()} />);

    await enterCartridge(user, ".30 Custom Improved");

    await waitFor(() => expect(caliberField()).toHaveValue(".30"));
    const tag = screen.getByText("Guess");
    expect(tag).toHaveAccessibleDescription("Guessed from the cartridge. Check it before saving.");
    expect(caliberField()).toHaveAccessibleDescription(
      "Guessed from the cartridge. Check it before saving.",
    );
  });

  it("empties Caliber and asks for it when no bore can be read (004 US1-4)", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<FirearmForm onSubmit={onSubmit} />);

    await enterCartridge(user, "9x19mm Parabellum");
    await waitFor(() => expect(caliberField()).toHaveValue("9mm"));
    await enterCartridge(user, "Wildcat Special");

    await waitFor(() => expect(caliberField()).toHaveValue(""));
    expect(
      screen.getByText(
        "We couldn't work out a caliber from \u201cWildcat Special\u201d. Enter it.",
      ),
    ).toBeInTheDocument();

    await user.type(screen.getByLabelText("Make"), "Custom");
    await user.type(screen.getByLabelText("Model"), "Rifle");
    await selectFirearmType(user);
    await user.type(screen.getByLabelText("Serial number"), "W-1");
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(screen.getByText("Enter the caliber.")).toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("keeps a caliber the user typed when the cartridge changes again (004 US1-3)", async () => {
    const user = userEvent.setup();
    render(<FirearmForm onSubmit={vi.fn()} />);

    await enterCartridge(user, ".30 Custom Improved");
    await waitFor(() => expect(caliberField()).toHaveValue(".30"));
    await user.clear(caliberField());
    await user.type(caliberField(), ".308");
    await enterCartridge(user, "9x19mm Parabellum");

    // The cartridge settled twice; the caliber typed in between settles too
    // (004 US2), and its answer changes nothing.
    await waitFor(() =>
      expect(settleEntry.mock.calls.filter(([field]) => field === "cartridge")).toHaveLength(2),
    );
    expect(caliberField()).toHaveValue(".308");
    expect(screen.queryByText("Guess")).not.toBeInTheDocument();
  });

  it("offers the new cartridge's caliber on a saved firearm instead of replacing it (004 US1-7)", async () => {
    const user = userEvent.setup();
    render(<FirearmForm initialValues={saved} onSubmit={vi.fn()} />);

    await enterCartridge(user, ".45 ACP");

    expect(await screen.findByText("The cartridge suggests \u201c.45\u201d.")).toBeInTheDocument();
    expect(caliberField()).toHaveValue("9mm");

    await user.click(screen.getByRole("button", { name: "Use .45" }));
    expect(caliberField()).toHaveValue(".45");
    expect(screen.queryByText("The cartridge suggests \u201c.45\u201d.")).not.toBeInTheDocument();
  });

  it("does not settle an untouched cartridge, or one typed back to its saved value", async () => {
    const user = userEvent.setup();
    render(<FirearmForm initialValues={saved} onSubmit={vi.fn()} />);

    await user.click(cartridgeField());
    await user.tab();
    await enterCartridge(user, "9x19mm Parabellum");

    expect(settleEntry).not.toHaveBeenCalled();
  });

  it.each([
    ["Make", "Make"],
    ["Model", "Model"],
    ["Cartridge", "Cartridge"],
    ["Caliber", "Caliber"],
  ])("shows 004 FR-015's rules for %s when it is left, without truncating", async (label, name) => {
    render(<FirearmForm onSubmit={vi.fn()} />);
    const field = screen.getByLabelText(label);

    const long = "x".repeat(101);
    fireEvent.change(field, { target: { value: long } });
    fireEvent.blur(field);
    expect(screen.getByText(`${name} can be at most 100 characters.`)).toBeInTheDocument();
    expect(field).toHaveValue(long);

    fireEvent.change(field, { target: { value: "Bad\u0007value" } });
    fireEvent.blur(field);
    expect(screen.getByText(`${name} can't contain control characters.`)).toBeInTheDocument();

    // 100 characters, counted as characters: fine.
    fireEvent.change(field, { target: { value: "\u{1F52B}".repeat(100) } });
    fireEvent.blur(field);
    expect(screen.queryByText(`${name} can be at most 100 characters.`)).not.toBeInTheDocument();
    // Let the suggestion and settle answers those events asked for arrive.
    await act(async () => {});
  });

  it("sends the cartridge trimmed, or null when blank", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.click(screen.getByRole("button", { name: "Add firearm" }));
    expect(onSubmit.mock.calls[0][0].cartridge).toBeNull();

    await user.type(cartridgeField(), "  9x19mm Parabellum ");
    await user.click(screen.getByRole("button", { name: "Add firearm" }));
    expect(onSubmit.mock.calls[1][0].cartridge).toBe("9x19mm Parabellum");
    expect(onSubmit.mock.calls[1][0].caliber).toBe("9mm");
  });

  it("keeps a saved firearm's cartridge", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<FirearmForm initialValues={saved} onSubmit={onSubmit} />);

    expect(cartridgeField()).toHaveValue("9x19mm Parabellum");
    await user.click(screen.getByRole("button", { name: "Save changes" }));
    expect(onSubmit.mock.calls[0][0]).toMatchObject({
      cartridge: "9x19mm Parabellum",
      caliber: "9mm",
    });
  });
});

describe("FirearmForm drafts of the caliber state (004 research.md §8)", () => {
  afterEach(() => setResumedDraft(null));

  // 006 research.md §19: the Mounted on choice raised it
  // from 3 to 4.
  it("is version 4", () => {
    expect(FORM_VERSION).toBe(4);
  });

  it("discards a version 3 draft", () => {
    setResumedDraft({
      formVersion: 3,
      kind: "firearm",
      mode: "add",
      targetId: null,
      label: "New firearm",
      values: { make: "Old draft", caliber: ".22" },
    });
    render(<FirearmForm onSubmit={vi.fn()} />);
    expect(screen.getByLabelText("Make")).toHaveValue("");
    expect(screen.getByLabelText("Caliber")).toHaveValue("");
  });

  it("discards a version 1 draft", () => {
    setResumedDraft({
      formVersion: 1,
      kind: "firearm",
      mode: "add",
      targetId: null,
      label: "New firearm",
      values: { make: "Old draft", caliber: ".22" },
    });
    render(<FirearmForm onSubmit={vi.fn()} />);
    expect(screen.getByLabelText("Make")).toHaveValue("");
    expect(screen.getByLabelText("Caliber")).toHaveValue("");
  });

  it("discards a version 2 draft", () => {
    setResumedDraft({
      formVersion: 2,
      kind: "firearm",
      mode: "add",
      targetId: null,
      label: "New firearm",
      values: { make: "Old draft", caliber: ".22" },
    });
    render(<FirearmForm onSubmit={vi.fn()} />);
    expect(screen.getByLabelText("Make")).toHaveValue("");
  });

  it("restores a guessed caliber, still derived, from a current draft", async () => {
    const user = userEvent.setup();
    setResumedDraft({
      formVersion: FORM_VERSION,
      kind: "firearm",
      mode: "add",
      targetId: null,
      label: "New firearm",
      values: {
        make: "Custom",
        cartridge: ".30 Custom Improved",
        caliber: ".30",
        caliberMode: "derived",
        caliberSource: "guess",
      },
    });
    render(<FirearmForm onSubmit={vi.fn()} />);

    expect(screen.getByLabelText("Caliber")).toHaveValue(".30");
    expect(screen.getByText("Guess")).toBeInTheDocument();

    // Still derived: another cartridge fills it again.
    await user.clear(screen.getByLabelText("Cartridge"));
    await user.type(screen.getByLabelText("Cartridge"), "9x19mm Parabellum");
    await user.tab();
    await waitFor(() => expect(screen.getByLabelText("Caliber")).toHaveValue("9mm"));
  });
});

// 004 US2: the four entry fields
// suggest and settle (004 contracts/ui-entry.md §1–§2).
describe("FirearmForm suggestions and snapping (004 US2)", () => {
  const field = (label: string) => screen.getByLabelText(label);
  const snappedMake = {
    value: "Smith & Wesson",
    changedBy: "record" as const,
    derivedCaliber: null,
  };
  const catalogNote = "Changed to the built-in spelling “9x19mm Parabellum”.";
  const recordNote = "Changed to “Smith & Wesson”, as already in your collection.";

  /** The description a field's hint gives, where a note shows. */
  const noteOf = (label: string) => field(label).getAttribute("aria-describedby");
  const noteText = (label: string) =>
    (noteOf(label) ?? "")
      .split(" ")
      .map((id) => document.getElementById(id)?.textContent ?? "")
      .join(" ");

  const saved = {
    id: 7,
    make: "Smith & Wesson",
    model: "686",
    serialNumber: "S-1",
    noSerialAttested: false,
    caliber: ".357",
    cartridge: ".357 Magnum",
    firearmTypeId: 1,
    actionTypeId: null,
    status: "active",
  } as Firearm;

  it("makes Make, Model, Cartridge and Caliber comboboxes", () => {
    render(<FirearmForm onSubmit={vi.fn()} />);

    for (const label of ["Make", "Model", "Cartridge", "Caliber"]) {
      expect(screen.getByRole("combobox", { name: label })).toHaveAttribute(
        "aria-autocomplete",
        "list",
      );
    }
    expect(screen.getByRole("textbox", { name: /^Nickname/ })).toBeInTheDocument();
  });

  it("asks for Model's list with the make on the form", async () => {
    const user = userEvent.setup();
    render(<FirearmForm onSubmit={vi.fn()} />);
    await user.type(field("Make"), "Ruger");
    suggestEntries.mockClear();

    await user.click(field("Model"));

    expect(suggestEntries).toHaveBeenCalledWith("model", "", "Ruger");
    await user.type(field("Model"), "1");
    expect(suggestEntries).toHaveBeenLastCalledWith("model", "1", "Ruger");
    // The other fields never carry a make.
    await user.click(field("Cartridge"));
    expect(suggestEntries).toHaveBeenLastCalledWith("cartridge", "", undefined);
  });

  // 004 US2-4
  it("lists the suggestions with their markers", async () => {
    suggestEntries.mockResolvedValue([
      { value: "9x19mm Parabellum", inCatalog: true, useCount: 2, caliber: "9mm" },
      { value: "Wildcat Special", inCatalog: false, useCount: 1, caliber: null },
      { value: ".380 ACP", inCatalog: true, useCount: 0, caliber: "9mm" },
    ]);
    const user = userEvent.setup();
    render(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(field("Cartridge"));

    expect(
      await screen.findByRole("option", {
        name: "9x19mm Parabellum Built-in · 9mm · 2 in collection",
      }),
    ).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Wildcat Special 1 in collection" })).toBeVisible();
    expect(screen.getByRole("option", { name: ".380 ACP Built-in · 9mm" })).toBeVisible();
  });

  it("settles a changed field when it is left and shows why it changed, until the next edit", async () => {
    settleEntry.mockImplementation(async (name: string, text: string) =>
      name === "make" ? snappedMake : { value: text.trim(), changedBy: null, derivedCaliber: null },
    );
    const user = userEvent.setup();
    render(<FirearmForm onSubmit={vi.fn()} />);

    await user.type(field("Make"), "smith and wesson");
    await user.tab();

    await waitFor(() => expect(field("Make")).toHaveValue("Smith & Wesson"));
    expect(noteText("Make")).toBe(recordNote);
    expect(within(field("Make").closest("[data-field]")!).getByRole("status")).toHaveTextContent(
      recordNote,
    );

    await user.type(field("Make"), "x");
    expect(noteText("Make")).toBe("");
  });

  it("names the built-in spelling when the catalog's is taken", async () => {
    settleEntry.mockResolvedValue({
      value: "9x19mm Parabellum",
      changedBy: "catalog",
      derivedCaliber: { caliber: "9mm", source: "catalog" },
    });
    const user = userEvent.setup();
    render(<FirearmForm onSubmit={vi.fn()} />);

    await user.type(field("Cartridge"), "9 x 19mm parabellum");
    await user.tab();

    await waitFor(() => expect(field("Cartridge")).toHaveValue("9x19mm Parabellum"));
    expect(noteText("Cartridge")).toContain(catalogNote);
    // The derived caliber follows the snapped cartridge.
    expect(field("Caliber")).toHaveValue("9mm");
  });

  it("snaps a caliber and reports it like the other fields", async () => {
    settleEntry.mockImplementation(async (name: string, text: string) =>
      name === "caliber"
        ? { value: "9mm", changedBy: "catalog", derivedCaliber: null }
        : { value: text.trim(), changedBy: null, derivedCaliber: null },
    );
    const user = userEvent.setup();
    render(<FirearmForm onSubmit={vi.fn()} />);

    await user.type(field("Caliber"), "9 MM");
    await user.tab();

    await waitFor(() => expect(field("Caliber")).toHaveValue("9mm"));
    expect(noteText("Caliber")).toContain("Changed to the built-in spelling “9mm”.");
  });

  it("does not settle an untouched field, or one typed back to its saved value (004 FR-014)", async () => {
    const user = userEvent.setup();
    render(<FirearmForm initialValues={saved} onSubmit={vi.fn()} />);

    for (const label of ["Make", "Model", "Cartridge", "Caliber"]) {
      await user.click(field(label));
      await user.tab();
    }
    expect(settleEntry).not.toHaveBeenCalled();

    await user.type(field("Make"), "x");
    await user.type(field("Make"), "{Backspace}");
    await user.tab();
    expect(settleEntry).not.toHaveBeenCalled();

    await user.type(field("Model"), "-2");
    await user.tab();
    expect(settleEntry).toHaveBeenCalledTimes(1);
    expect(settleEntry).toHaveBeenCalledWith("model", "686-2");
  });

  it("settles a picked suggestion at once and keeps focus in the field", async () => {
    suggestEntries.mockResolvedValue([
      { value: "9x19mm Parabellum", inCatalog: true, useCount: 0, caliber: "9mm" },
    ]);
    const user = userEvent.setup();
    render(<FirearmForm onSubmit={vi.fn()} />);
    await user.type(field("Cartridge"), "9x1");

    await user.click(await screen.findByRole("option", { name: /^9x19mm Parabellum/ }));

    expect(field("Cartridge")).toHaveValue("9x19mm Parabellum");
    expect(field("Cartridge")).toHaveFocus();
    await waitFor(() => expect(settleEntry).toHaveBeenCalledWith("cartridge", "9x19mm Parabellum"));
    await waitFor(() => expect(field("Caliber")).toHaveValue("9mm"));
  });

  it("shows the note and does not save when Enter settles a field to another value, then saves on the next Enter", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<FirearmForm onSubmit={onSubmit} />);
    await fillRequired(user);
    settleEntry.mockImplementation(async (name: string, text: string) =>
      name === "make" && text === "smith and wesson"
        ? snappedMake
        : { value: text.trim(), changedBy: null, derivedCaliber: null },
    );
    await user.clear(field("Make"));
    await user.type(field("Make"), "smith and wesson");

    await user.keyboard("{Enter}");

    await waitFor(() => expect(field("Make")).toHaveValue("Smith & Wesson"));
    expect(noteText("Make")).toBe(recordNote);
    expect(onSubmit).not.toHaveBeenCalled();

    await user.keyboard("{Enter}");
    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
    expect(onSubmit.mock.calls[0][0]).toMatchObject({ make: "Smith & Wesson" });
  });

  it("saves the value Enter left as it was when settling changes nothing", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<FirearmForm onSubmit={onSubmit} />);
    await fillRequired(user);
    await user.clear(field("Make"));
    await user.type(field("Make"), "Taurus");

    await user.keyboard("{Enter}");

    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
    expect(onSubmit.mock.calls[0][0]).toMatchObject({ make: "Taurus" });
  });

  it("waits for a settle in flight before saving, and saves what it settled to", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<FirearmForm onSubmit={onSubmit} />);
    await fillRequired(user);
    let answer: (value: typeof snappedMake) => void = () => {};
    settleEntry.mockImplementationOnce(
      () => new Promise((resolve) => (answer = resolve as typeof answer)),
    );
    await user.clear(field("Make"));
    await user.type(field("Make"), "smith and wesson");
    await user.tab();

    await user.click(screen.getByRole("button", { name: "Add firearm" }));
    expect(onSubmit).not.toHaveBeenCalled();

    await act(async () => answer(snappedMake));

    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
    expect(onSubmit.mock.calls[0][0]).toMatchObject({ make: "Smith & Wesson" });
  });

  it("ignores a settle answer for text the field no longer holds", async () => {
    const user = userEvent.setup();
    render(<FirearmForm onSubmit={vi.fn()} />);
    let answer: (value: typeof snappedMake) => void = () => {};
    settleEntry.mockImplementationOnce(
      () => new Promise((resolve) => (answer = resolve as typeof answer)),
    );
    await user.type(field("Make"), "smith and wesson");
    await user.tab();
    await user.click(field("Make"));
    await user.type(field("Make"), " Ltd");

    await act(async () => answer(snappedMake));

    expect(field("Make")).toHaveValue("smith and wesson Ltd");
    expect(noteText("Make")).toBe("");
  });
});

// 004 US3, contracts/ui-entry.md §4: the
// Action choice, filtered by the selected type.
describe("FirearmForm action (004 US3)", () => {
  // In list order, with 005's "Automatic or select-fire" (id 13) seventh.
  const ACTIONS = ACTION_TYPES;
  const NAMES = ACTIONS.map((action) => action.name);
  const ids = (...except: number[]) =>
    ACTIONS.map((action) => action.id).filter((id) => !except.includes(id));
  // 004 FR-018: Handgun (1) has no Pump action, Falling block or Inline
  // muzzleloader; Rifle (2) has all; Shotgun (3) has no Rolling block; Other
  // (4) maps none, which allows all.
  const collection = {
    firearmTypes: { types: FIREARM_TYPES },
    actionTypes: {
      actions: ACTIONS,
      allowedByFirearmType: { 1: ids(5, 7, 12), 2: ids(), 3: ids(8) },
    },
  } as unknown as CollectionState;

  function renderForm(ui: ReactElement = <FirearmForm onSubmit={vi.fn()} />) {
    return renderBase(
      <CollectionContext.Provider value={collection}>{ui}</CollectionContext.Provider>,
    );
  }

  const action = () => screen.getByRole("combobox", { name: "Action" });
  const chooseType = (user: ReturnType<typeof userEvent.setup>, type: string) =>
    user.click(screen.getByRole("radio", { name: type }));
  async function chooseAction(user: ReturnType<typeof userEvent.setup>, name: string) {
    await user.click(action());
    await user.click(await screen.findByRole("option", { name }));
  }
  async function offered(user: ReturnType<typeof userEvent.setup>): Promise<string[]> {
    await user.click(action());
    const names = (await screen.findAllByRole("option")).map((option) => option.textContent ?? "");
    await user.keyboard("{Escape}");
    return names;
  }
  const NOTE = "Pump action doesn't apply to a Handgun, so the action was cleared.";

  it("has an Action select, a third wide, directly after Type, starting Unspecified", () => {
    renderForm();

    expect(action()).toHaveTextContent("Unspecified");
    expect(action().closest(".hd-field--third")).not.toBeNull();
    const type = screen.getByRole("radiogroup", { name: /^Type/ });
    expect(type.compareDocumentPosition(action()) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    const cartridge = screen.getByRole("combobox", { name: "Cartridge" });
    expect(
      action().compareDocumentPosition(cartridge) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it("offers Unspecified, then only the actions the type allows, in list order", async () => {
    const user = userEvent.setup();
    renderForm();

    await chooseType(user, "Handgun");
    expect(await offered(user)).toEqual([
      "Unspecified",
      ...NAMES.filter(
        (name) => !["Pump action", "Falling block", "Inline muzzleloader"].includes(name),
      ),
    ]);

    await chooseType(user, "Shotgun");
    expect(await offered(user)).toEqual([
      "Unspecified",
      ...NAMES.filter((name) => name !== "Rolling block"),
    ]);

    await chooseType(user, "Rifle");
    expect(await offered(user)).toEqual(["Unspecified", ...NAMES]);
  });

  it("offers the whole list for a type that maps none, and before a type is chosen", async () => {
    const user = userEvent.setup();
    renderForm();

    expect(await offered(user)).toEqual(["Unspecified", ...NAMES]);
    await chooseType(user, "Other");
    expect(await offered(user)).toEqual(["Unspecified", ...NAMES]);
  });

  it("offers Automatic or select-fire for Handgun, Rifle, Shotgun and Other, with no classification prompt", async () => {
    const user = userEvent.setup();
    renderForm();

    for (const type of ["Handgun", "Rifle", "Shotgun", "Other"]) {
      await chooseType(user, type);
      expect(await offered(user)).toContain("Automatic or select-fire");
    }
    await chooseAction(user, "Automatic or select-fire");
    expect(action()).toHaveTextContent("Automatic or select-fire");
    expect(screen.queryByText(/classification|regist/i, { selector: "[role=alert]" })).toBeNull();
    expect(screen.queryByRole("status", { name: /classification/i })).toBeNull();
  });

  it("takes no typed text: it is a choice, not a text field", async () => {
    const user = userEvent.setup();
    renderForm();

    expect(action().tagName).toBe("BUTTON");
    await user.click(action());
    await user.keyboard("flintlockish");
    await user.keyboard("{Escape}");
    expect(action()).toHaveTextContent("Unspecified");
  });

  it("clears an action the new type disallows, saying so politely, once", async () => {
    const user = userEvent.setup();
    renderForm();
    await chooseType(user, "Rifle");
    await chooseAction(user, "Pump action");
    expect(action()).toHaveTextContent("Pump action");

    await chooseType(user, "Handgun");

    expect(action()).toHaveTextContent("Unspecified");
    expect(screen.getAllByText(NOTE).length).toBeGreaterThan(0);
    const live = screen.getAllByRole("status").find((status) => status.textContent === NOTE);
    expect(live).toHaveAttribute("aria-live", "polite");
    // It reads as the field's hint.
    expect(action().getAttribute("aria-describedby")).toBeTruthy();
    expect(
      (action().getAttribute("aria-describedby") ?? "")
        .split(" ")
        .some((id) => document.getElementById(id)?.textContent === NOTE),
    ).toBe(true);
  });

  it("removes the note when an action is chosen", async () => {
    const user = userEvent.setup();
    renderForm();
    await chooseType(user, "Rifle");
    await chooseAction(user, "Pump action");
    await chooseType(user, "Handgun");
    expect(screen.getAllByText(NOTE).length).toBeGreaterThan(0);

    await chooseAction(user, "Revolver");

    expect(screen.queryByText(NOTE)).not.toBeInTheDocument();
  });

  it("removes the note when the type changes again", async () => {
    const user = userEvent.setup();
    renderForm();
    await chooseType(user, "Rifle");
    await chooseAction(user, "Pump action");
    await chooseType(user, "Handgun");
    expect(screen.getAllByText(NOTE).length).toBeGreaterThan(0);

    await chooseType(user, "Rifle");

    expect(screen.queryByText(NOTE)).not.toBeInTheDocument();
    expect(action()).toHaveTextContent("Unspecified");
  });

  it("keeps an action the new type allows, with no note", async () => {
    const user = userEvent.setup();
    renderForm();
    await chooseType(user, "Rifle");
    await chooseAction(user, "Lever action");

    await chooseType(user, "Handgun");

    expect(action()).toHaveTextContent("Lever action");
    expect(screen.queryByText(/so the action was cleared/)).not.toBeInTheDocument();
  });

  it("shows a backend actionTypeId error under Action", async () => {
    const user = userEvent.setup();
    const message = "Pump action doesn't apply to a Handgun.";
    const onSubmit = vi.fn().mockRejectedValue(
      new CommandFailure({
        code: "VALIDATION_ERROR",
        message,
        fieldErrors: { actionTypeId: message },
      }),
    );
    renderForm(<FirearmForm onSubmit={onSubmit} />);
    await fillRequired(user);

    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(await screen.findByText(message)).toBeInTheDocument();
    expect(action()).toHaveAttribute("aria-invalid", "true");
  });

  it("saves the chosen action's id, or null for Unspecified", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderForm(<FirearmForm onSubmit={onSubmit} />);
    await fillRequired(user);

    await user.click(screen.getByRole("button", { name: "Add firearm" }));
    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
    expect(onSubmit.mock.calls[0][0].actionTypeId).toBeNull();

    await chooseAction(user, "Revolver");
    await user.click(screen.getByRole("button", { name: "Add firearm" }));
    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(2));
    expect(onSubmit.mock.calls[1][0].actionTypeId).toBe(2);
  });

  it("says so under Action when the list couldn't be loaded", () => {
    render(
      <CollectionContext.Provider
        value={
          {
            firearmTypes: { types: FIREARM_TYPES },
            actionTypes: { actions: [], allowedByFirearmType: {} },
            actionTypesFailed: true,
          } as unknown as CollectionState
        }
      >
        <FirearmForm onSubmit={vi.fn()} />
      </CollectionContext.Provider>,
    );

    expect(screen.getByText(/The list of actions couldn't be loaded/)).toBeInTheDocument();
    expect(action()).toHaveTextContent("Unspecified");
  });

  it("shows no such message when the list loaded", () => {
    renderForm();
    expect(screen.queryByText(/couldn't be loaded/)).not.toBeInTheDocument();
  });

  it("starts an edit on the saved action, and saves it unchanged", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderForm(
      <FirearmForm
        initialValues={
          {
            id: 7,
            make: "Colt",
            model: "Python",
            serialNumber: "V1",
            noSerialAttested: false,
            caliber: ".357",
            cartridge: null,
            firearmTypeId: 1,
            actionTypeId: 2,
            status: "active",
          } as Firearm
        }
        onSubmit={onSubmit}
      />,
    );

    expect(action()).toHaveTextContent("Revolver");
    await user.click(screen.getByRole("button", { name: /^Save/ }));
    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
    expect(onSubmit.mock.calls[0][0].actionTypeId).toBe(2);
  });
});
