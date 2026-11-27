// 014 FR-024, FR-025, FR-027: an edit form's Mounted list as the form holds it,
// compares it and saves it. The list itself is MountedEditor.tsx.
import { recordKey } from "./recordKey";
import type { MountChange } from "./mountsService";
import type { RecordLabel, RecordRef } from "./types";

/** A record the form will mount here, and where it is mounted now. */
export type MountAdd = { label: RecordLabel; from: RecordLabel | null };

/** The list's changes as the form holds them (and a kept draft keeps them). */
export interface MountedChanges {
  adds: MountAdd[];
  /** The saved entries to unmount, by `recordKey`. */
  removes: string[];
}

export const NO_MOUNTED_CHANGES: MountedChanges = { adds: [], removes: [] };

/** A kept draft's list, read as {@link MountedChanges} or dropped: a draft
 * comes from the database and may be of another shape. */
export function readMountedChanges(value: unknown): MountedChanges {
  if (typeof value !== "object" || value === null) return NO_MOUNTED_CHANGES;
  const { adds, removes } = value as Partial<Record<keyof MountedChanges, unknown>>;
  const isRef = (ref: unknown) =>
    typeof ref === "object" &&
    ref !== null &&
    ((ref as RecordRef).kind === "firearm" || (ref as RecordRef).kind === "accessory") &&
    Number.isInteger((ref as RecordRef).id);
  // Every field the list renders, so a malformed label can't break the form.
  const isLabel = (label: unknown) => {
    if (typeof label !== "object" || label === null) return false;
    const { record, make, model, nickname, typeName, serialNumber } = label as RecordLabel;
    const textOrNull = (text: unknown) => text === null || typeof text === "string";
    return (
      isRef(record) &&
      typeof make === "string" &&
      typeof model === "string" &&
      typeof typeName === "string" &&
      textOrNull(nickname) &&
      textOrNull(serialNumber)
    );
  };
  const okAdds =
    Array.isArray(adds) &&
    adds.every(
      (add: unknown) =>
        typeof add === "object" &&
        add !== null &&
        isLabel((add as MountAdd).label) &&
        ((add as MountAdd).from === null || isLabel((add as MountAdd).from)),
    );
  const okRemoves =
    Array.isArray(removes) && removes.every((key: unknown) => typeof key === "string");
  return okAdds && okRemoves
    ? { adds: adds as MountAdd[], removes: removes as string[] }
    : NO_MOUNTED_CHANGES;
}

/** What a save sends for the list. */
export function mountChangesInput(changes: MountedChanges, saved: RecordLabel[]): MountChange[] {
  const removed = new Set(changes.removes);
  return [
    ...saved
      .filter((label) => removed.has(recordKey(label.record)))
      .map((label) => ({ item: label.record, mounted: false })),
    ...changes.adds.map((add) => ({ item: add.label.record, mounted: true })),
  ];
}

/** The list as a save would store it, for the form's comparison (014 FR-025). */
export function storedMounted(changes: MountedChanges): string {
  const adds = changes.adds.map((add) => recordKey(add.label.record)).sort();
  return JSON.stringify([adds, [...changes.removes].sort()]);
}
