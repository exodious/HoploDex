import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FirearmForm } from "./FirearmForm";

async function selectFirearmType(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("combobox", { name: "Type" }));
  await user.click(await screen.findByRole("option", { name: "Handgun" }));
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
