// Mirrors src-tauri/src/models/firearm.rs's wire shapes (camelCase, per
// contracts/tauri-commands.md).

export type FirearmStatus = "active" | "disposed";

export type DispositionType = "sold" | "traded" | "gifted" | "destroyed" | "lost_stolen";

/** How worn a firearm is (FR-039), best first. Distinct from the free-form
 * notes. Mirrors `Condition` in src-tauri/src/models/firearm.rs. */
export type Condition = "new_in_box" | "like_new" | "excellent" | "good" | "fair" | "poor";

/** specs/002-firearm-identification FR-001: `null` means not specified.
 * Mirrors `Origin` in src-tauri/src/models/firearm.rs. */
export type Origin = "domestic" | "imported" | "reimported";

export interface Firearm {
  id: number;
  make: string;
  model: string;
  nickname: string | null;
  serialNumber: string | null;
  noSerialAttested: boolean;
  caliber: string;
  firearmTypeId: number;
  notes: string | null;
  accessories: string | null;
  /** FR-039: hundredths of an inch; format with `lib/measure`. */
  barrelLengthHundredths: number | null;
  /** FR-039: hundredths of an inch. */
  overallLengthHundredths: number | null;
  /** FR-039: tenths of an ounce. */
  weightTenthsOz: number | null;
  /** FR-039: rounds the magazine, cylinder or tube holds. */
  capacity: number | null;
  finish: string | null;
  condition: Condition | null;
  status: FirearmStatus;
  estimatedValue: number | null;
  acquisitionSource: string | null;
  acquisitionDate: string | null;
  acquisitionPrice: number | null;
  dispositionType: DispositionType | null;
  dispositionRecipient: string | null;
  dispositionDate: string | null;
  dispositionPrice: number | null;
  thumbnailPhotoId: number | null;
  insurancePolicyId: number | null;
  scheduledCoverageAmount: number | null;
  /** specs/002-firearm-identification FR-001: `null` means not specified. */
  origin: Origin | null;
  /** specs/002-firearm-identification FR-003. */
  yearOfManufacture: number | null;
  /** specs/002-firearm-identification FR-002: only for `origin === "imported"`. */
  countryOfManufacture: string | null;
  /** specs/002-firearm-identification FR-002: import-marked origins only. */
  importerName: string | null;
  /** specs/002-firearm-identification FR-004: import-marked origins only. */
  originalMake: string | null;
  originalModel: string | null;
  originalSerialNumber: string | null;
  createdAt: string;
  updatedAt: string;
}

/** A retained past disposition of a firearm restored to active (FR-033). */
export interface DispositionHistoryEntry {
  id: number;
  firearmId: number;
  dispositionType: DispositionType;
  dispositionRecipient: string;
  dispositionDate: string;
  dispositionPrice: number | null;
  reversedAt: string;
}

/** `get_firearm`'s output: the record plus its retained dispositions,
 * newest first. */
export type FirearmDetail = Firearm & { dispositionHistory: DispositionHistoryEntry[] };

/** What to do with the disposition being reversed (FR-033). */
export type HistoryChoice = "keep" | "discard";

export interface ReverseDispositionInput {
  history: HistoryChoice;
  /** Renames the firearm in the same step, to resolve a nickname clash. */
  nickname?: string;
  /** Resends after an `ORIGINAL_MARKS_MATCH` warning (FR-009). */
  confirmedWarnings?: boolean;
}

/** All `Firearm` fields except `id`, `createdAt`, `updatedAt`, `thumbnailPhotoId`. */
export type FirearmInput = Omit<Firearm, "id" | "createdAt" | "updatedAt" | "thumbnailPhotoId">;

export interface DisposeFirearmInput {
  dispositionType: DispositionType;
  recipient: string;
  date: string;
  price: number;
}

/** Seeded per src-tauri/src/db/migrations/0003_seed_firearm_types.sql. No
 * management UI exists for this list in this feature (data-model.md).
 * `key` is the type's `generic_thumbnail_key`. */
export const FIREARM_TYPE_OPTIONS = [
  { value: "1", label: "Handgun", key: "handgun" },
  { value: "2", label: "Rifle", key: "rifle" },
  { value: "3", label: "Shotgun", key: "shotgun" },
  { value: "4", label: "Other", key: "other" },
];

export function firearmTypeOption(firearmTypeId: number) {
  return (
    FIREARM_TYPE_OPTIONS.find((o) => o.value === String(firearmTypeId)) ?? {
      value: String(firearmTypeId),
      label: "Other",
      key: "other",
    }
  );
}

export const DISPOSITION_TYPE_OPTIONS: { value: DispositionType; label: string }[] = [
  { value: "sold", label: "Sold" },
  { value: "traded", label: "Traded" },
  { value: "gifted", label: "Gifted" },
  { value: "destroyed", label: "Destroyed" },
  { value: "lost_stolen", label: "Lost or stolen" },
];

export const CONDITION_OPTIONS: { value: Condition; label: string }[] = [
  { value: "new_in_box", label: "New in box" },
  { value: "like_new", label: "Like new" },
  { value: "excellent", label: "Excellent" },
  { value: "good", label: "Good" },
  { value: "fair", label: "Fair" },
  { value: "poor", label: "Poor" },
];

export function conditionLabel(condition: Condition): string {
  return CONDITION_OPTIONS.find((o) => o.value === condition)?.label ?? condition;
}

export function dispositionLabel(type: DispositionType | null): string {
  return DISPOSITION_TYPE_OPTIONS.find((o) => o.value === type)?.label ?? "Disposed";
}

/** specs/002-firearm-identification contracts/ui-identification.md §1. Kept
 * in step with the SQL `CASE` in 0002_fts5.sql and `Origin::label()` in
 * src-tauri/src/models/firearm.rs (research.md §6). */
export const ORIGIN_OPTIONS: { value: Origin | ""; label: string; description: string }[] = [
  { value: "domestic", label: "Domestic", description: "Made in the U.S." },
  { value: "imported", label: "Imported", description: "Made abroad and brought in" },
  {
    value: "reimported",
    label: "Re-imported",
    description: "Made in the U.S., exported, then brought back in",
  },
  { value: "", label: "Not specified", description: "Leave this if you're not sure." },
];

export function originLabel(origin: Origin | null): string {
  if (origin === null) return "Not specified";
  return ORIGIN_OPTIONS.find((o) => o.value === origin)?.label ?? origin;
}
