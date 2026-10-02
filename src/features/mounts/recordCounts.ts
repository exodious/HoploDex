import type { RecordKind } from "./types";

// Issue #56: the application doesn't call a firearm or an accessory a
// "record" on its own (spec.md FR-012's wording rule). A count says what it
// counts ("1 firearm and 2 accessories"), and a noun for a set of them names
// the kinds in it ("accessories", "firearms and accessories").

/** How many firearms and accessories a set holds. Mirrors `RecordCounts` in
 * `src-tauri/src/models/record.rs`. */
export type RecordCounts = { firearms: number; accessories: number };

export function countKinds(records: { kind: RecordKind }[]): RecordCounts {
  const firearms = records.filter((record) => record.kind === "firearm").length;
  return { firearms, accessories: records.length - firearms };
}

export function hasAny({ firearms, accessories }: RecordCounts): boolean {
  return firearms + accessories > 0;
}

function plural(n: number, one: string, many: string): string {
  return `${n} ${n === 1 ? one : many}`;
}

/** "1 firearm", "2 accessories", "1 firearm and 2 accessories"; "no firearms
 * or accessories" when both are 0. */
export function describeCounts({ firearms, accessories }: RecordCounts): string {
  const parts = [
    firearms > 0 && plural(firearms, "firearm", "firearms"),
    accessories > 0 && plural(accessories, "accessory", "accessories"),
  ].filter((part): part is string => Boolean(part));
  return parts.length > 0 ? parts.join(" and ") : "no firearms or accessories";
}

/** The noun for a set, without its count: "firearm" or "accessory" for one,
 * "firearms", "accessories" or "firearms and accessories" for more, or
 * always, with `plural`, for a statement about any of them. */
export function kindNoun({ firearms, accessories }: RecordCounts, plural = false): string {
  if (!plural && firearms + accessories === 1) return firearms === 1 ? "firearm" : "accessory";
  if (accessories === 0) return "firearms";
  if (firearms === 0) return "accessories";
  return "firearms and accessories";
}

/** A column heading over a set's names: "Firearm", "Accessory", or "Firearm
 * or accessory" when the set has both (or nothing yet). */
export function kindHeading({ firearms, accessories }: RecordCounts): string {
  if (accessories === 0 && firearms > 0) return "Firearm";
  if (firearms === 0 && accessories > 0) return "Accessory";
  return "Firearm or accessory";
}

/** {@link kindNoun} at the start of a sentence. */
export function capitalized(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}
