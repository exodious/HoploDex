import { describeCounts, hasAny, type RecordCounts } from "../mounts/recordCounts";
import { RecordName } from "../mounts/RecordName";
import type { RecordLabel } from "../mounts/types";

// specs/006-accessory-links FR-016, FR-016a, contracts/ui-accessories.md §8,
// §9: a mounted record names its host under its name (the host a link); a
// firearm or accessory with records mounted on it counts everything below at
// any depth, by kind ("1 firearm and 2 accessories mounted", issue #56), not
// linked, not naming them. Both, "Mounted on …" first, when both. Neither
// renders nothing.

export interface MountLinesProps {
  mountedOn: RecordLabel | null;
  /** Everything mounted below, at any depth. */
  mountedCounts: RecordCounts;
}

export function MountLines({ mountedOn, mountedCounts }: MountLinesProps) {
  return (
    <>
      {mountedOn && (
        <p className="hd-mountline">
          Mounted on <RecordName label={mountedOn} link />
        </p>
      )}
      {hasAny(mountedCounts) && (
        <p className="hd-mountline">{describeCounts(mountedCounts)} mounted</p>
      )}
    </>
  );
}
