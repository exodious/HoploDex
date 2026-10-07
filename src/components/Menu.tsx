import { useId } from "react";
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
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>{trigger}</DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content
          className="hd-menu"
          align={align}
          sideOffset={6}
          loop
          ref={focusCheckedItem}
        >
          {children}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

/** A menu of radio items opens with focus on the checked one, where Radix
 * would start at the first item (contracts/ui-registration.md §6). Radix
 * places focus in its own mount effects, so this waits a frame. */
function focusCheckedItem(content: HTMLElement | null) {
  if (!content) return;
  requestAnimationFrame(() => {
    content.querySelector<HTMLElement>('[role="menuitemradio"][aria-checked="true"]')?.focus();
  });
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

export interface MenuRadioGroupProps {
  /** The chosen value. A menu with several groups gives each the same value,
   * so exactly one item across them is checked. */
  value: string;
  onValueChange: (value: string) => void;
  /** The group's heading, which also names the group. Leave it out for a
   * group with no heading. */
  label?: string;
  children: ReactNode;
}

/** A group of `MenuRadioItem`s: `role="group"`, named by its `label`
 * (specs/005-regulated-item-types contracts/ui-registration.md §6). */
export function MenuRadioGroup({ value, onValueChange, label, children }: MenuRadioGroupProps) {
  const labelId = useId();
  return (
    <DropdownMenu.RadioGroup
      value={value}
      onValueChange={onValueChange}
      aria-labelledby={label ? labelId : undefined}
    >
      {label ? <MenuLabel id={labelId}>{label}</MenuLabel> : null}
      {children}
    </DropdownMenu.RadioGroup>
  );
}

export interface MenuRadioItemProps {
  value: string;
  children: ReactNode;
}

/** One choice of a `MenuRadioGroup`. A ring marks every item in the leading
 * gutter and a filled dot the checked one, so labels stay aligned. */
export function MenuRadioItem({ value, children }: MenuRadioItemProps) {
  return (
    <DropdownMenu.RadioItem className="hd-menu__item hd-menu__item--radio" value={value}>
      <span className="hd-menu__radio" aria-hidden>
        <DropdownMenu.ItemIndicator className="hd-menu__radio-dot" />
      </span>
      <span className="hd-menu__label">{children}</span>
    </DropdownMenu.RadioItem>
  );
}

/** A heading inside a menu, such as a section of radio items. */
export function MenuLabel({ id, children }: { id?: string; children: ReactNode }) {
  return (
    <DropdownMenu.Label id={id} className="hd-menu__heading">
      {children}
    </DropdownMenu.Label>
  );
}
