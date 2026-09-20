import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { todayIso } from "../../lib/dates";
import userEvent from "@testing-library/user-event";
import { FirearmForm } from "./FirearmForm";
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
