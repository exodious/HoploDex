import { useState } from "react";
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { todayIso } from "../lib/dates";
import { DateField } from "./DateField";

function Harness() {
  const [value, setValue] = useState("");
  return <DateField label="Date acquired" value={value} onValueChange={setValue} />;
}

describe("DateField", () => {
  it("fills the field and closes the calendar as soon as a day is picked", async () => {
    // Regression (spec_TODO): the native WebKitGTK date picker stayed open
    // after a date was chosen.
    const user = userEvent.setup();
    render(<Harness />);

    await user.click(screen.getByRole("button", { name: "Choose date acquired from a calendar" }));
    const fifteenth = `${todayIso().slice(0, 8)}15`;
    const day = document.querySelector<HTMLButtonElement>(`[data-iso="${fifteenth}"]`);
    expect(day).not.toBeNull();
    await user.click(day!);

    expect(screen.getByLabelText("Date acquired")).toHaveValue(fifteenth);
    expect(screen.queryByRole("button", { name: "Previous month" })).not.toBeInTheDocument();
  });

  it("normalizes a typed U.S. date to ISO when the field loses focus", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await user.type(screen.getByLabelText("Date acquired"), "3/14/2019");
    await user.tab();

    expect(screen.getByLabelText("Date acquired")).toHaveValue("2019-03-14");
  });
});
