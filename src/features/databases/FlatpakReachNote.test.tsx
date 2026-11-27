import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { FlatpakReachNote } from "./FlatpakReachNote";

describe("FlatpakReachNote (#260)", () => {
  it("in a Flatpak, says what it reaches and gives the override command to copy", () => {
    render(<FlatpakReachNote flatpak />);
    expect(screen.getByText(/by default it can reach only the HoploDex folder/)).toBeVisible();
    expect(
      screen.getByText("flatpak override --user --filesystem=<folder> io.github.exodious.HoploDex"),
    ).toHaveClass("hd-flatpak-reach__command");
  });

  it("outside a Flatpak, says nothing", () => {
    const { container } = render(<FlatpakReachNote flatpak={false} />);
    expect(container).toBeEmptyDOMElement();
  });
});
