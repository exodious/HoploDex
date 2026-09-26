import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Button } from "./Button";
import { Menu, MenuItem, MenuSeparator } from "./Menu";

function renderMenu() {
  const handlers = { lock: vi.fn(), switchDb: vi.fn(), close: vi.fn() };
  render(
    <Menu trigger={<Button>Main collection</Button>}>
      <MenuItem onSelect={handlers.lock} shortcut="Ctrl+L">
        Lock now
      </MenuItem>
      <MenuItem onSelect={handlers.switchDb}>Switch database…</MenuItem>
      <MenuSeparator />
      <MenuItem onSelect={handlers.close} note="The file stays where it is.">
        Close database
      </MenuItem>
    </Menu>,
  );
  return handlers;
}

describe("Menu (specs/003 contracts/ui-databases.md §0)", () => {
  it("opens a menu of menu items from its button", async () => {
    const user = userEvent.setup();
    renderMenu();
    const trigger = screen.getByRole("button", { name: "Main collection" });
    expect(trigger).toHaveAttribute("aria-haspopup", "menu");
    expect(trigger).toHaveAttribute("aria-expanded", "false");

    await user.click(trigger);

    const menu = await screen.findByRole("menu");
    expect(trigger).toHaveAttribute("aria-expanded", "true");
    expect(screen.getAllByRole("menuitem").map((item) => item.textContent)).toEqual([
      "Lock nowCtrl+L",
      "Switch database…",
      "Close databaseThe file stays where it is.",
    ]);
    expect(menu).toContainElement(screen.getByRole("separator"));
  });

  it("moves between items with the arrow keys and runs the chosen one", async () => {
    const user = userEvent.setup();
    const handlers = renderMenu();
    screen.getByRole("button", { name: "Main collection" }).focus();

    await user.keyboard("{Enter}");
    const items = await screen.findAllByRole("menuitem");
    await waitFor(() => expect(items[0]).toHaveFocus());
    await user.keyboard("{ArrowDown}");
    expect(items[1]).toHaveFocus();
    await user.keyboard("{ArrowDown}");
    expect(items[2]).toHaveFocus();
    await user.keyboard("{ArrowDown}");
    expect(items[0]).toHaveFocus(); // loops
    await user.keyboard("{ArrowUp}{Enter}");

    expect(handlers.close).toHaveBeenCalledTimes(1);
    expect(handlers.lock).not.toHaveBeenCalled();
    expect(handlers.switchDb).not.toHaveBeenCalled();
    await waitFor(() => expect(screen.queryByRole("menu")).not.toBeInTheDocument());
  });

  it("closes on Escape and returns focus to its button", async () => {
    const user = userEvent.setup();
    const handlers = renderMenu();
    const trigger = screen.getByRole("button", { name: "Main collection" });

    await user.click(trigger);
    await screen.findByRole("menu");
    await user.keyboard("{Escape}");

    await waitFor(() => expect(screen.queryByRole("menu")).not.toBeInTheDocument());
    expect(trigger).toHaveFocus();
    expect(Object.values(handlers).every((handler) => handler.mock.calls.length === 0)).toBe(true);
  });
});
