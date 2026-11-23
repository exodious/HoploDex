import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ThemeToggle } from "./ThemeToggle";

const themeAttribute = () => document.documentElement.getAttribute("data-theme");

const backend = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("../../services/tauriClient", () => ({ invoke: backend.invoke }));

function resetTheme() {
  document.documentElement.removeAttribute("data-theme");
  backend.invoke.mockReset().mockResolvedValue(undefined);
}

// 001 ui-review (PR #1): the color-mode control. Auto follows the system by
// leaving `data-theme` off <html>; Light and Dark set it.
describe("the color mode control", () => {
  beforeEach(resetTheme);
  afterEach(resetTheme);

  it("switches between light, dark and auto, setting and clearing data-theme", async () => {
    const user = userEvent.setup();
    render(<ThemeToggle />);
    expect(themeAttribute()).toBeNull();
    expect(screen.getByRole("radio", { name: "Auto (match system)" })).toBeChecked();

    await user.click(screen.getByRole("radio", { name: "Dark" }));
    expect(themeAttribute()).toBe("dark");
    expect(screen.getByRole("radio", { name: "Dark" })).toBeChecked();

    await user.click(screen.getByRole("radio", { name: "Light" }));
    expect(themeAttribute()).toBe("light");

    await user.click(screen.getByRole("radio", { name: "Auto (match system)" }));
    expect(themeAttribute()).toBeNull();
    // Kept on this computer, not in the web view (code scanning alert
    // plaintext-lifetime/cwe-922).
    expect(backend.invoke.mock.calls).toEqual([
      ["set_appearance", { appearance: { theme: "dark", browseView: "list" } }],
      ["set_appearance", { appearance: { theme: "light", browseView: "list" } }],
      ["set_appearance", { appearance: { theme: "auto", browseView: "list" } }],
    ]);
  });
});
