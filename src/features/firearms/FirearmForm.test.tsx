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

  it("blocks the save with a field-level message for too many decimal places, and never rounds", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.type(barrel(), "16.255");
    await user.type(weightLb(), "6.6251");
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(screen.getByText("Use at most 2 decimal places.")).toBeInTheDocument();
    expect(screen.getByText("Use at most 3 decimal places.")).toBeInTheDocument();
    expect(barrel()).toHaveValue("16.255");
    expect(onSubmit).not.toHaveBeenCalled();
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
