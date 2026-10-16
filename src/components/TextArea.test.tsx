import { useState } from "react";
import { describe, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { TextArea } from "./TextArea";

// 008 contracts/ui.md §3: a long text area with a
// `limit` shows "<n> of 4,000" under the field from 90% of the limit.

function Harness({ initial = "", limit }: { initial?: string; limit?: number }) {
  const [value, setValue] = useState(initial);
  return (
    <TextArea
      label="Notes"
      value={value}
      limit={limit}
      onChange={(e) => setValue(e.target.value)}
    />
  );
}

describe("TextArea counter", () => {
  it("shows no counter below 3,600 characters", () => {
    render(<Harness initial={"a".repeat(3599)} limit={4000} />);
    expect(screen.queryByText(/of 4,000/)).toBeNull();
    expect(screen.getByLabelText("Notes").getAttribute("aria-describedby")).toBeNull();
  });

  it("shows no counter without a limit, however long the text", () => {
    render(<Harness initial={"a".repeat(5000)} />);
    expect(screen.queryByText(/ of /)).toBeNull();
  });

  it("shows '<n> of 4,000' from 3,600 characters, linked and announced politely", () => {
    render(<Harness initial={"a".repeat(3600)} limit={4000} />);
    const counter = screen.getByText("3,600 of 4,000");
    expect(counter.getAttribute("aria-live")).toBe("polite");
    expect(screen.getByLabelText("Notes").getAttribute("aria-describedby")).toContain(counter.id);
    expect(counter.className).not.toContain("hd-textarea__counter--over");
  });

  it("appears as the text grows past 3,600 and goes when it shrinks", () => {
    render(<Harness initial={"a".repeat(3000)} limit={4000} />);
    const box = screen.getByLabelText("Notes");
    expect(screen.queryByText(/of 4,000/)).toBeNull();
    fireEvent.change(box, { target: { value: "a".repeat(3700) } });
    expect(screen.getByText("3,700 of 4,000")).toBeTruthy();
    fireEvent.change(box, { target: { value: "a".repeat(10) } });
    expect(screen.queryByText(/of 4,000/)).toBeNull();
  });

  it("counts characters, not UTF-16 units", () => {
    render(<Harness initial={"😀".repeat(3600)} limit={4000} />);
    expect(screen.getByText("3,600 of 4,000")).toBeTruthy();
  });

  it("counts an uncontrolled field too", () => {
    render(<TextArea label="Notes" limit={4000} defaultValue={"a".repeat(3650)} />);
    expect(screen.getByText("3,650 of 4,000")).toBeTruthy();
  });

  it("takes the error style over 4,000 characters", () => {
    render(<Harness initial={"a".repeat(4001)} limit={4000} />);
    const counter = screen.getByText("4,001 of 4,000");
    expect(counter.className).toContain("hd-textarea__counter--over");
  });

  it("stays at the limit without the error style", () => {
    render(<Harness initial={"a".repeat(4000)} limit={4000} />);
    expect(screen.getByText("4,000 of 4,000").className).not.toContain(
      "hd-textarea__counter--over",
    );
  });

  it("keeps the hint and error in the description beside the counter", () => {
    render(
      <TextArea
        label="Notes"
        limit={4000}
        hint="Anything else"
        error="Notes can be at most 4,000 characters."
        value={"a".repeat(4001)}
        onChange={() => {}}
      />,
    );
    const described = (screen.getByLabelText("Notes").getAttribute("aria-describedby") ?? "").split(
      " ",
    );
    expect(described).toHaveLength(3);
    expect(screen.getByText("4,001 of 4,000").id).toBe(described[2]);
  });
});
