import { recordKey } from "./recordKey";
import type { MountedEntry } from "./types";

/** What happens to the records kept, below the choices (FR-014): which are
 * unmounted and which stay mounted. `disposeWith` holds the records disposed
 * with `subjectName`, by `recordKey`. */
export function mountedStatements(
  subjectName: string,
  mounted: MountedEntry[],
  disposeWith: Record<string, unknown>,
): string[] {
  const kept = mounted.filter((entry) => !(recordKey(entry.label.record) in disposeWith));
  const hostIsDisposed = (entry: MountedEntry) => recordKey(entry.host) in disposeWith;
  return [
    kept.some((entry) => entry.depth <= 1) &&
      `Kept records mounted on ${subjectName} will be unmounted.`,
    kept.some((entry) => entry.depth > 1 && hostIsDisposed(entry)) &&
      "Kept records mounted on a record disposed with it will be unmounted.",
    kept.some((entry) => entry.depth > 1 && !hostIsDisposed(entry)) &&
      "Records kept with what they are mounted on stay mounted.",
  ].filter((text): text is string => Boolean(text));
}
