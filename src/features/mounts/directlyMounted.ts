import type { MountedEntry } from "./types";

/** The records mounted directly on a record (`depth` 1), as the delete
 * confirmation names them (specs/006-accessory-links FR-015, §8). */
export function directlyMounted(mounted: MountedEntry[]): MountedEntry[] {
  return mounted.filter((entry) => entry.depth <= 1);
}
