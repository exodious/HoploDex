import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { ReactElement } from "react";
import { todayIso } from "../../lib/dates";
import userEvent from "@testing-library/user-event";
import { getDirtyForm, setResumedDraft } from "../session/usePendingDraft";
import { FORM_VERSION, FirearmForm } from "./FirearmForm";
import { CommandFailure } from "../../services/tauriClient";
import type { DerivedCaliber, Firearm } from "./types";

// specs/004-cartridges-action-types: the form settles an entry through the
// backend (`settle_entry`); here a stand-in derives a few known calibers.
const settleEntry = vi.fn();
vi.mock("./firearmsService", () => ({
  settleEntry: (field: string, text: string) => settleEntry(field, text),
}));

const DERIVED: Record<string, DerivedCaliber> = {
  "9x19mm Parabellum": { caliber: "9mm", source: "catalog" },
  "9mm Luger": { caliber: "9mm", source: "catalog" },
  ".45 ACP": { caliber: ".45", source: "catalog" },
  ".30 Custom Improved": { caliber: ".30", source: "guess" },
  "6.5x47 Wildcat": { caliber: "6.5mm", source: "guess" },
};

beforeEach(() => {
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
      screen.getByRole("dialog", { name: "How to record where a firearm came from" }),
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

    await waitFor(() => expect(settleEntry).toHaveBeenCalledTimes(2));
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
  ])("shows FR-015's rules for %s when it is left, without truncating", (label, name) => {
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

  it("is version 2", () => {
    expect(FORM_VERSION).toBe(2);
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

  it("restores a guessed caliber, still derived, from a version 2 draft", async () => {
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
