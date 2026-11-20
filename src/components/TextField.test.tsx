import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { TextField } from "./TextField";

// Code scanning alert plaintext-lifetime/cwe-312: WebView2's autofill kept what was
// typed into record fields unencrypted on disk. main.rs turns it off for the window,
// and every field asks for it off as well.
describe("TextField autofill", () => {
  it("asks the web view not to autofill or keep what is typed", () => {
    render(<TextField label="Serial number" value="" onChange={() => {}} />);
    expect(screen.getByLabelText("Serial number")).toHaveAttribute("autocomplete", "off");
  });
});
