import type { ReactElement, ReactNode } from "react";
import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import { Icon } from "./Icon";
import type { IconName } from "./Icon";
import "./components.css";

export interface MenuProps {
  /** The button that opens the menu. It must forward its ref (the shared
   * `Button` does). */
  trigger: ReactElement;
  /** `MenuItem`s and `MenuSeparator`s. */
  children: ReactNode;
  align?: "start" | "end";
}

/** A menu of actions opened from a button, with menu roles, arrow-key
 * navigation, and Escape returning focus to the button (WCAG 2.1 AA,
 * specs/003 contracts/ui-databases.md §0). */
export function Menu({ trigger, children, align = "start" }: MenuProps) {
  return (
    <DropdownMenu.Root modal={false}>
      <DropdownMenu.Trigger asChild>{trigger}</DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content className="hd-menu" align={align} sideOffset={6} loop>
          {children}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

export interface MenuItemProps {
  onSelect: () => void;
  icon?: IconName;
  /** A keyboard shortcut shown at the end, such as "Ctrl+L". */
  shortcut?: string;
  /** A second line under the label. */
  note?: string;
  disabled?: boolean;
  children: ReactNode;
}

export function MenuItem({ onSelect, icon, shortcut, note, disabled, children }: MenuItemProps) {
  return (
    <DropdownMenu.Item className="hd-menu__item" onSelect={onSelect} disabled={disabled}>
      {icon ? <Icon name={icon} size={16} className="hd-menu__icon" /> : null}
      <span className="hd-menu__label">
        {children}
        {note && <span className="hd-menu__note">{note}</span>}
      </span>
      {shortcut && (
        <kbd className="hd-menu__shortcut" aria-hidden>
          {shortcut}
        </kbd>
      )}
    </DropdownMenu.Item>
  );
}

export function MenuSeparator() {
  return <DropdownMenu.Separator className="hd-menu__separator" />;
}
