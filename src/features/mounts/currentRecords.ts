import { useContext } from "react";
import { CollectionContext } from "../app/collectionStore";
import { accessoryRecord, firearmRecord } from "../insurance/records";
import type { RecordLabel, RecordRef } from "./types";

/** A record as the collection holds it now, and where it is mounted. */
export type CurrentRecord = { label: RecordLabel; mountedOn: RecordLabel | null };

/** Records as they are now, for a form naming records a resumed draft kept
 * (014 FR-020): a draft says only what its database did when it was kept.
 * `find` gives `undefined` until the collection has loaded without an error
 * (or outside one), and `null` for a record it doesn't have. */
export function useCurrentRecords(): {
  loaded: boolean;
  find: (record: RecordRef) => CurrentRecord | null | undefined;
} {
  const collection = useContext(CollectionContext);
  const loaded = !!collection?.loaded && !collection.error;
  return {
    loaded,
    find(record) {
      if (!collection || !loaded) return undefined;
      if (record.kind === "firearm") {
        const firearm = collection.firearmsById.get(record.id);
        return firearm
          ? { label: firearmRecord(firearm).label, mountedOn: firearm.mountedOn }
          : null;
      }
      const accessory = collection.accessoriesById.get(record.id);
      return accessory
        ? { label: accessoryRecord(accessory).label, mountedOn: accessory.mountedOn }
        : null;
    },
  };
}

/** Whether the collection has `record`, active (`undefined` until it has
 * loaded). */
export function isActiveNow(
  current: ReturnType<typeof useCurrentRecords>,
  record: RecordRef,
): boolean | undefined {
  const found = current.find(record);
  return found === undefined ? undefined : found?.label.status === "active";
}
