import { capitalized, countKinds, kindNoun } from "./recordCounts";
import { recordKey } from "./recordKey";
import { recordNameText } from "./recordNames";
import type { MountedEntry } from "./types";

/** "The kept accessory", "Kept firearms and accessories": what a statement
 * is about, named by its kinds (issue #56). */
function kept(entries: MountedEntry[]): string {
  const noun = kindNoun(countKinds(entries.map((entry) => entry.label.record)));
  return entries.length === 1 ? `The kept ${noun}` : `Kept ${noun}`;
}

/** What happens to the records kept, below the choices (FR-014): which are
 * unmounted, named by what they were mounted on, and which stay mounted.
 * `disposeWith` holds the records disposed with `subjectName`, by
 * `recordKey`. */
export function mountedStatements(
  subjectName: string,
  mounted: MountedEntry[],
  disposeWith: Record<string, unknown>,
): string[] {
  const keptEntries = mounted.filter((entry) => !(recordKey(entry.label.record) in disposeWith));
  const hostIsDisposed = (entry: MountedEntry) => recordKey(entry.host) in disposeWith;
  const statements: string[] = [];

  const onSubject = keptEntries.filter((entry) => entry.depth <= 1);
  if (onSubject.length > 0) {
    statements.push(`${kept(onSubject)} mounted on ${subjectName} will be unmounted.`);
  }

  // Grouped by the disposed record they are on, in the list's order.
  const onDisposed = new Map<string, MountedEntry[]>();
  for (const entry of keptEntries) {
    if (entry.depth > 1 && hostIsDisposed(entry)) {
      const key = recordKey(entry.host);
      onDisposed.set(key, [...(onDisposed.get(key) ?? []), entry]);
    }
  }
  const labels = new Map(mounted.map((entry) => [recordKey(entry.label.record), entry.label]));
  for (const [key, entries] of onDisposed) {
    const host = labels.get(key);
    const hostName = host ? recordNameText(host) : "what is disposed with it";
    statements.push(`${kept(entries)} mounted on ${hostName} will be unmounted.`);
  }

  const staying = keptEntries.filter((entry) => entry.depth > 1 && !hostIsDisposed(entry));
  if (staying.length > 0) {
    const noun = kindNoun(countKinds(staying.map((entry) => entry.label.record)));
    statements.push(
      staying.length === 1
        ? `The ${noun} kept with what it is mounted on stays mounted.`
        : `${capitalized(noun)} kept with what they are mounted on stay mounted.`,
    );
  }
  return statements;
}
