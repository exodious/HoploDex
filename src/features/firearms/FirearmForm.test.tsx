import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen, within } from "@testing-library/react";
import { todayIso } from "../../lib/dates";
import userEvent from "@testing-library/user-event";
import { FirearmForm } from "./FirearmForm";
import { CommandFailure } from "../../services/tauriClient";
import type { Firearm } from "./types";

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
    render(<FirearmForm onSubmit={vi.fn()} />);

    for (const field of [barrel(), overall(), weightLb(), weightOz(), capacity(), finish()]) {
      expect(field).toBeInTheDocument();
    }
    expect(within(group()).getByRole("combobox", { name: "Condition" })).toBeInTheDocument();
  });

  it("offers Not recorded and the six grades, best first", async () => {
    const user = userEvent.setup();
    render(<FirearmForm onSubmit={vi.fn()} />);

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
    render(<FirearmForm onSubmit={onSubmit} />);

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
    render(<FirearmForm onSubmit={onSubmit} />);

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
    render(<FirearmForm onSubmit={onSubmit} />);

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
    render(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.type(weightLb(), "0");
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(screen.getByText("Must be greater than 0.")).toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("submits null for Not recorded after a grade was chosen", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await pickCondition(user, "Good");
    await pickCondition(user, "Not recorded");
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(onSubmit.mock.calls[0][0].condition).toBeNull();
  });

  it("prefills from the record", () => {
    render(
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
    render(<FirearmForm onSubmit={vi.fn()} />);

    await user.type(capacity(), "1a2.5");

    expect(capacity()).toHaveValue("125");
  });

  it("rounds extra decimal places to the stored unit instead of blocking", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<FirearmForm onSubmit={onSubmit} />);

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
    render(<FirearmForm onSubmit={onSubmit} />);

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
    render(<FirearmForm onSubmit={onSubmit} />);

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
  it("offers Domestic/Imported/Re-imported/Not specified with their one-line descriptions", () => {
    render(<FirearmForm onSubmit={vi.fn()} />);

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
      screen.getByRole("radio", { name: /^Not specified Leave this if you're not sure\.$/ }),
    ).toBeInTheDocument();
  });

  it("starts a new record on Not specified", () => {
    render(<FirearmForm onSubmit={vi.fn()} />);
    expect(screen.getByRole("radio", { name: /^Not specified/ })).toBeChecked();
  });

  it("selecting Imported reveals Country of manufacture and Importer, both optional", async () => {
    const user = userEvent.setup();
    render(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /^Imported/ }));

    expect(screen.getByLabelText("Country of manufacture")).not.toBeRequired();
    expect(screen.getByLabelText("Importer")).not.toBeRequired();
    expect(screen.queryByText("Country of manufacture: United States")).not.toBeInTheDocument();
  });

  it("selecting Re-imported reveals only Importer plus a read-only United States country line", async () => {
    const user = userEvent.setup();
    render(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /^Re-imported/ }));

    expect(screen.getByLabelText("Importer")).toBeInTheDocument();
    expect(screen.queryByLabelText("Country of manufacture")).not.toBeInTheDocument();
    expect(screen.getByText("Country of manufacture: United States")).toBeInTheDocument();
  });

  it("shows neither field for Domestic or Not specified", async () => {
    const user = userEvent.setup();
    render(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /^Domestic/ }));
    expect(screen.queryByLabelText("Country of manufacture")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Importer")).not.toBeInTheDocument();

    await user.click(screen.getByRole("radio", { name: /^Not specified/ }));
    expect(screen.queryByLabelText("Country of manufacture")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Importer")).not.toBeInTheDocument();
  });

  it("shows the Domestic cue to consider Re-imported", async () => {
    const user = userEvent.setup();
    render(<FirearmForm onSubmit={vi.fn()} />);

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
    render(<FirearmForm onSubmit={onSubmit} />);

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
    render(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.type(screen.getByLabelText("Year of manufacture"), "1943");
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit.mock.calls[0][0].yearOfManufacture).toBe(1943);
  });

  it("asks before discarding importer and country when moving away from an import-marked origin", async () => {
    const user = userEvent.setup();
    render(<FirearmForm onSubmit={vi.fn()} />);

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
    render(<FirearmForm onSubmit={vi.fn()} />);

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
    render(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /^Imported/ }));
    await user.click(screen.getByRole("radio", { name: /^Domestic/ }));

    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(screen.getByRole("radio", { name: /^Domestic/ })).toBeChecked();
  });

  it("opens the origin guide from the How do I record this? button", async () => {
    const user = userEvent.setup();
    render(<FirearmForm onSubmit={vi.fn()} />);

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
    render(<FirearmForm onSubmit={vi.fn()} />);

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
    render(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /^Re-imported/ }));

    expect(within(group()).getByLabelText("Original maker")).toBeInTheDocument();
  });

  it("shows no original-marks fields for a domestic or unspecified origin", async () => {
    const user = userEvent.setup();
    render(<FirearmForm onSubmit={vi.fn()} />);

    expect(screen.queryByRole("group", { name: "Original maker's marks" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("radio", { name: /^Domestic/ }));
    expect(screen.queryByRole("group", { name: "Original maker's marks" })).not.toBeInTheDocument();
  });

  it("submits a partial set with no message", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<FirearmForm onSubmit={onSubmit} />);

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
    render(<FirearmForm onSubmit={onSubmit} />);

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
    render(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /^Imported/ }));
    await user.type(within(group()).getByLabelText("Original maker"), "Fabrique Nationale");

    await user.click(screen.getByRole("radio", { name: /^Domestic/ }));
    expect(screen.getByRole("alertdialog")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Discard and change" }));

    expect(screen.queryByRole("group", { name: "Original maker's marks" })).not.toBeInTheDocument();
  });

  it("carries original marks over when moving between Imported and Re-imported", async () => {
    const user = userEvent.setup();
    render(<FirearmForm onSubmit={vi.fn()} />);

    await user.click(screen.getByRole("radio", { name: /^Imported/ }));
    await user.type(within(group()).getByLabelText("Original maker"), "Fabrique Nationale");

    await user.click(screen.getByRole("radio", { name: /^Re-imported/ }));

    expect(within(group()).getByLabelText("Original maker")).toHaveValue("Fabrique Nationale");
  });
});
