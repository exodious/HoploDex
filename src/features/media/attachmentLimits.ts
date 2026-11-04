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

const MIB = 1024 * 1024;

/**
 * The largest photo and the largest document that may be added, and the most one record's
 * photos and documents may hold together, in bytes, mirroring `MAX_PHOTO_BYTES`,
 * `MAX_DOCUMENT_BYTES` and `MAX_ATTACHMENT_BYTES_PER_RECORD` in
 * `src-tauri/src/models/rules.rs` (001 FR-007 and FR-010, amended 2026-11-04). The backend
 * refuses each with `ATTACHMENT_TOO_LARGE`.
 */
export const MAX_PHOTO_BYTES = 15 * MIB;
export const MAX_DOCUMENT_BYTES = 30 * MIB;
export const MAX_ATTACHMENT_BYTES_PER_RECORD = 1024 * MIB;

/** Whether `e` is the backend refusing a file for its size, alone or with the record's others. */
export function isTooLarge(e: unknown): e is CommandFailure {
  return e instanceof CommandFailure && e.code === "ATTACHMENT_TOO_LARGE";
}

/** The backend's refusal of a photo or document over `limit`, for a file the chooser gave,
 * refused here from its size so that none of it is read into the page. */
export function tooLarge(what: "photo" | "document", limit: number): CommandFailure {
  return new CommandFailure({
    code: "ATTACHMENT_TOO_LARGE",
    message: `A ${what} can be at most ${limit / MIB} MiB.`,
    details: { limit },
  });
}

/** The files of a batch refused for their size, grouped by why: a batch carries on past
 * them, since a smaller file may still fit, and then says for each reason which weren't
 * added. */
export function sizeRefusals() {
  const groups = new Map<string, { names: string[]; failure: CommandFailure }>();
  return {
    add(name: string, failure: CommandFailure) {
      const group = groups.get(failure.message) ?? { names: [], failure };
      group.names.push(name);
      groups.set(failure.message, group);
    },
    messages(plural: string, verb: string): string[] {
      return [...groups.values()].map((g) => limitRefusal(g.names, plural, verb, g.failure));
    },
  };
}

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
