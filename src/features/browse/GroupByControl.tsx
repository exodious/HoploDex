import { Select } from "../../components";
import { GROUP_BY_OPTIONS } from "./types";
import type { GroupBy } from "./types";

export interface GroupByControlProps {
  value: GroupBy | undefined;
  onChange: (value: GroupBy | undefined) => void;
}

const NONE_VALUE = "__none__";

/** Groups the collection by a structured field (type/caliber/make), per
 * FR-012 and US2 Acceptance Scenario 2. */
export function GroupByControl({ value, onChange }: GroupByControlProps) {
  return (
    <Select
      label="Group by"
      value={value ?? NONE_VALUE}
      onValueChange={(next) => onChange(next === NONE_VALUE ? undefined : (next as GroupBy))}
      options={[{ value: NONE_VALUE, label: "None" }, ...GROUP_BY_OPTIONS]}
    />
  );
}
