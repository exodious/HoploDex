import { RecordName } from "../mounts/RecordName";
import type { RecordLabel } from "../mounts/types";

// specs/006-accessory-links FR-016a, contracts/ui-accessories.md §9: a mounted
// record names its host under its name (the host a link); a firearm with
// records mounted on it counts everything below at any depth ("{n} mounted",
// not linked, not naming them). Both, "Mounted on …" first, when both. Neither
// renders nothing.

export interface MountLinesProps {
  mountedOn: RecordLabel | null;
  /** Firearms only: everything mounted below, at any depth. */
  mountedCount?: number;
}

export function MountLines({ mountedOn, mountedCount = 0 }: MountLinesProps) {
  return (
    <>
      {mountedOn && (
        <p className="hd-mountline">
          Mounted on <RecordName label={mountedOn} link />
        </p>
      )}
      {mountedCount > 0 && <p className="hd-mountline">{mountedCount} mounted</p>}
    </>
  );
}
