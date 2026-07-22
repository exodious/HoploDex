import { TextField } from "../../components";

export interface SearchBarProps {
  value: string;
  onChange: (value: string) => void;
}

/** Free-text search across all recorded firearm information, including
 * free-form notes (FR-013, US2 Acceptance Scenarios 3-4). */
export function SearchBar({ value, onChange }: SearchBarProps) {
  return (
    <TextField
      label="Search"
      placeholder="Search make, model, caliber, notes…"
      value={value}
      onChange={(e) => onChange(e.target.value)}
    />
  );
}
