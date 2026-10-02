import type { RecordRef } from "./types";

/** A stable key for a record of either kind. */
export function recordKey(record: RecordRef): string {
  return `${record.kind}:${record.id}`;
}
