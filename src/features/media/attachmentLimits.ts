import { CommandFailure } from "../../services/tauriClient";
import type { RecordRef } from "../mounts/types";

/**
 * The most photos, and the most documents, one firearm or accessory may have,
 * mirroring `MAX_PHOTOS_PER_RECORD` and `MAX_DOCUMENTS_PER_RECORD` in
 * `src-tauri/src/models/rules.rs` (001 FR-007 and FR-010, amended
 * 2026-10-11). The backend stays the authority: it refuses one more with
 * `ATTACHMENT_LIMIT_REACHED`.
 */
export const MAX_PHOTOS_PER_RECORD = 50;
export const MAX_DOCUMENTS_PER_RECORD = 50;

/** Whether `e` is the backend refusing a file because the record is full. */
export function isLimitReached(e: unknown): e is CommandFailure {
  return e instanceof CommandFailure && e.code === "ATTACHMENT_LIMIT_REACHED";
}

/** What the panel says in place of its drop strip once the record is full. */
export function fullNote(owner: RecordRef, limit: number, plural: string): string {
  return `This ${owner.kind} has ${limit} ${plural}, the most it can have. Delete one to add another.`;
}

/** What a batch says about the files a full record refused: which weren't
 * added (the name of one, or how many), then the backend's message. */
export function limitRefusal(
  names: string[],
  plural: string,
  verb: string,
  failure: CommandFailure,
): string {
  const which = names.length === 1 ? `${names[0]} wasn't` : `${names.length} ${plural} weren't`;
  return `${which} ${verb}. ${failure.message}`;
}
