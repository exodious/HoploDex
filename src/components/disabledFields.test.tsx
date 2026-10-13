import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Checkbox } from "./Checkbox";
import { Select } from "./Select";
import { TextArea } from "./TextArea";
import { TextField } from "./TextField";

// Every shared control marks its label and field as disabled, which components.css (with the
// --disabled-* tokens) turns into the one disabled look (issue #35).
describe("disabled fields look disabled", () => {
  it("TextField: the field, its label and the input wrapper", () => {
    render(<TextField label="Name" disabled />);

    expect(screen.getByLabelText("Name").closest(".hd-field")).toHaveAttribute("data-disabled");
    expect(screen.getByLabelText("Name").closest(".hd-input")).toHaveClass("hd-input--disabled");
  });

  it("Select: the field around the trigger", () => {
    render(<Select label="After" value="1" onValueChange={() => {}} options={[]} disabled />);

    expect(screen.getByRole("combobox", { name: "After" }).closest(".hd-field")).toHaveAttribute(
      "data-disabled",
    );
  });

  it("TextArea: the field around it", () => {
    render(<TextArea label="Notes" disabled />);

    expect(screen.getByLabelText("Notes").closest(".hd-field")).toHaveAttribute("data-disabled");
  });

  it("Checkbox: the label and hint", () => {
    render(
      <Checkbox label="Lock" checked={false} onCheckedChange={() => {}} hint="Hint" disabled />,
    );

    expect(screen.getByRole("checkbox", { name: "Lock" }).closest(".hd-checkbox")).toHaveAttribute(
      "data-disabled",
    );
  });

  it("leaves an enabled field unmarked", () => {
    render(<TextField label="Name" />);

    expect(screen.getByLabelText("Name").closest(".hd-field")).not.toHaveAttribute("data-disabled");
  });
});
