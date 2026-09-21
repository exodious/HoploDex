import { useState } from "react";
import { describe, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MoneyField } from "./MoneyField";

function Harness({ initial = "" }: { initial?: string }) {
  const [value, setValue] = useState(initial);
  return <MoneyField label="Estimated value" value={value} onValueChange={setValue} />;
}

const field = () => screen.getByLabelText("Estimated value") as HTMLInputElement;

describe("MoneyField (FR-037, US1 Acceptance Scenario 16)", () => {
  it("accepts digits only while typing: a decimal point, comma or other character is ignored", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await user.type(field(), "1,2.5x0$");

    expect(field().value).toBe("1250");
  });

  it("shows digits with no grouping while editing", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await user.type(field(), "1250000");

    expect(field().value).toBe("1250000");
  });

  it("rejects a pasted amount with cents, with a whole-dollar message, and changes nothing", async () => {
    const user = userEvent.setup();
    render(<Harness initial="900" />);

    await user.click(field());
    await user.paste("1250.50");

    expect(field().value).toBe("900");
    expect(screen.getByRole("alert")).toHaveTextContent(/whole dollars/i);
    expect(field()).toHaveAttribute("aria-invalid", "true");
  });

  it("clears the paste message once the user types again", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await user.click(field());
    await user.paste("1250.50");
    expect(screen.getByRole("alert")).toBeInTheDocument();

    await user.type(field(), "7");

    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(field().value).toBe("7");
  });

  it("drops a pasted dollar sign, thousands commas and spaces", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await user.click(field());
    await user.paste(" $1,250 ");

    expect(field().value).toBe("1250");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("does not regroup the text when the field loses focus", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await user.type(field(), "1250");
    fireEvent.blur(field());

    expect(field().value).toBe("1250");
  });
});
