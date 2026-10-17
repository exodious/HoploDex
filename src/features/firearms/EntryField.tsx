import { Combobox } from "../../components";
import type { ComboboxOption, ComboboxProps } from "../../components";
import { useSessionScope } from "../session/sessionScope";
import { suggestEntries } from "./firearmsService";
import type { EntryFieldName, Suggestion } from "./types";

export interface EntryFieldProps extends Omit<
  ComboboxProps,
  "loadOptions" | "onInputChange" | "onPick" | "onBlurSettle"
> {
  field: EntryFieldName;
  /** Model only: the make on the form, whose models the list is limited to. */
  make?: string;
  onValueChange: (text: string) => void;
  onPick: (value: string) => void;
  /** The user left the field: the form settles it (004 contracts/ui-entry.md §2). */
  onLeave: () => void;
}

/** The row's marker (004 FR-016, contracts/ui-entry.md §1): "Built-in", with a
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
 * `Combobox` fed by `suggest_entries` (004 US2). Snapping and the derived caliber
 * are the form's, because they touch other fields and Save. */
export function EntryField({
  field,
  make,
  onValueChange,
  onPick,
  onLeave,
  ...props
}: EntryFieldProps) {
  const scope = useSessionScope();
  async function loadOptions(text: string): Promise<ComboboxOption[]> {
    const suggestions = await suggestEntries(
      scope,
      field,
      text,
      field === "model" ? make : undefined,
    );
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
