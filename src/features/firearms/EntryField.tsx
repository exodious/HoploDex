import { Combobox } from "../../components";
import type { ComboboxOption, ComboboxProps } from "../../components";
import { suggestEntries } from "./firearmsService";
import type { EntryFieldName, Suggestion } from "./types";

export interface EntryFieldProps extends Omit<
  ComboboxProps,
  "loadOptions" | "onInputChange" | "onPick" | "onBlurSettle"
> {
  field: EntryFieldName;
  /** Model only: the make on the form, whose models the list ranks first. */
  make?: string;
  onValueChange: (text: string) => void;
  onPick: (value: string) => void;
  /** The user left the field: the form settles it (contracts/ui-entry.md §2). */
  onLeave: () => void;
}

/** The row's marker (FR-016, contracts/ui-entry.md §1): "Built-in", with a
 * cartridge's caliber, and/or how many firearms use the value. */
function suggestionMarker(suggestion: Suggestion): string {
  const parts: string[] = [];
  if (suggestion.inCatalog) {
    parts.push(suggestion.caliber ? `Built-in · ${suggestion.caliber}` : "Built-in");
  }
  if (suggestion.useCount > 0) parts.push(`${suggestion.useCount} in collection`);
  return parts.join(" · ");
}

/** Make, Model, Cartridge or Caliber on the firearm form: the shared
 * `Combobox` fed by `suggest_entries` (specs/004-cartridges-action-types
 * US2). Snapping and the derived caliber are the form's, because they touch
 * other fields and Save. */
export function EntryField({
  field,
  make,
  onValueChange,
  onPick,
  onLeave,
  ...props
}: EntryFieldProps) {
  async function loadOptions(text: string): Promise<ComboboxOption[]> {
    const suggestions = await suggestEntries(field, text, field === "model" ? make : undefined);
    return suggestions.map((suggestion) => ({
      value: suggestion.value,
      marker: suggestionMarker(suggestion) || undefined,
    }));
  }
  return (
    <Combobox
      {...props}
      loadOptions={loadOptions}
      onInputChange={onValueChange}
      onPick={onPick}
      onBlurSettle={onLeave}
    />
  );
}
