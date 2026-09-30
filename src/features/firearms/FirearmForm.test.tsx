import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  act,
  fireEvent,
  render as renderBase,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import type { ReactElement } from "react";
import { todayIso } from "../../lib/dates";
import userEvent from "@testing-library/user-event";
import { ACTION_TYPES, FIREARM_TYPES, REGISTRATION_CLASSES } from "../../test/collectionFixtures";
import { CollectionContext } from "../app/collectionStore";
import type { CollectionState } from "../app/collectionStore";
import { getDirtyForm, setResumedDraft } from "../session/usePendingDraft";
import { FORM_VERSION, FirearmForm } from "./FirearmForm";
import { CommandFailure } from "../../services/tauriClient";
import type { DerivedCaliber, Firearm } from "./types";

// specs/005-regulated-item-types: the form reads the firearm types from the
// collection store, so every render gets the fixture's five seeded types
// unless a test provides its own collection.
const typesCollection = {
  firearmTypes: { types: FIREARM_TYPES },
  registrationClasses: { classes: REGISTRATION_CLASSES },
} as unknown as CollectionState;
function render(ui: ReactElement) {
  return renderBase(ui, {
    wrapper: ({ children }) => (
      <CollectionContext.Provider value={typesCollection}>{children}</CollectionContext.Provider>
    ),
  });
}

// specs/004-cartridges-action-types: the form settles an entry through the
// backend (`settle_entry`) and lists suggestions (`suggest_entries`); here
// stand-ins derive a few known calibers and suggest nothing unless a test says.
const settleEntry = vi.fn();
const suggestEntries = vi.fn();
vi.mock("./firearmsService", () => ({
  settleEntry: (field: string, text: string) => settleEntry(field, text),
  suggestEntries: (field: string, text: string, make?: string | null) =>
    suggestEntries(field, text, make),
}));

const DERIVED: Record<string, DerivedCaliber> = {
  "9x19mm Parabellum": { caliber: "9mm", source: "catalog" },
  "9mm Luger": { caliber: "9mm", source: "catalog" },
  ".45 ACP": { caliber: ".45", source: "catalog" },
  ".30 Custom Improved": { caliber: ".30", source: "guess" },
  "6.5x47 Wildcat": { caliber: "6.5mm", source: "guess" },
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

/** The "Origin and year of manufacture" disclosure's button. Closed, its
 * name also carries the summary line, so match on the title alone. */
const originGroupButton = () =>
  screen.getByRole("button", { name: /^Origin and year of manufacture/ });

/** Renders the form with the origin group open (it starts closed on a record
 * with none of its fields recorded). */
function renderWithOriginGroup(ui: ReactElement) {
  const result = render(ui);
  if (originGroupButton().getAttribute("aria-expanded") === "false") {
    fireEvent.click(originGroupButton());
  }
  return result;
}

/** The "Physical details" disclosure's button, matched on its title alone. */
const physicalGroupButton = () => screen.getByRole("button", { name: /^Physical details/ });

/** Renders the form with the physical details group open. */
function renderWithPhysicalGroup(ui: ReactElement) {
  const result = render(ui);
  if (physicalGroupButton().getAttribute("aria-expanded") === "false") {
    fireEvent.click(physicalGroupButton());
  }
  return result;
}

async function selectFirearmType(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("radio", { name: "Handgun" }));
}

describe("FirearmForm serial-attestation rule", () => {
  it("blocks submission when serial number is blank and attestation is unchecked", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<FirearmForm onSubmit={onSubmit} />);

    await user.type(screen.getByLabelText("Make"), "Glock");
    await user.type(screen.getByLabelText("Model"), "19");
    await user.type(screen.getByLabelText("Caliber"), "9mm");
    // Serial number left blank, attestation left unchecked.

    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(
      screen.getByText("Enter a serial number, or confirm this firearm has none."),
    ).toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("allows submission with a blank serial number once attestation is checked", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<FirearmForm onSubmit={onSubmit} />);

    await user.type(screen.getByLabelText("Make"), "Homemade");
    await user.type(screen.getByLabelText("Model"), "80% build");
    await user.type(screen.getByLabelText("Caliber"), ".223");
    await selectFirearmType(user);
    await user.click(screen.getByLabelText("This firearm has no serial number"));

    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(onSubmit).toHaveBeenCalledTimes(1);
    const submitted = onSubmit.mock.calls[0][0];
    expect(submitted.serialNumber).toBeNull();
    expect(submitted.noSerialAttested).toBe(true);
  });

  it("allows submission when a serial number is provided, without requiring attestation", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<FirearmForm onSubmit={onSubmit} />);

    await user.type(screen.getByLabelText("Make"), "Glock");
    await user.type(screen.getByLabelText("Model"), "19");
    await user.type(screen.getByLabelText("Caliber"), "9mm");
    await selectFirearmType(user);
    await user.type(screen.getByLabelText("Serial number"), "ABC123");

    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(onSubmit).toHaveBeenCalledTimes(1);
    const submitted = onSubmit.mock.calls[0][0];
    expect(submitted.serialNumber).toBe("ABC123");
    expect(submitted.noSerialAttested).toBe(false);
  });

  it("checking attestation disables and does not require the serial number field", async () => {
    const user = userEvent.setup();
    render(<FirearmForm onSubmit={vi.fn()} />);

    const checkbox = screen.getByLabelText("This firearm has no serial number");
    await user.click(checkbox);

    expect(screen.getByLabelText("Serial number")).toBeDisabled();
  });
});

async function fillRequired(user: ReturnType<typeof userEvent.setup>) {
  await user.type(screen.getByLabelText("Make"), "Glock");
  await user.type(screen.getByLabelText("Model"), "19");
  await user.type(screen.getByLabelText("Caliber"), "9mm");
  await selectFirearmType(user);
  await user.type(screen.getByLabelText("Serial number"), "ABC123");
}

function tomorrowIso(): string {
  const next = new Date(`${todayIso()}T00:00:00Z`);
  next.setUTCDate(next.getUTCDate() + 1);
  return next.toISOString().slice(0, 10);
}

describe("FirearmForm date rules (FR-003 / FR-004)", () => {
  it("blocks a future acquisition date with a field-level message", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.type(screen.getByLabelText("Date acquired"), tomorrowIso());
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(screen.getByText("Acquisition date can't be in the future.")).toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("accepts today as the acquisition date", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.type(screen.getByLabelText("Date acquired"), todayIso());
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(onSubmit).toHaveBeenCalledTimes(1);
  });

  it("blocks a disposition dated before the acquisition when correcting a disposed record", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    const disposed = {
      id: 1,
      make: "Colt",
      model: "Python",
      serialNumber: "X1",
      noSerialAttested: false,
      caliber: ".357",
      firearmTypeId: 1,
      status: "disposed",
      acquisitionDate: "2025-03-01",
      dispositionType: "sold",
      dispositionRecipient: "Jane",
      dispositionDate: "2025-06-01",
      dispositionPrice: 100,
    } as Firearm;
    render(<FirearmForm initialValues={disposed} onSubmit={onSubmit} />);

    const date = screen.getByLabelText("Date");
    await user.clear(date);
    await user.type(date, "2025-02-28");
    await user.click(screen.getByRole("button", { name: "Save changes" }));

    expect(
      screen.getByText("Disposition date can't be earlier than the acquisition date."),
    ).toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();
  });
});

describe("FirearmForm nickname (FR-031)", () => {
  it("submits a trimmed nickname, or null when left blank", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.click(screen.getByRole("button", { name: "Add firearm" }));
    expect(onSubmit.mock.calls[0][0].nickname).toBeNull();

    await user.type(screen.getByLabelText("Nickname"), "  Old Faithful ");
    await user.click(screen.getByRole("button", { name: "Add firearm" }));
    expect(onSubmit.mock.calls[1][0].nickname).toBe("Old Faithful");
  });

  it("shows the backend's duplicate-nickname message on the nickname field", async () => {
    const user = userEvent.setup();
    const message = 'That nickname is already used by Glock 19 "Old Faithful".';
    const onSubmit = vi.fn().mockRejectedValue(
      new CommandFailure({
        code: "VALIDATION_ERROR",
        message,
        fieldErrors: { nickname: message },
      }),
    );
    render(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.type(screen.getByLabelText("Nickname"), "Old Faithful");
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(await screen.findByText(message)).toBeInTheDocument();
  });

  it("prefills the nickname when editing", () => {
    render(
      <FirearmForm
        initialValues={
          { id: 1, make: "Colt", model: "Python", nickname: "Snake", status: "active" } as Firearm
        }
        onSubmit={vi.fn()}
      />,
    );
    expect(screen.getByLabelText("Nickname")).toHaveValue("Snake");
  });
});

describe("FirearmForm physical details (FR-039, US1 Acceptance Scenario 17)", () => {
  const group = () => screen.getByRole("group", { name: "Physical details" });
  const barrel = () => within(group()).getByLabelText("Barrel length (in)");
  const overall = () => within(group()).getByLabelText("Overall length (in)");
  const weightLb = () => within(group()).getByLabelText("Weight (lb)");
  const weightOz = () => within(group()).getByLabelText("Weight (oz)");
  const capacity = () => within(group()).getByLabelText("Capacity");
  const finish = () => within(group()).getByLabelText("Finish");

  async function pickCondition(user: ReturnType<typeof userEvent.setup>, option: string) {
    await user.click(within(group()).getByRole("combobox", { name: "Condition" }));
    await user.click(await screen.findByRole("option", { name: option }));
  }

  it("has a Physical details group with all six fields", () => {
    renderWithPhysicalGroup(<FirearmForm onSubmit={vi.fn()} />);

    for (const field of [barrel(), overall(), weightLb(), weightOz(), capacity(), finish()]) {
      expect(field).toBeInTheDocument();
    }
    expect(within(group()).getByRole("combobox", { name: "Condition" })).toBeInTheDocument();
  });

  it("offers Not recorded and the six grades, best first", async () => {
    const user = userEvent.setup();
    renderWithPhysicalGroup(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(within(group()).getByRole("combobox", { name: "Condition" }));

    const options = (await screen.findAllByRole("option")).map((o) => o.textContent);
    expect(options).toEqual([
      "Not recorded",
      "New in box",
      "Like new",
      "Excellent",
      "Good",
      "Fair",
      "Poor",
    ]);
  });

  it("submits blanks as null", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderWithPhysicalGroup(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(onSubmit.mock.calls[0][0]).toMatchObject({
      barrelLengthHundredths: null,
      overallLengthHundredths: null,
      weightTenthsOz: null,
      capacity: null,
      finish: null,
      condition: null,
    });
  });

  it("submits entered values as the scaled integers", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderWithPhysicalGroup(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.type(barrel(), "16.25");
    await user.type(overall(), "18");
    await user.type(weightLb(), "2");
    await user.type(weightOz(), "8.5");
    await user.type(capacity(), "15");
    await user.type(finish(), "  Cerakote  ");
    await pickCondition(user, "Like new");
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(onSubmit.mock.calls[0][0]).toMatchObject({
      barrelLengthHundredths: 1625,
      overallLengthHundredths: 1800,
      weightTenthsOz: 405,
      capacity: 15,
      finish: "Cerakote",
      condition: "like_new",
    });
  });

  it("converts pounds alone, ounces alone, or both to tenths of an ounce", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderWithPhysicalGroup(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.type(weightLb(), "6.5");
    await user.click(screen.getByRole("button", { name: "Add firearm" }));
    expect(onSubmit.mock.calls[0][0].weightTenthsOz).toBe(1040);

    await user.clear(weightLb());
    await user.type(weightOz(), "40.5");
    await user.click(screen.getByRole("button", { name: "Add firearm" }));
    expect(onSubmit.mock.calls[1][0].weightTenthsOz).toBe(405);

    await user.type(weightLb(), "2");
    await user.clear(weightOz());
    await user.type(weightOz(), "8.5");
    await user.click(screen.getByRole("button", { name: "Add firearm" }));
    expect(onSubmit.mock.calls[2][0].weightTenthsOz).toBe(405);
  });

  it("blocks a zero weight with a message on the box", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderWithPhysicalGroup(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.type(weightLb(), "0");
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(screen.getByText("Must be greater than 0.")).toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("submits null for Not recorded after a grade was chosen", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderWithPhysicalGroup(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await pickCondition(user, "Good");
    await pickCondition(user, "Not recorded");
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(onSubmit.mock.calls[0][0].condition).toBeNull();
  });

  it("prefills from the record", () => {
    renderWithPhysicalGroup(
      <FirearmForm
        initialValues={
          {
            id: 1,
            make: "Colt",
            model: "Python",
            status: "active",
            barrelLengthHundredths: 1625,
            overallLengthHundredths: 1800,
            weightTenthsOz: 405,
            capacity: 6,
            finish: "Blued",
            condition: "excellent",
          } as Firearm
        }
        onSubmit={vi.fn()}
      />,
    );

    expect(barrel()).toHaveValue("16.25");
    expect(overall()).toHaveValue("18");
    expect(weightLb()).toHaveValue("2");
    expect(weightOz()).toHaveValue("8.5");
    expect(capacity()).toHaveValue("6");
    expect(finish()).toHaveValue("Blued");
    expect(within(group()).getByRole("combobox", { name: "Condition" })).toHaveTextContent(
      "Excellent",
    );
  });

  it("accepts only digits in capacity", async () => {
    const user = userEvent.setup();
    renderWithPhysicalGroup(<FirearmForm onSubmit={vi.fn()} />);

    await user.type(capacity(), "1a2.5");

    expect(capacity()).toHaveValue("125");
  });

  it("rounds extra decimal places to the stored unit instead of blocking", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderWithPhysicalGroup(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.type(barrel(), "16.255");
    await user.type(weightLb(), "2.53");
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(onSubmit.mock.calls[0][0]).toMatchObject({
      barrelLengthHundredths: 1626,
      weightTenthsOz: 405,
    });
  });

  it("blocks the save for a zero length and a capacity of 0", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderWithPhysicalGroup(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.type(overall(), "0");
    await user.type(capacity(), "0");
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(screen.getByText("Must be greater than 0.")).toBeInTheDocument();
    expect(screen.getByText("Capacity must be at least 1.")).toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("shows the backend's message on the field it names", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockRejectedValue(
      new CommandFailure({
        code: "VALIDATION_ERROR",
        message: "The firearm record has validation errors.",
        fieldErrors: { weightTenthsOz: "Weight must be greater than 0." },
      }),
    );
    renderWithPhysicalGroup(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(await screen.findByText("Weight must be greater than 0.")).toBeInTheDocument();
  });
});

describe("FirearmForm focusField (FR-038, US1 Acceptance Scenario 15)", () => {
  function reducedMotion(reduce: boolean) {
    vi.stubGlobal(
      "matchMedia",
      vi.fn().mockImplementation((query: string) => ({
        matches: reduce && query.includes("prefers-reduced-motion"),
        media: query,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      })),
    );
  }

  beforeEach(() => {
    vi.useFakeTimers();
    Element.prototype.scrollIntoView = vi.fn();
    reducedMotion(false);
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  for (const [focusField, label] of [
    ["notes", "Notes"],
    ["accessories", "Accessories"],
  ] as const) {
    it(`focuses ${focusField}, scrolls it into view and briefly highlights its section`, () => {
      render(<FirearmForm onSubmit={vi.fn()} focusField={focusField} />);

      const field = screen.getByLabelText(label);
      expect(field).toHaveFocus();
      expect(Element.prototype.scrollIntoView).toHaveBeenCalledTimes(1);
      const section = field.closest("[data-highlight]");
      expect(section).not.toBeNull();
      expect(section).toHaveAttribute("data-highlight", "animated");

      act(() => {
        vi.advanceTimersByTime(3000);
      });
      expect(field.closest("[data-highlight]")).toBeNull();
      expect(field).toHaveFocus();
    });
  }

  it("skips the highlight animation and smooth scrolling under prefers-reduced-motion", () => {
    reducedMotion(true);
    render(<FirearmForm onSubmit={vi.fn()} focusField="notes" />);

    const notes = screen.getByLabelText("Notes");
    expect(notes).toHaveFocus();
    // Still marked so the user can see where to type, but without motion.
    expect(notes.closest("[data-highlight]")).toHaveAttribute("data-highlight", "static");
    expect(Element.prototype.scrollIntoView).toHaveBeenCalledWith(
      expect.objectContaining({ behavior: "auto" }),
    );
  });

  it("changes nothing when no field was asked for", () => {
    render(<FirearmForm onSubmit={vi.fn()} />);

    expect(document.querySelector("[data-highlight]")).toBeNull();
    expect(Element.prototype.scrollIntoView).not.toHaveBeenCalled();
    expect(screen.getByLabelText("Notes")).not.toHaveFocus();
  });
});

// specs/002-firearm-identification contracts/ui-identification.md §1-§3
describe("FirearmForm origin control (US1)", () => {
  it("offers Domestic/Imported/Re-imported/Unspecified with their one-line descriptions", () => {
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    expect(
      screen.getByRole("radio", { name: /^Domestic Made in the U\.S\.$/ }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("radio", { name: /^Imported Made abroad and brought in$/ }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("radio", {
        name: /^Re-imported Made in the U\.S\., exported, then brought back in$/,
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("radio", { name: /^Unspecified Leave this if you're not sure\.$/ }),
    ).toBeInTheDocument();
  });

  it("starts a new record on Unspecified", () => {
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);
    expect(screen.getByRole("radio", { name: /^Unspecified/ })).toBeChecked();
  });

  it("selecting Imported reveals Country of manufacture and Importer, both optional", async () => {
    const user = userEvent.setup();
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /^Imported/ }));

    expect(screen.getByLabelText("Country of manufacture")).not.toBeRequired();
    expect(screen.getByLabelText("Importer")).not.toBeRequired();
    expect(screen.queryByText("Country of manufacture: United States")).not.toBeInTheDocument();
  });

  it("selecting Re-imported reveals only Importer plus a read-only United States country line", async () => {
    const user = userEvent.setup();
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /^Re-imported/ }));

    expect(screen.getByLabelText("Importer")).toBeInTheDocument();
    expect(screen.queryByLabelText("Country of manufacture")).not.toBeInTheDocument();
    expect(screen.getByText("Country of manufacture: United States")).toBeInTheDocument();
  });

  it("shows neither field for Domestic or Unspecified", async () => {
    const user = userEvent.setup();
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /^Domestic/ }));
    expect(screen.queryByLabelText("Country of manufacture")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Importer")).not.toBeInTheDocument();

    await user.click(screen.getByRole("radio", { name: /^Unspecified/ }));
    expect(screen.queryByLabelText("Country of manufacture")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Importer")).not.toBeInTheDocument();
  });

  it("shows the Domestic cue to consider Re-imported", async () => {
    const user = userEvent.setup();
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    expect(
      screen.queryByText(
        "Made in the U.S. but stamped with an importer's name? Choose Re-imported.",
      ),
    ).not.toBeInTheDocument();

    await user.click(screen.getByRole("radio", { name: /^Domestic/ }));
    expect(
      screen.getByText("Made in the U.S. but stamped with an importer's name? Choose Re-imported."),
    ).toBeInTheDocument();
  });

  it("shows Year of manufacture for every origin and validates a four-digit range", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderWithOriginGroup(<FirearmForm onSubmit={onSubmit} />);

    const currentYear = new Date().getFullYear();
    const year = screen.getByLabelText("Year of manufacture");
    await user.type(year, "43");
    await user.tab();

    expect(
      screen.getByText(
        `Year of manufacture must be a four-digit year from 1400 to ${currentYear}.`,
      ),
    ).toBeInTheDocument();

    await fillRequired(user);
    await user.click(screen.getByRole("button", { name: "Add firearm" }));
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("accepts a valid year and submits it as a number", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderWithOriginGroup(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.type(screen.getByLabelText("Year of manufacture"), "1943");
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit.mock.calls[0][0].yearOfManufacture).toBe(1943);
  });

  it("asks before discarding importer and country when moving away from an import-marked origin", async () => {
    const user = userEvent.setup();
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /^Imported/ }));
    await user.type(screen.getByLabelText("Country of manufacture"), "Belgium");
    await user.type(screen.getByLabelText("Importer"), "Global Arms Import Co.");

    await user.click(screen.getByRole("radio", { name: /^Domestic/ }));

    expect(screen.getByRole("alertdialog")).toBeInTheDocument();
    expect(screen.getByText("Discard importer and original marks?")).toBeInTheDocument();
    // Cancelling keeps everything, including the origin.
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.getByRole("radio", { name: /^Imported/ })).toBeChecked();
    expect(screen.getByLabelText("Country of manufacture")).toHaveValue("Belgium");

    await user.click(screen.getByRole("radio", { name: /^Domestic/ }));
    await user.click(screen.getByRole("button", { name: "Discard and change" }));
    expect(screen.getByRole("radio", { name: /^Domestic/ })).toBeChecked();
    expect(screen.queryByLabelText("Country of manufacture")).not.toBeInTheDocument();
  });

  it("asks before discarding only the country when moving from Imported to Re-imported", async () => {
    const user = userEvent.setup();
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /^Imported/ }));
    await user.type(screen.getByLabelText("Country of manufacture"), "Belgium");
    await user.type(screen.getByLabelText("Importer"), "Global Arms Import Co.");

    await user.click(screen.getByRole("radio", { name: /^Re-imported/ }));

    expect(screen.getByText("Discard the country of manufacture?")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Discard and change" }));

    expect(screen.getByRole("radio", { name: /^Re-imported/ })).toBeChecked();
    // Importer carries over.
    expect(screen.getByLabelText("Importer")).toHaveValue("Global Arms Import Co.");
  });

  it("does not ask when nothing would be lost", async () => {
    const user = userEvent.setup();
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /^Imported/ }));
    await user.click(screen.getByRole("radio", { name: /^Domestic/ }));

    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(screen.getByRole("radio", { name: /^Domestic/ })).toBeChecked();
  });

  it("opens the origin guide from the How do I record this? button", async () => {
    const user = userEvent.setup();
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("button", { name: "How do I record this?" }));
    expect(
      screen.getByRole("dialog", {
        name: "How to record where a firearm came from and how it's registered",
      }),
    ).toBeInTheDocument();
  });
});

// specs/002-firearm-identification contracts/ui-identification.md §2, US2
describe("FirearmForm original maker's marks (US2)", () => {
  const group = () => screen.getByRole("group", { name: "Original maker's marks" });

  it("shows the fieldset for an import-marked origin, with a hint and all three fields optional", async () => {
    const user = userEvent.setup();
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /^Imported/ }));

    expect(
      screen.getByText(
        "Only if the original maker's marks differ from the make, model and serial number above, or you want both.",
      ),
    ).toBeInTheDocument();
    expect(within(group()).getByLabelText("Original maker")).not.toBeRequired();
    expect(within(group()).getByLabelText("Original model")).not.toBeRequired();
    expect(within(group()).getByLabelText("Original serial number")).not.toBeRequired();
  });

  it("shows the fieldset for a re-imported origin too", async () => {
    const user = userEvent.setup();
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /^Re-imported/ }));

    expect(within(group()).getByLabelText("Original maker")).toBeInTheDocument();
  });

  it("shows no original-marks fields for a domestic or unspecified origin", async () => {
    const user = userEvent.setup();
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    expect(screen.queryByRole("group", { name: "Original maker's marks" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("radio", { name: /^Domestic/ }));
    expect(screen.queryByRole("group", { name: "Original maker's marks" })).not.toBeInTheDocument();
  });

  it("submits a partial set with no message", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderWithOriginGroup(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.click(screen.getByRole("radio", { name: /^Imported/ }));
    await user.type(within(group()).getByLabelText("Original maker"), "Fabrique Nationale");
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit.mock.calls[0][0]).toMatchObject({
      originalMake: "Fabrique Nationale",
      originalModel: null,
      originalSerialNumber: null,
    });
  });

  it("submits all three fields, and null when left blank", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderWithOriginGroup(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.click(screen.getByRole("radio", { name: /^Imported/ }));
    await user.type(within(group()).getByLabelText("Original maker"), "Fabrique Nationale");
    await user.type(within(group()).getByLabelText("Original model"), "High Power");
    await user.type(within(group()).getByLabelText("Original serial number"), "FN-99001");
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(onSubmit.mock.calls[0][0]).toMatchObject({
      originalMake: "Fabrique Nationale",
      originalModel: "High Power",
      originalSerialNumber: "FN-99001",
    });
  });

  it("discards original marks (with importer) when moving away from an import-marked origin", async () => {
    const user = userEvent.setup();
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /^Imported/ }));
    await user.type(within(group()).getByLabelText("Original maker"), "Fabrique Nationale");

    await user.click(screen.getByRole("radio", { name: /^Domestic/ }));
    expect(screen.getByRole("alertdialog")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Discard and change" }));

    expect(screen.queryByRole("group", { name: "Original maker's marks" })).not.toBeInTheDocument();
  });

  it("carries original marks over when moving between Imported and Re-imported", async () => {
    const user = userEvent.setup();
    renderWithOriginGroup(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /^Imported/ }));
    await user.type(within(group()).getByLabelText("Original maker"), "Fabrique Nationale");

    await user.click(screen.getByRole("radio", { name: /^Re-imported/ }));

    expect(within(group()).getByLabelText("Original maker")).toHaveValue("Fabrique Nationale");
  });
});

describe("FirearmForm original-marks warning (US3, FR-009)", () => {
  it("opens a confirm-to-save dialog on ORIGINAL_MARKS_MATCH and resends confirmed on Save anyway", async () => {
    const user = userEvent.setup();
    const message =
      "Ridgeline Arms Hi-Power (serial RA-1) already has these original maker's marks.";
    const onSubmit = vi
      .fn()
      .mockRejectedValueOnce(new CommandFailure({ code: "ORIGINAL_MARKS_MATCH", message }))
      .mockResolvedValueOnce(undefined);
    render(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(
      await screen.findByText("Another firearm has the same original marks"),
    ).toBeInTheDocument();
    expect(screen.getByText(message)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Save anyway" }));

    expect(onSubmit).toHaveBeenCalledTimes(2);
    expect(onSubmit.mock.calls[1][1]).toBe(true);
  });

  it("cancelling the warning saves nothing", async () => {
    const user = userEvent.setup();
    const message =
      "Ridgeline Arms Hi-Power (serial RA-1) already has these original maker's marks.";
    const onSubmit = vi
      .fn()
      .mockRejectedValueOnce(new CommandFailure({ code: "ORIGINAL_MARKS_MATCH", message }));
    render(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(await screen.findByText(message)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Cancel" }));

    expect(onSubmit).toHaveBeenCalledTimes(1);
  });
});

// The origin fields fold into one disclosure so the form stays short for the
// many records that never use them; closing it must never hide a value or an
// error.
describe("FirearmForm origin group disclosure", () => {
  const imported = {
    id: 1,
    make: "Glock",
    model: "17",
    status: "active",
    origin: "imported",
    yearOfManufacture: 1998,
    countryOfManufacture: "Austria",
    importerName: "Glock Inc.",
    originalMake: "Glock GmbH",
  } as Firearm;

  it("starts closed on a new record, saying what it holds", () => {
    render(<FirearmForm onSubmit={vi.fn()} />);

    expect(originGroupButton()).toHaveAttribute("aria-expanded", "false");
    expect(originGroupButton()).toHaveTextContent(
      "Optional: where and when it was made, and who imported it.",
    );
    expect(screen.queryByLabelText("Year of manufacture")).not.toBeInTheDocument();
    expect(screen.queryByRole("radio", { name: /^Domestic/ })).not.toBeInTheDocument();
  });

  it("starts open on a record with any of its fields recorded", () => {
    render(<FirearmForm initialValues={imported} onSubmit={vi.fn()} />);

    expect(originGroupButton()).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByLabelText("Year of manufacture")).toHaveValue("1998");
  });

  it("reads the recorded values back when closed", async () => {
    const user = userEvent.setup();
    render(<FirearmForm initialValues={imported} onSubmit={vi.fn()} />);

    await user.click(originGroupButton());

    expect(originGroupButton()).toHaveAttribute("aria-expanded", "false");
    expect(originGroupButton()).toHaveTextContent(
      "Imported from Austria by Glock Inc. Made in 1998. Original maker's marks recorded.",
    );
  });

  it("still submits values recorded in a closed group", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(
      <FirearmForm
        initialValues={{ ...imported, caliber: "9mm", firearmTypeId: 1, serialNumber: "A1" }}
        onSubmit={onSubmit}
      />,
    );

    await user.click(originGroupButton());
    await user.click(screen.getByRole("button", { name: "Save changes" }));

    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit.mock.calls[0][0]).toMatchObject({
      origin: "imported",
      yearOfManufacture: 1998,
      importerName: "Glock Inc.",
    });
  });

  it("opens and focuses an invalid year when a save is tried with the group closed", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    render(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.click(originGroupButton());
    await user.type(screen.getByLabelText("Year of manufacture"), "1200");
    await user.click(originGroupButton());
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(onSubmit).not.toHaveBeenCalled();
    expect(originGroupButton()).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByLabelText("Year of manufacture")).toHaveFocus();
  });

  it("opens on a server error for one of its fields", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockRejectedValue(
      new CommandFailure({
        code: "VALIDATION_ERROR",
        message: "The firearm record has validation errors.",
        fieldErrors: { importerName: "Importer is too long." },
      }),
    );
    render(
      <FirearmForm
        initialValues={{ ...imported, caliber: "9mm", firearmTypeId: 1, serialNumber: "A1" }}
        onSubmit={onSubmit}
      />,
    );

    await user.click(originGroupButton());
    await user.click(screen.getByRole("button", { name: "Save changes" }));

    expect(originGroupButton()).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("Importer is too long.")).toBeInTheDocument();
  });

  it("opens and brings Year of manufacture into view on an identity clash with no year", async () => {
    const user = userEvent.setup();
    Element.prototype.scrollIntoView = vi.fn();
    const message =
      "Glock 19 (serial ABC123) already has these marks. Or record a year of manufacture on each firearm: two firearms with the same marks are accepted when both have a year and the years differ.";
    const onSubmit = vi.fn().mockRejectedValue(
      new CommandFailure({
        code: "VALIDATION_ERROR",
        message,
        fieldErrors: { serialNumber: message },
      }),
    );
    render(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(originGroupButton()).toHaveAttribute("aria-expanded", "true");
    expect(Element.prototype.scrollIntoView).toHaveBeenCalledTimes(1);
    expect(vi.mocked(Element.prototype.scrollIntoView).mock.contexts[0]).toHaveAttribute(
      "data-field",
      "yearOfManufacture",
    );
  });
});

describe("FirearmForm physical details disclosure", () => {
  const measured = {
    id: 1,
    make: "Glock",
    model: "17",
    caliber: "9mm",
    firearmTypeId: 1,
    serialNumber: "A1",
    status: "active",
    barrelLengthHundredths: 449,
    overallLengthHundredths: 802,
    weightTenthsOz: 400,
    capacity: 17,
    finish: "nDLC",
    condition: "excellent",
  } as Firearm;

  it("starts closed on a new record, saying what it holds", () => {
    render(<FirearmForm onSubmit={vi.fn()} />);

    expect(physicalGroupButton()).toHaveAttribute("aria-expanded", "false");
    expect(physicalGroupButton()).toHaveTextContent(
      "Optional: lengths, weight, capacity, finish and condition.",
    );
    expect(screen.queryByLabelText("Barrel length (in)")).not.toBeInTheDocument();
  });

  it("starts open on a record with any physical detail recorded", () => {
    render(
      <FirearmForm
        initialValues={
          { id: 1, make: "Colt", model: "Python", status: "active", capacity: 6 } as Firearm
        }
        onSubmit={vi.fn()}
      />,
    );

    expect(physicalGroupButton()).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByLabelText("Capacity")).toHaveValue("6");
  });

  it("says how precisely lengths and weight are saved", () => {
    renderWithPhysicalGroup(<FirearmForm onSubmit={vi.fn()} />);

    expect(screen.getByText("Saved to the nearest 0.01 in.")).toBeInTheDocument();
    expect(
      screen.getByText("Fill in either or both. Saved to the nearest 0.1 oz."),
    ).toBeInTheDocument();
  });

  it("reads the recorded values back when closed", async () => {
    const user = userEvent.setup();
    render(<FirearmForm initialValues={measured} onSubmit={vi.fn()} />);

    await user.click(physicalGroupButton());

    expect(physicalGroupButton()).toHaveAttribute("aria-expanded", "false");
    expect(physicalGroupButton()).toHaveTextContent(
      "4.49 in barrel, 8.02 in overall. 2 lb 8 oz. 17 rounds. Finish: nDLC. Condition: Excellent.",
    );
  });

  it("still submits values recorded in a closed group", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<FirearmForm initialValues={measured} onSubmit={onSubmit} />);

    await user.click(physicalGroupButton());
    await user.click(screen.getByRole("button", { name: "Save changes" }));

    expect(onSubmit.mock.calls[0][0]).toMatchObject({
      barrelLengthHundredths: 449,
      weightTenthsOz: 400,
      capacity: 17,
      condition: "excellent",
    });
  });

  it("opens and focuses an invalid field when a save is tried with the group closed", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    render(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.click(physicalGroupButton());
    await user.type(screen.getByLabelText("Capacity"), "0");
    await user.click(physicalGroupButton());
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(onSubmit).not.toHaveBeenCalled();
    expect(physicalGroupButton()).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByLabelText("Capacity")).toHaveFocus();
  });

  it("opens on a server error for one of its fields", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockRejectedValue(
      new CommandFailure({
        code: "VALIDATION_ERROR",
        message: "The firearm record has validation errors.",
        fieldErrors: { weightTenthsOz: "Weight must be greater than 0." },
      }),
    );
    render(<FirearmForm initialValues={measured} onSubmit={onSubmit} />);

    await user.click(physicalGroupButton());
    await user.click(screen.getByRole("button", { name: "Save changes" }));

    expect(physicalGroupButton()).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("Weight must be greater than 0.")).toBeInTheDocument();
  });
});

describe("FirearmForm resuming pending changes (FR-039)", () => {
  afterEach(() => setResumedDraft(null));

  const beretta = {
    id: 4,
    make: "Beretta",
    model: "92FS",
    serialNumber: "BER92-0417",
    noSerialAttested: false,
    caliber: "9mm",
    firearmTypeId: 1,
    status: "active",
    notes: null,
  } as Firearm;

  it("opens with a draft that is short of fields, or has one of another kind, taking the record's values for them", () => {
    // As the human-testing seed keeps it: only some of the form's fields,
    // which once left the others undefined and blanked the window.
    setResumedDraft({
      formVersion: FORM_VERSION,
      kind: "firearm",
      mode: "edit",
      targetId: 4,
      label: "Beretta 92FS — edit",
      values: { make: "Beretta", model: 92, notes: "Swapped the grips; half typed" },
    });

    render(<FirearmForm initialValues={beretta} onSubmit={vi.fn()} />);

    expect(screen.getByLabelText(/Notes/)).toHaveValue("Swapped the grips; half typed");
    expect(screen.getByLabelText(/^Model/)).toHaveValue("92FS");
    expect(screen.getByLabelText(/^Caliber/)).toHaveValue("9mm");
    expect(getDirtyForm()?.label).toBe("Beretta 92FS (edit)");
  });
});

// specs/004-cartridges-action-types User Story 1: the cartridge, and the
// caliber it fills in (contracts/ui-entry.md §3, research.md §8).
describe("FirearmForm cartridge and caliber (US1)", () => {
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

  it("fills Caliber from a built-in cartridge when Cartridge is left (US1-1)", async () => {
    const user = userEvent.setup();
    render(<FirearmForm onSubmit={vi.fn()} />);

    await user.type(cartridgeField(), "9x19mm Parabellum");
    await user.tab();

    await waitFor(() => expect(caliberField()).toHaveValue("9mm"));
    expect(settleEntry).toHaveBeenCalledWith("cartridge", "9x19mm Parabellum");
    expect(screen.getByText("From the cartridge.")).toBeInTheDocument();
    expect(screen.queryByText("Guess")).not.toBeInTheDocument();
  });

  it("marks a guessed caliber with a Guess tag described by its hint (US1-2)", async () => {
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

  it("empties Caliber and asks for it when no bore can be read (US1-4)", async () => {
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

  it("keeps a caliber the user typed when the cartridge changes again (US1-3)", async () => {
    const user = userEvent.setup();
    render(<FirearmForm onSubmit={vi.fn()} />);

    await enterCartridge(user, ".30 Custom Improved");
    await waitFor(() => expect(caliberField()).toHaveValue(".30"));
    await user.clear(caliberField());
    await user.type(caliberField(), ".308");
    await enterCartridge(user, "9x19mm Parabellum");

    // The cartridge settled twice; the caliber typed in between settles too
    // (US2), and its answer changes nothing.
    await waitFor(() =>
      expect(settleEntry.mock.calls.filter(([field]) => field === "cartridge")).toHaveLength(2),
    );
    expect(caliberField()).toHaveValue(".308");
    expect(screen.queryByText("Guess")).not.toBeInTheDocument();
  });

  it("offers the new cartridge's caliber on a saved firearm instead of replacing it (US1-7)", async () => {
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
  ])("shows FR-015's rules for %s when it is left, without truncating", async (label, name) => {
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

describe("FirearmForm drafts of the caliber state (research.md §8)", () => {
  afterEach(() => setResumedDraft(null));

  it("is version 3", () => {
    expect(FORM_VERSION).toBe(3);
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

// specs/004-cartridges-action-types User Story 2: the four entry fields
// suggest and settle (contracts/ui-entry.md §1–§2).
describe("FirearmForm suggestions and snapping (US2)", () => {
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

  it("does not settle an untouched field, or one typed back to its saved value (FR-014)", async () => {
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

// specs/004-cartridges-action-types US3, contracts/ui-entry.md §4: the
// Action choice, filtered by the selected type.
describe("FirearmForm action (US3)", () => {
  const NAMES = [
    "Semi-automatic",
    "Revolver",
    "Bolt action",
    "Lever action",
    "Pump action",
    "Break action",
    "Falling block",
    "Rolling block",
    "Single shot (other)",
    "Flintlock",
    "Percussion",
    "Inline muzzleloader",
  ];
  const ACTIONS = NAMES.map((name, index) => ({ id: index + 1, name }));
  const ids = (...except: number[]) =>
    ACTIONS.map((action) => action.id).filter((id) => !except.includes(id));
  // FR-018: Handgun (1) has no Pump action, Falling block or Inline
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

  it("offers the types in id order, Suppressor last", () => {
    renderForm();

    const names = screen.getAllByRole("radio").map((radio) => radio.closest("label")?.textContent);
    expect(names).toEqual(["Handgun", "Rifle", "Shotgun", "Other", "Suppressor"]);
  });

  it("offers no Action, Barrel length or Capacity for a Suppressor, and says Caliber rating", async () => {
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
    expect(screen.getByLabelText("Caliber rating")).toBeInTheDocument();
    expect(screen.queryByLabelText("Caliber")).not.toBeInTheDocument();
    expect(screen.getByText("The largest bore the suppressor is rated for.")).toBeInTheDocument();
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

  it("marks the five built-in form names in Form's suggestions", async () => {
    suggestEntries.mockResolvedValue(
      ["Form 4", "Form 1", "Form 3", "Form 5", "Form 10"].map((value) => ({
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
      "Form 3 Built-in",
      "Form 5 Built-in",
      "Form 10 Built-in",
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
