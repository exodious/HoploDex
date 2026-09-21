import { useState } from "react";
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DecimalField } from "./DecimalField";

function Harness({ initial = "" }: { initial?: string }) {
  const [value, setValue] = useState(initial);
  return <DecimalField label="Barrel length" value={value} onValueChange={setValue} />;
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

  it("keeps extra decimal places as typed, with no error (the parser rounds on submit)", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    await user.type(field(), "16.255");

    expect(field().value).toBe("16.255");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
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
        value="0"
        onValueChange={() => {}}
        error="Must be greater than 0."
      />,
    );

    expect(screen.getByRole("alert")).toHaveTextContent("Must be greater than 0.");
  });
});
