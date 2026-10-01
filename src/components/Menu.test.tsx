import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Button } from "./Button";
import { Menu, MenuLabel, MenuRadioGroup, MenuRadioItem, MenuItem, MenuSeparator } from "./Menu";
import { useState } from "react";

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

function RadioMenu({ onChange }: { onChange?: (value: string) => void }) {
  const [value, setValue] = useState("caliber");
  const change = (next: string) => {
    setValue(next);
    onChange?.(next);
  };
  return (
    <Menu trigger={<Button>Group by {value}</Button>}>
      <MenuRadioGroup value={value} onValueChange={change}>
        <MenuRadioItem value="none">None</MenuRadioItem>
      </MenuRadioGroup>
      <MenuSeparator />
      <MenuRadioGroup value={value} onValueChange={change} label="The firearm">
        <MenuRadioItem value="type">Type</MenuRadioItem>
        <MenuRadioItem value="caliber">Caliber</MenuRadioItem>
      </MenuRadioGroup>
      <MenuRadioGroup value={value} onValueChange={change} label="Registration">
        <MenuRadioItem value="registered_as">Registered as</MenuRadioItem>
        <MenuRadioItem value="registered_to">Registered to</MenuRadioItem>
      </MenuRadioGroup>
    </Menu>
  );
}

describe("Menu radio items (specs/005 contracts/ui-registration.md §6)", () => {
  it("gives items menuitemradio roles and groups named by their label", async () => {
    const user = userEvent.setup();
    render(<RadioMenu />);
    await user.click(screen.getByRole("button", { name: /Group by/ }));

    const items = await screen.findAllByRole("menuitemradio");
    expect(items.map((i) => i.textContent)).toEqual([
      "None",
      "Type",
      "Caliber",
      "Registered as",
      "Registered to",
    ]);
    const groups = screen.getAllByRole("group");
    expect(groups).toHaveLength(3);
    expect(screen.getByRole("group", { name: "The firearm" })).toContainElement(items[1]);
    expect(screen.getByRole("group", { name: "Registration" })).toContainElement(items[3]);
    expect(groups[0]).not.toHaveAccessibleName();
  });

  it("checks exactly one item across the groups and opens on it", async () => {
    const user = userEvent.setup();
    render(<RadioMenu />);
    await user.click(screen.getByRole("button", { name: /Group by/ }));

    const items = await screen.findAllByRole("menuitemradio");
    expect(items.filter((i) => i.getAttribute("aria-checked") === "true")).toEqual([items[2]]);
    await waitFor(() => expect(items[2]).toHaveFocus());
  });

  it("moves across groups, to the ends and by typeahead", async () => {
    const user = userEvent.setup();
    render(<RadioMenu />);
    await user.click(screen.getByRole("button", { name: /Group by/ }));
    const items = await screen.findAllByRole("menuitemradio");
    await waitFor(() => expect(items[2]).toHaveFocus());

    await user.keyboard("{ArrowDown}");
    expect(items[3]).toHaveFocus();
    await user.keyboard("{ArrowUp}{ArrowUp}");
    expect(items[1]).toHaveFocus();
    await user.keyboard("{End}");
    expect(items[4]).toHaveFocus();
    await user.keyboard("{Home}");
    expect(items[0]).toHaveFocus();
    await user.keyboard("r");
    expect(items[3]).toHaveFocus();
  });

  it("chooses with Enter or Space and closes", async () => {
    const user = userEvent.setup();
    const changes: string[] = [];
    render(<RadioMenu onChange={(v) => changes.push(v)} />);
    await user.click(screen.getByRole("button", { name: /Group by/ }));
    let items = await screen.findAllByRole("menuitemradio");
    await waitFor(() => expect(items[2]).toHaveFocus());
    await user.keyboard("{ArrowDown}{Enter}");
    expect(changes).toEqual(["registered_as"]);
    await waitFor(() => expect(screen.queryByRole("menu")).not.toBeInTheDocument());

    await user.click(screen.getByRole("button", { name: "Group by registered_as" }));
    items = await screen.findAllByRole("menuitemradio");
    await waitFor(() => expect(items[3]).toHaveFocus());
    await user.keyboard("{ArrowDown}[Space]");
    expect(changes).toEqual(["registered_as", "registered_to"]);
    await waitFor(() => expect(screen.queryByRole("menu")).not.toBeInTheDocument());
  });

  it("closes on Escape, chooses nothing and returns focus to the trigger", async () => {
    const user = userEvent.setup();
    const changes: string[] = [];
    render(<RadioMenu onChange={(v) => changes.push(v)} />);
    const trigger = screen.getByRole("button", { name: /Group by/ });
    await user.click(trigger);
    await screen.findAllByRole("menuitemradio");

    await user.keyboard("{Escape}");

    await waitFor(() => expect(screen.queryByRole("menu")).not.toBeInTheDocument());
    expect(changes).toEqual([]);
    expect(trigger).toHaveFocus();
  });

  it("exports MenuLabel as a heading inside a menu", async () => {
    const user = userEvent.setup();
    render(
      <Menu trigger={<Button>Open</Button>}>
        <MenuLabel>Section</MenuLabel>
        <MenuItem onSelect={() => undefined}>Item</MenuItem>
      </Menu>,
    );
    await user.click(screen.getByRole("button", { name: "Open" }));
    expect(await screen.findByText("Section")).toBeInTheDocument();
  });
});
