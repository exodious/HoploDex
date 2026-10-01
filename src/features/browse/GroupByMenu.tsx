import { Fragment } from "react";
import { Button, Icon, Menu, MenuRadioGroup, MenuRadioItem, MenuSeparator } from "../../components";

export interface GroupByOption<T extends string> {
  value: T;
  label: string;
  /** The heading it sits under, when the menu has headed sections. */
  section?: string;
}

export interface GroupByMenuProps<T extends string> {
  value: T | undefined;
  onChange: (value: T | undefined) => void;
  options: GroupByOption<T>[];
  /** Fixed headings, in order, each holding the options that name it. Leave
   * out for one group with no heading. */
  sections?: { section: string; heading: string }[];
  /** The "no grouping" item's label. The button reads "None" either way. */
  noneLabel?: string;
}

/** The "Group by" control of the collection and Accessories pages: a button
 * naming the current grouping that opens a menu of radio items
 * (specs/005-regulated-item-types contracts/ui-registration.md §6;
 * specs/006-accessory-links contracts/ui-accessories.md §2). */
export function GroupByMenu<T extends string>({
  value,
  onChange,
  options,
  sections,
  noneLabel = "None",
}: GroupByMenuProps<T>) {
  const chosen: string = value ?? "none";
  const label = options.find((option) => option.value === value)?.label ?? "None";
  const choose = (next: string) => onChange(next === "none" ? undefined : (next as T));
  const items = (list: GroupByOption<T>[]) =>
    list.map((option) => (
      <MenuRadioItem key={option.value} value={option.value}>
        {option.label}
      </MenuRadioItem>
    ));
  return (
    <Menu
      trigger={
        <Button size="sm" className="hd-groupby" aria-label={`Group by, ${label}`}>
          <span className="hd-groupby__prompt">Group by</span>
          <span className="hd-groupby__value">{label}</span>
          <Icon name="chevronDown" size={16} />
        </Button>
      }
    >
      <MenuRadioGroup value={chosen} onValueChange={choose}>
        <MenuRadioItem value="none">{noneLabel}</MenuRadioItem>
      </MenuRadioGroup>
      {sections ? (
        sections.map(({ section, heading }) => (
          <Fragment key={section}>
            <MenuSeparator />
            <MenuRadioGroup value={chosen} onValueChange={choose} label={heading}>
              {items(options.filter((option) => option.section === section))}
            </MenuRadioGroup>
          </Fragment>
        ))
      ) : (
        <>
          <MenuSeparator />
          <MenuRadioGroup value={chosen} onValueChange={choose}>
            {items(options)}
          </MenuRadioGroup>
        </>
      )}
    </Menu>
  );
}
