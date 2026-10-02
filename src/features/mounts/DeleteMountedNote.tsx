import { directlyMounted } from "./directlyMounted";
import { countKinds, describeCounts } from "./recordCounts";
import { recordKey } from "./recordKey";
import { recordNameWithType } from "./recordNames";
import type { MountedEntry } from "./types";
import "./mounts.css";

// specs/006-accessory-links FR-015, contracts/ui-accessories.md §8: what the
// delete confirmation says is left behind. Only the records mounted directly
// on the one being deleted are named: those further down stay where they are.

export interface DeleteMountedNoteProps {
  /** `MountDetail.mounted`: everything below the record, depth-first. */
  mounted: MountedEntry[];
}

/** The confirmation's children; the page renders it only when
 * {@link directlyMounted} is not empty, so an empty body is not left. */
export function DeleteMountedNote({ mounted }: DeleteMountedNoteProps) {
  const direct = directlyMounted(mounted);
  return (
    <>
      <p>
        {describeCounts(countKinds(direct.map((entry) => entry.label.record)))} mounted on it will
        stay in the collection, unmounted:
      </p>
      <ul className="hd-mounted-stay">
        {direct.map((entry) => (
          <li key={recordKey(entry.label.record)}>{recordNameWithType(entry.label)}</li>
        ))}
      </ul>
    </>
  );
}
