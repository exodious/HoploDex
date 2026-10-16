import type { MountedEntry } from "./types";

/** The records mounted directly on a record (`depth` 1), as the delete
 * confirmation names them (006 FR-015, 006 contracts/ui-accessories.md §8). */
export function directlyMounted(mounted: MountedEntry[]): MountedEntry[] {
  return mounted.filter((entry) => entry.depth <= 1);
}
