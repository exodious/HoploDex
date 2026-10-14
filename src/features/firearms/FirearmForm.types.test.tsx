import { beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor, within } from "@testing-library/react";
import type { ReactElement } from "react";
import userEvent from "@testing-library/user-event";
import { ACTION_TYPES, FIREARM_TYPES, REGISTRATION_CLASSES } from "../../test/collectionFixtures";
import {
  fillRequired,
  listMountCandidates,
  render,
  resetServiceMocks,
  settleEntry,
  suggestEntries,
} from "./firearmFormTestSupport";
import { CollectionContext } from "../app/collectionStore";
import type { CollectionState } from "../app/collectionStore";
import { FirearmForm } from "./FirearmForm";
import { CommandFailure } from "../../services/tauriClient";
import type { Firearm } from "./types";
import type { RecordLabel } from "../mounts/types";
import { render as renderBase } from "../../test/sessionScope";

beforeEach(resetServiceMocks);

// specs/005-regulated-item-types US1 (contracts/ui-registration.md §1): the
// Type cards come from the store, and a type that omits the action, barrel
// length or capacity hides those fields.
describe("FirearmForm suppressor (US1)", () => {
  const collection = {
    firearmTypes: { types: FIREARM_TYPES },
    actionTypes: { actions: ACTION_TYPES, allowedByFirearmType: {} },
  } as unknown as CollectionState;

  function renderForm(ui: ReactElement = <FirearmForm onSubmit={vi.fn()} />) {
    return renderBase(
      <CollectionContext.Provider value={collection}>{ui}</CollectionContext.Provider>,
    );
  }

  const chooseType = (user: ReturnType<typeof userEvent.setup>, type: string) =>
    user.click(screen.getByRole("radio", { name: type }));

  /** A saved Rifle with an action, a barrel length and a capacity. */
  const rifle = {
    id: 7,
    make: "Ruger",
    model: "American",
    serialNumber: "R-1",
    noSerialAttested: false,
    caliber: ".308",
    cartridge: null,
    firearmTypeId: 2,
    actionTypeId: 3,
    barrelLengthHundredths: 2000,
    capacity: 5,
    status: "active",
  } as Firearm;

  it("offers the types in list order, Suppressor before Other", () => {
    renderForm();

    const names = screen.getAllByRole("radio").map((radio) => radio.closest("label")?.textContent);
    expect(names).toEqual(["Handgun", "Rifle", "Shotgun", "Suppressor", "Other"]);
  });

  it("offers no Action, Barrel length or Capacity for a Suppressor, and labels its Rated cartridge", async () => {
    const user = userEvent.setup();
    renderForm();

    expect(screen.getByRole("combobox", { name: "Action" })).toBeInTheDocument();
    await chooseType(user, "Suppressor");

    expect(screen.queryByRole("combobox", { name: "Action" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /^Physical details/ }));
    expect(screen.queryByLabelText(/^Barrel length/)).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Capacity")).not.toBeInTheDocument();
    for (const label of ["Overall length (in)", "Weight (lb)", "Weight (oz)", "Finish"]) {
      expect(screen.getByLabelText(label)).toBeInTheDocument();
    }
    expect(screen.getByRole("combobox", { name: "Condition" })).toBeInTheDocument();
    // FR-002, research.md §15: the caliber is the bore, the cartridge the
    // rating.
    const caliber = screen.getByLabelText("Caliber");
    expect(caliber).toBeRequired();
    expect(caliber).toHaveAttribute("placeholder", "e.g. .30");
    expect(caliber).toHaveAccessibleDescription("The bore diameter.");
    const rated = screen.getByLabelText("Rated cartridge");
    expect(rated).not.toBeRequired();
    expect(rated).toHaveAttribute("placeholder", "e.g. .300 Winchester Magnum");
    expect(rated).toHaveAccessibleDescription(
      "The most powerful cartridge the suppressor is rated for.",
    );
    expect(screen.queryByLabelText("Cartridge")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Caliber rating")).not.toBeInTheDocument();
  });

  /** Types a cartridge into the field under `label` and leaves it. */
  async function enterRated(user: ReturnType<typeof userEvent.setup>, label: string, text: string) {
    await user.clear(screen.getByLabelText(label));
    await user.type(screen.getByLabelText(label), text);
    await user.tab();
    await waitFor(() => expect(settleEntry).toHaveBeenCalledWith("cartridge", text));
  }

  it("fills no caliber from a new Suppressor's rated cartridge (US1-7)", async () => {
    const user = userEvent.setup();
    renderForm();
    await chooseType(user, "Suppressor");

    await enterRated(user, "Rated cartridge", ".300 Winchester Magnum");
    expect(screen.getByLabelText("Caliber")).toHaveValue("");
    expect(screen.queryByText("From the cartridge.")).not.toBeInTheDocument();

    // Nothing is guessed, and an unreadable one asks for nothing either.
    await enterRated(user, "Rated cartridge", ".30 Custom Improved");
    await enterRated(user, "Rated cartridge", "Wildcat Special");
    expect(screen.getByLabelText("Caliber")).toHaveValue("");
    expect(screen.queryByText("Guess")).not.toBeInTheDocument();
    expect(screen.queryByText(/We couldn't work out a caliber/)).not.toBeInTheDocument();

    // Leaving the caliber empty derives nothing.
    await user.click(screen.getByLabelText("Caliber"));
    await user.tab();
    expect(screen.getByLabelText("Caliber")).toHaveValue("");
    expect(screen.getByLabelText("Caliber")).toHaveAccessibleDescription("The bore diameter.");
  });

  it("suggests no caliber for a saved Suppressor's new rated cartridge (US1-7)", async () => {
    const user = userEvent.setup();
    renderForm(
      <FirearmForm
        initialValues={{
          ...rifle,
          firearmTypeId: 5,
          caliber: ".46",
          actionTypeId: null,
          barrelLengthHundredths: null,
          capacity: null,
        }}
        onSubmit={vi.fn()}
      />,
    );

    await enterRated(user, "Rated cartridge", ".300 Winchester Magnum");

    expect(screen.getByLabelText("Caliber")).toHaveValue(".46");
    expect(screen.queryByText(/The cartridge suggests/)).not.toBeInTheDocument();
  });

  it("keeps a Rifle's derived caliber, without its hint, when the type becomes Suppressor", async () => {
    const user = userEvent.setup();
    renderForm();
    await chooseType(user, "Rifle");
    await enterRated(user, "Cartridge", ".30 Custom Improved");
    await waitFor(() => expect(screen.getByLabelText("Caliber")).toHaveValue(".30"));
    expect(screen.getByText("Guess")).toBeInTheDocument();

    await chooseType(user, "Suppressor");

    expect(screen.getByLabelText("Caliber")).toHaveValue(".30");
    expect(screen.queryByText("Guess")).not.toBeInTheDocument();
    expect(screen.queryByText(/Guessed from the cartridge/)).not.toBeInTheDocument();
    expect(screen.getByLabelText("Caliber")).toHaveAccessibleDescription("The bore diameter.");

    // Back to a Rifle, the caliber stays the owner's: a new cartridge only
    // suggests (004 FR-006).
    await chooseType(user, "Rifle");
    expect(screen.getByLabelText("Caliber")).toHaveValue(".30");
    await enterRated(user, "Cartridge", ".45 ACP");
    expect(await screen.findByText("The cartridge suggests \u201c.45\u201d.")).toBeInTheDocument();
    expect(screen.getByLabelText("Caliber")).toHaveValue(".30");
  });

  it("derives an empty caliber from the rated cartridge at once when the type stops being Suppressor", async () => {
    const user = userEvent.setup();
    renderForm();
    await chooseType(user, "Suppressor");
    await enterRated(user, "Rated cartridge", ".300 Winchester Magnum");
    expect(screen.getByLabelText("Caliber")).toHaveValue("");

    await chooseType(user, "Rifle");

    expect(screen.getByLabelText("Caliber")).toHaveValue(".30");
    expect(screen.getByText("From the cartridge.")).toBeInTheDocument();
  });

  it("leaves the hidden fields out of the closed summary", async () => {
    const user = userEvent.setup();
    // The Rifle's recorded values open the group; close it to read the summary.
    renderForm(<FirearmForm initialValues={rifle} onSubmit={vi.fn()} />);
    await chooseType(user, "Suppressor");
    await user.click(screen.getByRole("button", { name: /^Physical details/ }));

    const button = screen.getByRole("button", { name: /^Physical details/ });
    expect(button).not.toHaveTextContent(/barrel|rounds/);
    expect(button).toHaveTextContent("Optional: length, weight, finish and condition.");
  });

  it("names what will be cleared when a Rifle is changed to a Suppressor, and keeps it until save", async () => {
    const user = userEvent.setup();
    renderForm(<FirearmForm initialValues={rifle} onSubmit={vi.fn()} />);

    await chooseType(user, "Suppressor");

    const note =
      "A Suppressor has no action, barrel length or capacity, so Bolt action, 20 in and 5 rounds will be cleared when you save.";
    expect(screen.getAllByText(note).length).toBeGreaterThan(0);

    // The values come back when the type changes back, and the note goes.
    await chooseType(user, "Rifle");
    expect(screen.queryByText(note)).not.toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Action" })).toHaveTextContent("Bolt action");
    expect(screen.getByLabelText(/^Barrel length/)).toHaveValue("20");
    expect(screen.getByLabelText("Capacity")).toHaveValue("5");
  });

  it("names only the fields that hold a value", async () => {
    const user = userEvent.setup();
    renderForm(
      <FirearmForm
        initialValues={{ ...rifle, actionTypeId: null, capacity: null }}
        onSubmit={vi.fn()}
      />,
    );

    await chooseType(user, "Suppressor");

    expect(
      screen.getAllByText(
        "A Suppressor has no action, barrel length or capacity, so 20 in will be cleared when you save.",
      ).length,
    ).toBeGreaterThan(0);
  });

  it("shows no note when nothing is recorded in a field the type omits", async () => {
    const user = userEvent.setup();
    renderForm();

    await chooseType(user, "Suppressor");

    expect(screen.queryByText(/will be cleared when you save/)).not.toBeInTheDocument();
  });

  it("announces the note through the polite live region", async () => {
    const user = userEvent.setup();
    renderForm(<FirearmForm initialValues={rifle} onSubmit={vi.fn()} />);

    await chooseType(user, "Suppressor");

    const live = screen
      .getAllByRole("status")
      .find((el) => /will be cleared when you save/.test(el.textContent ?? ""));
    expect(live).toHaveAttribute("aria-live", "polite");
  });

  it("sends the action, barrel length and capacity as null for a Suppressor", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderForm(<FirearmForm initialValues={rifle} onSubmit={onSubmit} />);

    await chooseType(user, "Suppressor");
    await user.click(screen.getByRole("button", { name: /^Save/ }));

    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
    expect(onSubmit.mock.calls[0][0]).toMatchObject({
      firearmTypeId: 5,
      actionTypeId: null,
      barrelLengthHundredths: null,
      capacity: null,
    });
  });

  it("does not check a barrel length the type omits", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderForm(
      <FirearmForm
        initialValues={{ ...rifle, barrelLengthHundredths: null, capacity: null }}
        onSubmit={onSubmit}
      />,
    );
    await user.click(screen.getByRole("button", { name: /^Physical details/ }));
    await user.type(screen.getByLabelText(/^Barrel length/), "abc");
    await chooseType(user, "Suppressor");
    await user.click(screen.getByRole("button", { name: /^Save/ }));

    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
    expect(onSubmit.mock.calls[0][0].barrelLengthHundredths).toBeNull();
  });

  it("keeps a record's registration when it is saved", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderForm(
      <FirearmForm
        initialValues={{
          ...rifle,
          registrationClassId: 2,
          registrationForm: "Form 1",
          registrationApproved: "2026-02-10",
          registeredTo: "Smith Family Trust",
        }}
        onSubmit={onSubmit}
      />,
    );

    await user.click(screen.getByRole("button", { name: /^Save/ }));

    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
    expect(onSubmit.mock.calls[0][0]).toMatchObject({
      registrationClassId: 2,
      registrationForm: "Form 1",
      registrationApproved: "2026-02-10",
      registeredTo: "Smith Family Trust",
    });
  });

  it("says so under Type when the list couldn't be loaded", () => {
    renderBase(
      <CollectionContext.Provider
        value={
          {
            firearmTypes: { types: [] },
            firearmTypesFailed: true,
            actionTypes: { actions: [], allowedByFirearmType: {} },
          } as unknown as CollectionState
        }
      >
        <FirearmForm onSubmit={vi.fn()} />
      </CollectionContext.Provider>,
    );

    expect(screen.getByText(/The list of types couldn't be loaded/)).toBeInTheDocument();
  });
});

// specs/005-regulated-item-types User Story 2 (contracts/ui-registration.md
// §2, §3): an optional classification with a form, an approved date and
// "Registered to", and nothing on the form that judges legal status.
describe("FirearmForm registration (US2)", () => {
  const offered = REGISTRATION_CLASSES;
  const collection = (classes = offered) =>
    ({
      firearmTypes: { types: FIREARM_TYPES },
      actionTypes: { actions: ACTION_TYPES, allowedByFirearmType: {} },
      registrationClasses: { classes },
    }) as unknown as CollectionState;

  function renderForm(ui: ReactElement = <FirearmForm onSubmit={vi.fn()} />, classes = offered) {
    return renderBase(
      <CollectionContext.Provider value={collection(classes)}>{ui}</CollectionContext.Provider>,
    );
  }

  const registrationButton = () => screen.getByRole("button", { name: /^Registration/ });
  const openRegistration = async (user: ReturnType<typeof userEvent.setup>) => {
    if (registrationButton().getAttribute("aria-expanded") === "false") {
      await user.click(registrationButton());
    }
  };
  const chooseClass = async (user: ReturnType<typeof userEvent.setup>, name: string) => {
    await user.click(screen.getByRole("combobox", { name: "Registered as" }));
    await user.click(await screen.findByRole("option", { name }));
  };

  const suppressor = {
    id: 9,
    make: "SilencerCo",
    model: "Omega 300",
    serialNumber: "OM-1",
    noSerialAttested: false,
    caliber: ".30",
    cartridge: null,
    firearmTypeId: 5,
    actionTypeId: null,
    status: "active",
    registrationClassId: 1,
    registrationForm: "Form 4",
    registrationApproved: "2026-02-10",
    registeredTo: "Smith Family Trust",
  } as Firearm;

  const plainRifle = {
    id: 10,
    make: "Ruger",
    model: "American",
    serialNumber: "R-1",
    noSerialAttested: false,
    caliber: ".308",
    cartridge: null,
    firearmTypeId: 2,
    actionTypeId: 1,
    barrelLengthHundredths: 1050,
    status: "active",
    registrationClassId: null,
  } as Firearm;

  it("puts the Registration section after Origin and before Physical details", () => {
    renderForm();

    const buttons = screen
      .getAllByRole("button", { name: /^(Origin and year|Registration|Physical details)/ })
      .map((button) => button.textContent);
    expect(buttons[0]).toMatch(/^Origin and year of manufacture/);
    expect(buttons[1]).toMatch(/^Registration/);
    expect(buttons[2]).toMatch(/^Physical details/);
  });

  it("starts closed and says what it holds when nothing is recorded", () => {
    renderForm();

    expect(registrationButton()).toHaveAttribute("aria-expanded", "false");
    expect(registrationButton()).toHaveTextContent(
      "Optional: what the firearm is registered as, and the approval.",
    );
  });

  it("reads back everything recorded when closed, leaving out what is missing", async () => {
    const user = userEvent.setup();
    renderForm(<FirearmForm initialValues={suppressor} onSubmit={vi.fn()} />);

    expect(registrationButton()).toHaveAttribute("aria-expanded", "true");
    await user.click(registrationButton());
    expect(registrationButton()).toHaveTextContent(
      "Registered as Suppressor. Form 4, approved Feb 10, 2026. Registered to Smith Family Trust.",
    );
  });

  it("leaves out the parts that aren't recorded from the summary", async () => {
    const user = userEvent.setup();
    renderForm(
      <FirearmForm
        initialValues={{
          ...suppressor,
          registrationForm: null,
          registrationApproved: "2026-02-10",
          registeredTo: null,
        }}
        onSubmit={vi.fn()}
      />,
    );
    await user.click(registrationButton());

    expect(registrationButton()).toHaveTextContent(
      "Registered as Suppressor. Approved Feb 10, 2026.",
    );
    expect(registrationButton()).not.toHaveTextContent("Registered to");
  });

  it("opens itself when a save fails on a field inside it", async () => {
    const user = userEvent.setup();
    const message = "Approved date can't be in the future.";
    const onSubmit = vi.fn().mockRejectedValue(
      new CommandFailure({
        code: "VALIDATION_ERROR",
        message,
        fieldErrors: { registrationApproved: message },
      }),
    );
    renderForm(
      <FirearmForm
        initialValues={{ ...suppressor, registrationApproved: null }}
        onSubmit={onSubmit}
      />,
    );
    await user.click(registrationButton());
    expect(registrationButton()).toHaveAttribute("aria-expanded", "false");

    await user.click(screen.getByRole("button", { name: /^Save/ }));

    await waitFor(() => expect(registrationButton()).toHaveAttribute("aria-expanded", "true"));
    expect(screen.getByText(message)).toBeInTheDocument();
  });

  it("opens with the standing note and a link to the guide's Registered items", async () => {
    const user = userEvent.setup();
    Element.prototype.scrollIntoView = vi.fn();
    renderForm();
    await openRegistration(user);

    expect(screen.getByRole("note")).toHaveTextContent(
      "HoploDex records what you enter here. It doesn't decide what is regulated or needs registering, and the law changes.",
    );
    await user.click(screen.getByRole("button", { name: "How to record registrations" }));

    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    const heading = screen.getByRole("heading", { name: "Registered items" });
    await waitFor(() => expect(heading).toHaveFocus());
  });

  it("offers Unspecified, then the six classifications in list order", async () => {
    const user = userEvent.setup();
    renderForm();
    await openRegistration(user);

    await user.click(screen.getByRole("combobox", { name: "Registered as" }));

    const names = (await screen.findAllByRole("option")).map((option) => option.textContent);
    expect(names).toEqual([
      "Unspecified",
      "Suppressor",
      "Short-barreled rifle",
      "Short-barreled shotgun",
      "Any other weapon",
      "Machine gun",
      "Destructive device",
    ]);
  });

  it("shows Form, Approved and Registered to only once a classification is chosen", async () => {
    const user = userEvent.setup();
    renderForm();
    await openRegistration(user);
    expect(screen.queryByLabelText("Form")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Approved")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Registered to")).not.toBeInTheDocument();

    await chooseClass(user, "Suppressor");

    for (const label of ["Form", "Approved", "Registered to"]) {
      const input = screen.getByLabelText(label);
      expect(input).toBeInTheDocument();
      expect(input).not.toBeRequired();
    }
    expect(screen.getByText("The date on the approved form (the tax stamp date).")).toBeVisible();
    expect(screen.getByText("A person, trust or company, as named on the form.")).toBeVisible();
    expect(
      within(screen.getByRole("group", { name: "Registration" })).queryByText("*"),
    ).not.toBeInTheDocument();
  });

  it("marks the three built-in form names in Form's suggestions", async () => {
    suggestEntries.mockResolvedValue(
      ["Form 4", "Form 1", "Form 5"].map((value) => ({
        value,
        inCatalog: true,
        useCount: 0,
        caliber: null,
      })),
    );
    const user = userEvent.setup();
    renderForm(<FirearmForm initialValues={suppressor} onSubmit={vi.fn()} />);

    await user.click(screen.getByLabelText("Form"));

    const options = await screen.findAllByRole("option");
    expect(options.map((option) => option.textContent)).toEqual([
      "Form 4 Built-in",
      "Form 1 Built-in",
      "Form 5 Built-in",
    ]);
    expect(suggestEntries).toHaveBeenCalledWith("registrationForm", "Form 4", undefined);
  });

  it("settles Form and Registered to and shows the note", async () => {
    settleEntry.mockImplementation(async (field: string, text: string) => ({
      value: field === "registeredTo" ? "Smith Family Trust" : text.trim(),
      changedBy: field === "registeredTo" ? "record" : null,
      derivedCaliber: null,
    }));
    const user = userEvent.setup();
    renderForm(
      <FirearmForm initialValues={{ ...suppressor, registeredTo: null }} onSubmit={vi.fn()} />,
    );

    await user.type(screen.getByLabelText("Registered to"), "smith family trust");
    await user.tab();

    await waitFor(() =>
      expect(screen.getByLabelText("Registered to")).toHaveValue("Smith Family Trust"),
    );
    expect(settleEntry).toHaveBeenCalledWith("registeredTo", "smith family trust");
    expect(
      screen.getAllByText("Changed to “Smith Family Trust”, as already in your collection.").length,
    ).toBeGreaterThan(0);
  });

  it("blocks a future approved date with the field's message", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderForm(<FirearmForm initialValues={suppressor} onSubmit={onSubmit} />);
    const approved = screen.getByLabelText("Approved");
    await user.clear(approved);
    await user.type(approved, "2999-01-01");

    await user.click(screen.getByRole("button", { name: /^Save/ }));

    expect(await screen.findByText("Approved date can't be in the future.")).toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("saves the classification and its details, with nothing derived", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderForm(
      <FirearmForm
        initialValues={{
          ...suppressor,
          registrationClassId: null,
          registrationForm: null,
          registrationApproved: null,
          registeredTo: null,
        }}
        onSubmit={onSubmit}
      />,
    );
    await openRegistration(user);
    await chooseClass(user, "Short-barreled rifle");
    await user.type(screen.getByLabelText("Form"), "Form 1");
    await user.type(screen.getByLabelText("Approved"), "2026-02-10");
    await user.type(screen.getByLabelText("Registered to"), "Alex Rivera");
    await user.click(screen.getByRole("button", { name: /^Save/ }));

    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
    expect(onSubmit.mock.calls[0][0]).toMatchObject({
      registrationClassId: 2,
      registrationForm: "Form 1",
      registrationApproved: "2026-02-10",
      registeredTo: "Alex Rivera",
    });
  });

  it("asks before Unspecified discards details, naming only what is recorded", async () => {
    const user = userEvent.setup();
    renderForm(
      <FirearmForm
        initialValues={{ ...suppressor, registrationApproved: null }}
        onSubmit={vi.fn()}
      />,
    );

    await chooseClass(user, "Unspecified");

    const dialog = await screen.findByRole("alertdialog", {
      name: "Discard the registration details?",
    });
    expect(dialog).toHaveTextContent(
      "Clearing what the firearm is registered as will discard the form “Form 4” and Registered to “Smith Family Trust”. They can't be recovered once saved.",
    );
    expect(within(dialog).getByRole("button", { name: "Discard details" })).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Keep them" })).toBeInTheDocument();
  });

  it("names all three parts when all are recorded", async () => {
    const user = userEvent.setup();
    renderForm(<FirearmForm initialValues={suppressor} onSubmit={vi.fn()} />);

    await chooseClass(user, "Unspecified");

    expect(await screen.findByRole("alertdialog")).toHaveTextContent(
      "will discard the form “Form 4”, the approved date and Registered to “Smith Family Trust”. They can't be recovered once saved.",
    );
  });

  it("keeps the classification on Keep them and clears all four on Discard details", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderForm(<FirearmForm initialValues={suppressor} onSubmit={onSubmit} />);

    await chooseClass(user, "Unspecified");
    await user.click(await screen.findByRole("button", { name: "Keep them" }));
    expect(screen.getByRole("combobox", { name: "Registered as" })).toHaveTextContent("Suppressor");
    expect(screen.getByLabelText("Form")).toHaveValue("Form 4");

    await chooseClass(user, "Unspecified");
    await user.click(await screen.findByRole("button", { name: "Discard details" }));
    expect(screen.getByRole("combobox", { name: "Registered as" })).toHaveTextContent(
      "Unspecified",
    );
    expect(screen.queryByLabelText("Form")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /^Save/ }));
    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
    expect(onSubmit.mock.calls[0][0]).toMatchObject({
      registrationClassId: null,
      registrationForm: null,
      registrationApproved: null,
      registeredTo: null,
    });
  });

  it("keeps the details, with no question, when another classification is chosen", async () => {
    const user = userEvent.setup();
    renderForm(<FirearmForm initialValues={suppressor} onSubmit={vi.fn()} />);

    await chooseClass(user, "Machine gun");

    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Registered as" })).toHaveTextContent(
      "Machine gun",
    );
    expect(screen.getByLabelText("Form")).toHaveValue("Form 4");
    expect(screen.getByLabelText("Registered to")).toHaveValue("Smith Family Trust");
  });

  it("clears at once when Unspecified is chosen with no details recorded", async () => {
    const user = userEvent.setup();
    renderForm(
      <FirearmForm
        initialValues={{
          ...suppressor,
          registrationForm: null,
          registrationApproved: null,
          registeredTo: null,
        }}
        onSubmit={vi.fn()}
      />,
    );

    await chooseClass(user, "Unspecified");

    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Registered as" })).toHaveTextContent(
      "Unspecified",
    );
  });

  it("offers a classification that is no longer offered to the record holding it, in its place", async () => {
    const user = userEvent.setup();
    const classes = offered.map((item) => (item.id === 5 ? { ...item, offered: false } : item));
    const { unmount } = renderForm(
      <FirearmForm initialValues={{ ...suppressor, registrationClassId: 5 }} onSubmit={vi.fn()} />,
      classes,
    );
    await user.click(screen.getByRole("combobox", { name: "Registered as" }));
    expect((await screen.findAllByRole("option")).map((o) => o.textContent)).toEqual([
      "Unspecified",
      "Suppressor",
      "Short-barreled rifle",
      "Short-barreled shotgun",
      "Any other weapon",
      "Machine gun",
      "Destructive device",
    ]);
    unmount();

    // Another record is not offered it.
    renderForm(<FirearmForm initialValues={plainRifle} onSubmit={vi.fn()} />, classes);
    await openRegistration(user);
    await user.click(screen.getByRole("combobox", { name: "Registered as" }));
    expect((await screen.findAllByRole("option")).map((o) => o.textContent)).not.toContain(
      "Machine gun",
    );
  });

  it("still offers the saved record's no-longer-offered classification after switching away and back", async () => {
    const user = userEvent.setup();
    const classes = offered.map((item) => (item.id === 5 ? { ...item, offered: false } : item));
    renderForm(
      <FirearmForm initialValues={{ ...suppressor, registrationClassId: 5 }} onSubmit={vi.fn()} />,
      classes,
    );
    const select = () => screen.getByRole("combobox", { name: "Registered as" });
    await user.click(select());
    await user.click(await screen.findByRole("option", { name: "Suppressor" }));
    expect(select()).toHaveTextContent("Suppressor");

    await user.click(select());
    const names = (await screen.findAllByRole("option")).map((o) => o.textContent);
    expect(names).toEqual([
      "Unspecified",
      "Suppressor",
      "Short-barreled rifle",
      "Short-barreled shotgun",
      "Any other weapon",
      "Machine gun",
      "Destructive device",
    ]);
    await user.click(screen.getByRole("option", { name: "Machine gun" }));
    expect(select()).toHaveTextContent("Machine gun");
  });

  it("says so when the classifications couldn't be loaded", async () => {
    const user = userEvent.setup();
    renderBase(
      <CollectionContext.Provider
        value={
          {
            firearmTypes: { types: FIREARM_TYPES },
            registrationClasses: { classes: [] },
            registrationClassesFailed: true,
          } as unknown as CollectionState
        }
      >
        <FirearmForm onSubmit={vi.fn()} />
      </CollectionContext.Provider>,
    );
    await openRegistration(user);

    expect(screen.getByText(/The list of classifications couldn't be loaded/)).toBeInTheDocument();
  });

  // FR-014, SC-002, US2-8: nothing on the form judges legal status.
  describe.each([
    ["a Rifle with a 10.5 in barrel and no classification", plainRifle],
    [
      "a Suppressor with no classification",
      {
        ...suppressor,
        registrationClassId: null,
        registrationForm: null,
        registrationApproved: null,
        registeredTo: null,
      },
    ],
    [
      "a Rifle registered as Machine gun with a Semi-automatic action",
      { ...plainRifle, registrationClassId: 5, actionTypeId: 1 },
    ],
  ])("for %s", (_name, record) => {
    it("shows no regulatory text outside the Registration section's own labels and note", async () => {
      const user = userEvent.setup();
      const { container } = renderForm(
        <FirearmForm initialValues={record as Firearm} onSubmit={vi.fn()} />,
      );
      // Open every folded section, so all of the form's text is rendered.
      for (const button of screen.getAllByRole("button", { expanded: false })) {
        if (/^(Origin|Physical|Registration)/.test(button.textContent ?? "")) {
          await user.click(button);
        }
      }

      // The form body: the footer's "Required" key belongs to the form itself.
      const text = container.querySelector(".hd-dialog__body")?.textContent ?? "";
      const withoutOwn = text
        .replace(
          "HoploDex records what you enter here. It doesn't decide what is regulated or needs registering, and the law changes.",
          "",
        )
        .replace("Registered as", "")
        .replace("Registered to", "")
        .replace("How to record registrations", "")
        .replace("Optional: what the firearm is registered as, and the approval.", "");
      expect(withoutOwn).not.toMatch(/regulat|\bNFA\b|pending|unregistered|compliant/i);
      expect(withoutOwn).not.toMatch(/register/i);
      // Nothing inside the Registration section is marked required.
      const section = screen.getByRole("group", { name: "Registration" });
      expect(section.textContent ?? "").not.toMatch(/required/i);
    });
  });
});

// specs/006-accessory-links User Story 2 (contracts/ui-accessories.md §4,
// FR-010, FR-012): the Mounted on choice, after Type and Action. A saved
// firearm's current host arrives with its detail (`mount.chain[0]`), which is
// what the record page passes as `initialValues`; the form sends only the
// host's `RecordRef` as `mountedOn`.
describe("FirearmForm Mounted on (US2)", () => {
  const HINT =
    "Only if this firearm is mounted on another firearm or an accessory, such as a suppressor on a rifle.";

  const upper: RecordLabel = {
    record: { kind: "accessory", id: 11 },
    make: "BCM",
    model: "upper",
    nickname: null,
    typeName: "Upper receiver",
    serialNumber: "U-100",
    status: "active",
  };
  const rifle: RecordLabel = {
    record: { kind: "firearm", id: 7 },
    make: "Winchester",
    model: "Model 70",
    nickname: "Deer rifle",
    typeName: "Rifle",
    serialNumber: "W70-123",
    status: "active",
  };

  const mountedOnField = () => screen.getByRole("combobox", { name: /^Mounted on/ });

  /** A saved Suppressor mounted on the upper. */
  const mounted = {
    id: 4,
    make: "SilencerCo",
    model: "Omega 300",
    serialNumber: "S-1",
    noSerialAttested: false,
    caliber: ".30",
    firearmTypeId: 5,
    status: "active",
    notes: null,
    mountedOn: upper.record,
    mount: { chain: [upper], mounted: [] },
  } as unknown as Firearm;

  it("puts Mounted on after Type and Action", () => {
    render(<FirearmForm onSubmit={vi.fn()} />);

    const after = (earlier: HTMLElement) =>
      earlier.compareDocumentPosition(mountedOnField()) & Node.DOCUMENT_POSITION_FOLLOWING;
    expect(after(screen.getByRole("radio", { name: "Handgun" }))).toBeTruthy();
    expect(after(screen.getByRole("combobox", { name: "Action" }))).toBeTruthy();
  });

  it("gives the hint about a suppressor on a rifle", () => {
    render(<FirearmForm onSubmit={vi.fn()} />);

    expect(mountedOnField()).toHaveAccessibleDescription(expect.stringContaining(HINT));
  });

  it("reads 'Not mounted' on a new firearm and submits mountedOn null", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<FirearmForm onSubmit={onSubmit} />);

    expect(mountedOnField()).toHaveAttribute("placeholder", "Not mounted");
    await fillRequired(user);
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit.mock.calls[0][0].mountedOn).toBeNull();
  });

  it("searches the hosts for a new firearm, with no record to leave out", async () => {
    const user = userEvent.setup();
    render(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(mountedOnField());

    await waitFor(() =>
      expect(listMountCandidates).toHaveBeenLastCalledWith(
        expect.objectContaining({ role: "host", record: null }),
      ),
    );
  });

  it("sends the chosen host as mountedOn on save", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    listMountCandidates.mockResolvedValue({
      candidates: [{ label: rifle, mountedOn: null }],
    });
    render(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.click(mountedOnField());
    await user.click(await screen.findByRole("option", { name: /Deer rifle/ }));
    expect(mountedOnField()).toHaveDisplayValue(/Winchester Model 70/);
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(onSubmit.mock.calls[0][0].mountedOn).toEqual({ kind: "firearm", id: 7 });
  });

  it("shows the current host when editing, searches without the firearm itself, and keeps it on save", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<FirearmForm initialValues={mounted} onSubmit={onSubmit} />);

    expect(mountedOnField()).toHaveDisplayValue(/BCM upper · Upper receiver/);
    await user.click(mountedOnField());
    await waitFor(() =>
      expect(listMountCandidates).toHaveBeenLastCalledWith(
        expect.objectContaining({ role: "host", record: { kind: "firearm", id: 4 } }),
      ),
    );
    await user.keyboard("{Escape}");
    await user.click(screen.getByRole("button", { name: "Save changes" }));

    expect(onSubmit.mock.calls[0][0].mountedOn).toEqual({ kind: "accessory", id: 11 });
  });

  it("sends mountedOn null when the host is cleared", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<FirearmForm initialValues={mounted} onSubmit={onSubmit} />);

    await user.click(screen.getByRole("button", { name: /clear|×/i }));
    await user.click(screen.getByRole("button", { name: "Save changes" }));

    expect(onSubmit.mock.calls[0][0].mountedOn).toBeNull();
  });

  it("shows a mountedOn field error on the Mounted on field", async () => {
    const user = userEvent.setup();
    const message = "A firearm can't be mounted on itself, or on something mounted on it.";
    const onSubmit = vi.fn().mockRejectedValue(
      new CommandFailure({
        code: "VALIDATION_ERROR",
        message,
        fieldErrors: { mountedOn: message },
      }),
    );
    render(<FirearmForm initialValues={mounted} onSubmit={onSubmit} />);

    await user.click(screen.getByRole("button", { name: "Save changes" }));

    expect(await screen.findByText(message)).toBeInTheDocument();
    expect(mountedOnField()).toHaveAccessibleDescription(expect.stringContaining(message));
  });
});
