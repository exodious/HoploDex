import { useState } from "react";
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { NumberField } from "./NumberField";

function Harness({
  initial = "5",
  disabled,
  error,
}: {
  initial?: string;
  disabled?: boolean;
  error?: string;
}) {
  const [value, setValue] = useState(initial);
  return (
    <NumberField
      label="Keep the latest"
      value={value}
      onValueChange={setValue}
      min={1}
      max={100}
      suffix="backups"
      disabled={disabled}
      error={error}
    />
  );
}

const field = () => screen.getByRole("spinbutton", { name: "Keep the latest" });
const up = () => screen.getByRole("button", { name: "Increase Keep the latest" });
const down = () => screen.getByRole("button", { name: "Decrease Keep the latest" });

describe("NumberField (issue #35)", () => {
  it("is a spin button with its range, a unit after it and named step buttons", () => {
    render(<Harness />);

    expect(field()).toHaveValue("5");
    expect(field()).toHaveAttribute("aria-valuemin", "1");
    expect(field()).toHaveAttribute("aria-valuemax", "100");
    expect(field()).toHaveAttribute("aria-valuenow", "5");
    expect(screen.getByText("backups")).toBeInTheDocument();
    expect(up()).toBeInTheDocument();
    expect(down()).toBeInTheDocument();
  });

  it("still takes typed text, as typed", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await user.clear(field());
    await user.type(field(), "12");
    expect(field()).toHaveValue("12");
    await user.clear(field());
    await user.type(field(), "abc");

    expect(field()).toHaveValue("abc");
    expect(field()).not.toHaveAttribute("aria-valuenow");
  });

  it("steps with the buttons and keeps the focus in the input", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    field().focus();

    await user.click(up());
    await user.click(up());
    await user.click(down());

    expect(field()).toHaveValue("6");
    expect(field()).toHaveFocus();
    expect(up()).toHaveAttribute("tabindex", "-1");
  });

  it("steps with ArrowUp and ArrowDown", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await user.click(field());
    await user.keyboard("{ArrowUp}{ArrowUp}{ArrowUp}{ArrowDown}");

    expect(field()).toHaveValue("7");
  });

  it("stays within the range: the buttons stop at the ends and the keys don't pass them", async () => {
    const user = userEvent.setup();
    render(<Harness initial="1" />);

    expect(down()).toBeDisabled();
    expect(up()).toBeEnabled();
    await user.click(field());
    await user.keyboard("{ArrowDown}");
    expect(field()).toHaveValue("1");

    await user.clear(field());
    await user.type(field(), "100");
    expect(up()).toBeDisabled();
    await user.keyboard("{ArrowUp}");
    expect(field()).toHaveValue("100");
  });

  it("brings a typed number outside the range back in when stepped", async () => {
    const user = userEvent.setup();
    render(<Harness initial="500" />);

    await user.click(down());

    expect(field()).toHaveValue("100");
  });

  it("starts from the lowest number when the text isn't a number", async () => {
    const user = userEvent.setup();
    render(<Harness initial="" />);

    await user.click(up());

    expect(field()).toHaveValue("1");
  });

  it("takes no input while disabled, and looks disabled", async () => {
    const user = userEvent.setup();
    render(<Harness disabled />);

    expect(field()).toBeDisabled();
    expect(up()).toBeDisabled();
    expect(down()).toBeDisabled();
    expect(field().closest(".hd-input")).toHaveClass("hd-input--disabled");
    expect(field().closest(".hd-field")).toHaveAttribute("data-disabled");
    await user.click(up());
    expect(field()).toHaveValue("5");
  });

  it("shows the form's error against the field", () => {
    render(<Harness error="Enter a whole number of backups." />);

    expect(field()).toHaveAccessibleDescription("Enter a whole number of backups.");
  });
});
