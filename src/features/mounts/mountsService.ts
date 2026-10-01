// Typed wrappers over `invoke` for the mount commands: contracts/tauri-commands.md
// "Mounts (new)" (`mount_record`, `list_mount_candidates`); the screens that use
// them are in contracts/ui-accessories.md. The collection is refreshed by the
// caller after a mount (`useCollection().refresh()`), as for every mutation.

import { invoke } from "../../services/tauriClient";
import type { RecordLabel, RecordRef } from "./types";

export type MountCandidate = { label: RecordLabel; mountedOn: RecordLabel | null };

export type ListMountCandidatesInput = {
  /** "host": candidates for `record` to be mounted on; "item": candidates to
   * mount on `record` (required). */
  role: "host" | "item";
  record: RecordRef | null;
  /** Matched in make, model, nickname, serial number; "" = the first 50. */
  query: string;
  /** Default 50, at most 100. */
  limit?: number;
};

export type MountRecordOutput = { item: RecordLabel; host: RecordLabel | null };

/** Moves `item` onto `host`, or unmounts it with `null` (FR-012). A rejected
 * host fails with `fieldErrors.host`. */
export function mountRecord(item: RecordRef, host: RecordRef | null): Promise<MountRecordOutput> {
  return invoke<MountRecordOutput>("mount_record", { input: { item, host } });
}

export function listMountCandidates(
  input: ListMountCandidatesInput,
): Promise<{ candidates: MountCandidate[] }> {
  return invoke<{ candidates: MountCandidate[] }>("list_mount_candidates", { input });
}
