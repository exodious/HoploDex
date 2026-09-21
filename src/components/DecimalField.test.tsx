import { useState } from "react";
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DecimalField } from "./DecimalField";

function Harness({ places = 2, initial = "" }: { places?: number; initial?: string }) {
  const [value, setValue] = useState(initial);
  return (
    <DecimalField label="Barrel length" places={places} value={value} onValueChange={setValue} />
  );
}

const field = () => screen.getByLabelText("Barrel length") as HTMLInputElement;

describe("DecimalField (FR-039)", () => {
  it("accepts digits and a single decimal point, ignoring letters", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await user.type(field(), "1a6.2x5");

    expect(field().value).toBe("16.25");
  });

  it("ignores a second decimal point", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await user.type(field(), "1.2.3");

    expect(field().value).toBe("1.23");
  });

  it("shows a field-level message for too many decimal places and never rounds", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await user.type(field(), "16.255");

    expect(field().value).toBe("16.255");
    expect(screen.getByRole("alert")).toHaveTextContent(/2 decimal places/);
    expect(field()).toHaveAttribute("aria-invalid", "true");
  });

  it("uses the places it is given", async () => {
    const user = userEvent.setup();
    render(<Harness places={1} />);

    await user.type(field(), "40.55");

    expect(screen.getByRole("alert")).toHaveTextContent(/1 decimal place/);
  });

  it("clears the message once the value fits again", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await user.type(field(), "16.255");
    await user.type(field(), "{Backspace}");

    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(field().value).toBe("16.25");
  });

  it("drops a pasted dollar sign, commas and spaces", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await user.click(field());
    await user.paste(" $1,016.25 ");

    expect(field().value).toBe("1016.25");
  });

  it("shows a message from the form in preference to its own", () => {
    render(
      <DecimalField
        label="Barrel length"
        places={2}
        value="0"
        onValueChange={() => {}}
        error="Must be greater than 0."
      />,
    );

    expect(screen.getByRole("alert")).toHaveTextContent("Must be greater than 0.");
  });
});
