import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { todayIso } from "../../lib/dates";
import userEvent from "@testing-library/user-event";
import {
  fillRequired,
  originGroupButton,
  physicalGroupButton,
  render,
  renderWithPhysicalGroup,
  resetServiceMocks,
  selectFirearmType,
  tomorrowIso,
} from "./firearmFormTestSupport";
import { FirearmForm } from "./FirearmForm";
import { CommandFailure } from "../../services/tauriClient";
import type { Firearm } from "./types";

beforeEach(resetServiceMocks);

describe("FirearmForm serial-attestation rule", () => {
  // 001 US1-7
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

  // 001 US1-6
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

  // 001 US1-11
  it("checking attestation disables and does not require the serial number field", async () => {
    const user = userEvent.setup();
    render(<FirearmForm onSubmit={vi.fn()} />);

    const checkbox = screen.getByLabelText("This firearm has no serial number");
    await user.click(checkbox);

    expect(screen.getByLabelText("Serial number")).toBeDisabled();
  });

  // 001 US1-11: a firearm with no serial number is taken out of the
  // duplicate check, so what was typed before is not sent.
  it("submits serialNumber null when 'no serial' is checked after a serial was typed", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    expect(screen.getByLabelText("Serial number")).toHaveValue("ABC123");
    await user.click(screen.getByLabelText("This firearm has no serial number"));
    expect(screen.getByLabelText("Serial number")).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit.mock.calls[0][0]).toMatchObject({
      serialNumber: null,
      noSerialAttested: true,
    });
  });

  // 001 US1-11: unlocking the serial field puts the rule back.
  it("unchecking 'no serial' unlocks the field, asks for a serial again and blocks the save", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<FirearmForm onSubmit={onSubmit} />);

    await user.type(screen.getByLabelText("Make"), "Homemade");
    await user.type(screen.getByLabelText("Model"), "80% build");
    await user.type(screen.getByLabelText("Caliber"), ".223");
    await selectFirearmType(user);
    const checkbox = screen.getByLabelText("This firearm has no serial number");
    await user.click(checkbox);
    expect(screen.getByLabelText("Serial number")).toBeDisabled();

    await user.click(checkbox);

    expect(screen.getByLabelText("Serial number")).toBeEnabled();
    await user.click(screen.getByRole("button", { name: "Add firearm" }));
    expect(
      screen.getByText("Enter a serial number, or confirm this firearm has none."),
    ).toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();
  });
});

describe("FirearmForm amount cap (#67)", () => {
  it("refuses an amount above $99,999,999 before sending, and accepts exactly that", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<FirearmForm onSubmit={onSubmit} />);

    await fillRequired(user);
    await user.type(screen.getByLabelText("Estimated replacement value"), "100000000");
    await user.type(screen.getByLabelText("Price paid"), "100000000");
    await user.click(screen.getByRole("button", { name: "Add firearm" }));

    expect(screen.getAllByText("Enter an amount of $99,999,999 or less.")).toHaveLength(2);
    expect(onSubmit).not.toHaveBeenCalled();

    for (const label of ["Estimated replacement value", "Price paid"]) {
      const field = screen.getByLabelText(label);
      await user.clear(field);
      await user.type(field, "99999999");
    }
    await user.click(screen.getByRole("button", { name: "Add firearm" }));
    expect(onSubmit.mock.calls[0][0]).toMatchObject({
      estimatedValue: 99_999_999,
      acquisitionPrice: 99_999_999,
    });
  });
});

// 001 US1-3 and US1-16: amounts are whole dollars, and editing shows them
// as typed (no grouping, no cents).
describe("FirearmForm amounts when editing (001 FR-037)", () => {
  it("prefills the replacement value and price paid as plain whole dollars", () => {
    render(
      <FirearmForm
        initialValues={
          {
            id: 1,
            make: "Colt",
            model: "Python",
            serialNumber: "V1",
            noSerialAttested: false,
            caliber: ".357",
            firearmTypeId: 1,
            status: "active",
            estimatedValue: 1250,
            acquisitionPrice: 1450,
          } as Firearm
        }
        onSubmit={vi.fn()}
      />,
    );

    expect(screen.getByLabelText("Estimated replacement value")).toHaveValue("1250");
    expect(screen.getByLabelText("Price paid")).toHaveValue("1450");
  });
});

// 008 research.md §10, contracts/ui.md §3: the free-text
// maximums, shown at once and never cut off. The labels are the backend's.
describe("FirearmForm free-text maximums", () => {
  const SHORT = [
    "Nickname",
    "Serial number",
    "Finish",
    "Acquired from",
    "Country of manufacture",
    "Importer",
    "Original maker",
    "Original model",
    "Original serial number",
  ];

  /** The form with the origin (set to Imported) and physical groups open, so
   * every free-text field is on the page. */
  async function renderWithEveryField(onSubmit = vi.fn().mockResolvedValue(undefined)) {
    const user = userEvent.setup();
    const result = render(<FirearmForm onSubmit={onSubmit} />);
    await fillRequired(user);
    await user.click(originGroupButton());
    await user.click(screen.getByRole("radio", { name: /^Imported/ }));
    await user.click(physicalGroupButton());
    return { user, onSubmit, ...result };
  }

  it.each([
    ...SHORT.map((label) => [label, 200] as const),
    ["Notes", 4000] as const,
    ["Accessories", 4000] as const,
  ])("shows %s's maximum of %i at once and refuses to save", async (label, max) => {
    const { user, onSubmit } = await renderWithEveryField();
    const message = `${label} can be at most ${max.toLocaleString("en-US")} characters.`;
    const field = screen.getByLabelText(label);

    fireEvent.change(field, { target: { value: "x".repeat(max) } });
    expect(screen.queryByText(message)).not.toBeInTheDocument();

    // Pasted text is never cut off, and the error shows before leaving the field.
    const long = "x".repeat(max + 1);
    fireEvent.change(field, { target: { value: long } });
    expect(screen.getByText(message)).toBeInTheDocument();
    expect(field).toHaveValue(long);
    expect(field).toHaveAccessibleDescription(expect.stringContaining(message));

    await user.click(screen.getByRole("button", { name: "Add firearm" }));
    expect(onSubmit).not.toHaveBeenCalled();

    fireEvent.change(field, { target: { value: "x".repeat(max) } });
    expect(screen.queryByText(message)).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Add firearm" }));
    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
  });

  it("shows the maximum of the disposition recipient on a disposed record", async () => {
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
      dispositionType: "sold",
      dispositionRecipient: "Jane",
      dispositionDate: "2025-06-01",
      dispositionPrice: 100,
    } as Firearm;
    render(<FirearmForm initialValues={disposed} onSubmit={onSubmit} />);
    const message = "Transferred to can be at most 200 characters.";
    const field = screen.getByLabelText(/^Transferred to/);

    fireEvent.change(field, { target: { value: "x".repeat(201) } });
    expect(screen.getByText(message)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Save changes" }));
    expect(onSubmit).not.toHaveBeenCalled();

    fireEvent.change(field, { target: { value: "x".repeat(200) } });
    await user.click(screen.getByRole("button", { name: "Save changes" }));
    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
  });

  it("counts characters after trimming, shows a count under notes and accessories from 3,600, and sets no maxLength", async () => {
    const { container } = await renderWithEveryField();

    fireEvent.change(screen.getByLabelText("Nickname"), {
      target: { value: `  ${"\u{1F52B}".repeat(200)}  ` },
    });
    expect(screen.queryByText(/can be at most/)).not.toBeInTheDocument();

    for (const label of ["Notes", "Accessories"]) {
      const field = screen.getByLabelText(label);
      fireEvent.change(field, { target: { value: "x".repeat(3599) } });
      expect(screen.queryByText(/of 4,000/)).not.toBeInTheDocument();
      fireEvent.change(field, { target: { value: "x".repeat(3600) } });
      expect(screen.getByText("3,600 of 4,000")).toBeInTheDocument();
      fireEvent.change(field, { target: { value: "" } });
    }

    expect(container.querySelectorAll("[maxlength]")).toHaveLength(0);
  });
});

// 001 US1-13 and US1-14
describe("FirearmForm date rules (001 FR-003 / FR-004)", () => {
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

describe("FirearmForm nickname (001 FR-031)", () => {
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

// 001 US1-17
describe("FirearmForm physical details (001 FR-039, US1-17)", () => {
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

  // 001 US1-17: clearing all six removes the Physical details panel
  // from the record, so every one is sent as null.
  it("submits all six as null after a record's physical details are cleared", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    renderWithPhysicalGroup(
      <FirearmForm
        initialValues={
          {
            id: 1,
            make: "Colt",
            model: "Python",
            serialNumber: "V1",
            noSerialAttested: false,
            caliber: ".357",
            firearmTypeId: 1,
            status: "active",
            barrelLengthHundredths: 1625,
            overallLengthHundredths: 1800,
            weightTenthsOz: 405,
            capacity: 6,
            finish: "Blued",
            condition: "excellent",
          } as Firearm
        }
        onSubmit={onSubmit}
      />,
    );
    expect(barrel()).toHaveValue("16.25");

    await user.clear(barrel());
    await user.clear(overall());
    await user.clear(weightLb());
    await user.clear(weightOz());
    await user.clear(capacity());
    await user.clear(finish());
    await pickCondition(user, "Not recorded");
    await user.click(screen.getByRole("button", { name: "Save changes" }));

    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit.mock.calls[0][0]).toMatchObject({
      barrelLengthHundredths: null,
      overallLengthHundredths: null,
      weightTenthsOz: null,
      capacity: null,
      finish: null,
      condition: null,
    });
  });
});

describe("FirearmForm focusField (001 FR-038, US1-15)", () => {
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
