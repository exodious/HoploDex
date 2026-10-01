// Mirrors src-tauri/src/models/firearm.rs's wire shapes (camelCase, per
// contracts/tauri-commands.md).

// specs/006-accessory-links: a firearm or accessory, as mounts name it.
import type { MountDetail, RecordRef } from "../mounts/types";
export type { RecordRef };

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
  /** specs/004-cartridges-action-types FR-001: the exact round; `null` = none. */
  cartridge: string | null;
  firearmTypeId: number;
  /** specs/004-cartridges-action-types FR-017: an id from `list_action_types`;
   * `null` = not specified. */
  actionTypeId: number | null;
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
  /** specs/005-regulated-item-types FR-007: an id from
   * `list_registration_classes`; `null` = no classification. */
  registrationClassId: number | null;
  /** FR-009: e.g. "Form 4". Only with a classification. */
  registrationForm: string | null;
  /** FR-009, FR-010: `YYYY-MM-DD`. Only with a classification. */
  registrationApproved: string | null;
  /** FR-009: e.g. "Smith Family Trust". Only with a classification. */
  registeredTo: string | null;
  /** specs/006-accessory-links FR-010: the direct host, if mounted. The
   * forms always send it; `null` = not mounted. */
  mountedOn: RecordRef | null;
  createdAt: string;
  updatedAt: string;
}

/** A retained past disposition of a firearm or accessory restored to active
 * (FR-033; 006 FR-006). */
export interface DispositionHistoryEntry {
  id: number;
  /** The record it belonged to (006 data-model.md). */
  owner: RecordRef;
  dispositionType: DispositionType;
  dispositionRecipient: string;
  dispositionDate: string;
  dispositionPrice: number | null;
  reversedAt: string;
}

/** `get_firearm`'s output: the record plus its retained dispositions,
 * newest first. */
export type FirearmDetail = Firearm & {
  dispositionHistory: DispositionHistoryEntry[];
  /** specs/006-accessory-links FR-013: the chain above and everything below. */
  mount: MountDetail;
};

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

/** specs/004-cartridges-action-types FR-017/FR-018. Mirrors `ActionType` in
 * src-tauri/src/models/action_type.rs. */
export interface ActionType {
  id: number;
  name: string;
}

/** `list_action_types`' output: the actions in list order, and per firearm
 * type id the allowed action ids. A type that is absent, or maps to `[]`,
 * allows every action (FR-017). */
export interface ActionTypesOutput {
  actions: ActionType[];
  allowedByFirearmType: Record<number, number[]>;
}

/** specs/004-cartridges-action-types FR-009 and specs/005-regulated-item-types
 * research.md §7: the six fields with suggestions and snapping. Mirrors
 * `EntryField::ipc_name` in src-tauri/src/services/entry_text.rs. */
export type EntryFieldName =
  "make" | "model" | "cartridge" | "caliber" | "registrationForm" | "registeredTo";

/** The caliber a cartridge derives, and whether it was read from the
 * catalog or guessed (FR-005). Mirrors `DerivedCaliber` in
 * src-tauri/src/services/cartridges/mod.rs. */
export interface DerivedCaliber {
  caliber: string;
  source: "catalog" | "guess";
}

/** One row of `suggest_entries`' list (contracts/tauri-commands.md). Mirrors
 * `Suggestion` in src-tauri/src/services/suggestions.rs. */
export interface Suggestion {
  /** The display spelling: the catalog's when it has one. */
  value: string;
  /** Built in (FR-016). */
  inCatalog: boolean;
  /** Firearms on record, active and disposed; 0 = catalog only. */
  useCount: number;
  /** Catalog cartridges only: the bore class, as a hint. */
  caliber: string | null;
}

/** `settle_entry`'s output (contracts/tauri-commands.md). */
export interface SettleEntryOutput {
  /** Trimmed; the snapped spelling if any. */
  value: string;
  /** `null`: kept as typed, apart from trimming. */
  changedBy: "catalog" | "record" | null;
  /** Cartridge only; `null` for a cartridge means no caliber could be read. */
  derivedCaliber: DerivedCaliber | null;
}

export interface DisposeFirearmInput {
  dispositionType: DispositionType;
  recipient: string;
  date: string;
  price: number;
}

/** specs/005-regulated-item-types FR-001/FR-003. Mirrors `FirearmTypeInfo` in
 * src-tauri/src/models/firearm_type.rs. A flag of `false` means the field
 * doesn't apply to the type: the form doesn't offer it and no firearm of the
 * type may hold a value. */
export interface FirearmTypeInfo {
  id: number;
  name: string;
  /** The type's drawing, a key of `DRAWINGS`. */
  genericThumbnailKey: string;
  actionTypeApplies: boolean;
  barrelLengthApplies: boolean;
  capacityApplies: boolean;
  /** `false`: the caliber is never worked out from the cartridge. A
   * Suppressor's caliber is its bore (FR-002; research.md §15). */
  caliberFromCartridge: boolean;
}

/** `list_firearm_types`' output: every type, in list order (Other last). */
export interface FirearmTypesOutput {
  types: FirearmTypeInfo[];
}

/** specs/005-regulated-item-types FR-007. Mirrors `RegistrationClass` in
 * src-tauri/src/models/registration.rs. */
export interface RegistrationClass {
  id: number;
  name: string;
  /** `false`: no longer offered for new choices; a record holding it keeps it. */
  offered: boolean;
}

/** `list_registration_classes`' output: every classification, in list order. */
export interface RegistrationClassesOutput {
  classes: RegistrationClass[];
}

/** A type as the form and the record page use it: its id as a select value,
 * its name, its drawing key and which fields apply. */
export interface FirearmTypeOption {
  value: string;
  label: string;
  key: string;
  actionTypeApplies: boolean;
  barrelLengthApplies: boolean;
  capacityApplies: boolean;
  caliberFromCartridge: boolean;
}

/** The type for an id, from the store's list (`useFirearmTypes`). An id the
 * list doesn't hold, or a list that hasn't loaded, reads as "Other" with its
 * drawing, and every field applies. */
export function firearmTypeOption(
  types: readonly FirearmTypeInfo[],
  firearmTypeId: number,
): FirearmTypeOption {
  const type = types.find((t) => t.id === firearmTypeId);
  return type
    ? {
        value: String(type.id),
        label: type.name,
        key: type.genericThumbnailKey,
        actionTypeApplies: type.actionTypeApplies,
        barrelLengthApplies: type.barrelLengthApplies,
        capacityApplies: type.capacityApplies,
        caliberFromCartridge: type.caliberFromCartridge,
      }
    : {
        value: String(firearmTypeId),
        label: "Other",
        key: "other",
        actionTypeApplies: true,
        barrelLengthApplies: true,
        capacityApplies: true,
        caliberFromCartridge: true,
      };
}

/** FR-002: a suppressor's cartridge is the most powerful one it is rated
 * for, and its caliber keeps its label as its bore. A display choice for the
 * one seeded type (research.md §15); whether the caliber is derived is the
 * type's `caliberFromCartridge`. */
export function cartridgeLabel(typeName: string): string {
  return typeName === "Suppressor" ? "Rated cartridge" : "Cartridge";
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
  { value: "", label: "Unspecified", description: "Leave this if you're not sure." },
];

/** specs/004-cartridges-action-types research.md §11: the one term for a
 * value that was not recorded, on every screen. */
export const UNSPECIFIED = "Unspecified";

export function originLabel(origin: Origin | null): string {
  if (origin === null) return UNSPECIFIED;
  return ORIGIN_OPTIONS.find((o) => o.value === origin)?.label ?? origin;
}
