import { afterEach, describe, expect, it, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FlatpakReachNote } from "./FlatpakReachNote";

const COMMAND = "flatpak override --user --filesystem=<folder> io.github.exodious.HoploDex";

describe("FlatpakReachNote (#260)", () => {
  afterEach(() => vi.restoreAllMocks());

  it("in a Flatpak, says what it reaches in one line, with a link to how to allow another folder", () => {
    render(<FlatpakReachNote flatpak />);
    expect(
      screen.getByText(/HoploDex can reach Documents\/HoploDex and removable drives\./),
    ).toBeVisible();
    expect(screen.queryByText(COMMAND)).toBeNull();
    expect(screen.getByRole("button", { name: "Allow another folder…" })).toBeVisible();
  });

  it("the link opens a dialog with the command on one line, and Copy puts it on the clipboard", async () => {
    const user = userEvent.setup();
    const writeText = vi.spyOn(navigator.clipboard, "writeText").mockResolvedValue();
    render(<FlatpakReachNote flatpak />);

    await user.click(screen.getByRole("button", { name: "Allow another folder…" }));
    const dialog = screen.getByRole("dialog", { name: "Allow another folder" });
    expect(within(dialog).getByText(/in place of <folder>, then restart HoploDex/)).toBeVisible();
    expect(within(dialog).getByText(COMMAND)).toHaveClass("hd-flatpak-reach__command");

    await user.click(within(dialog).getByRole("button", { name: "Copy" }));
    expect(writeText).toHaveBeenCalledWith(COMMAND);
    expect(within(dialog).getByRole("button", { name: "Copied" })).toBeVisible();

    await user.click(within(dialog).getByRole("button", { name: "Done" }));
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("outside a Flatpak, shows nothing: no line, no link, no command", () => {
    const { container } = render(<FlatpakReachNote flatpak={false} />);
    expect(container).toBeEmptyDOMElement();
    expect(screen.queryByRole("button", { name: "Allow another folder…" })).toBeNull();
  });
});
