// Mirrors src-tauri/src/commands/firearms.rs's list_firearms wire shapes
// (camelCase, per contracts/tauri-commands.md), User Story 2.

import type { FirearmStatus } from "../firearms/types";
import type { RecordLabel } from "../mounts/types";

export type GroupBy =
  | "type"
  | "action_type"
  | "caliber"
  | "cartridge"
  | "make"
  | "origin"
  | "registered_as"
  | "registered_to";

export type InsuranceWarning = "none" | "uninsured" | "under_insured";

export interface ListFirearmsInput {
  query?: string;
  groupBy?: GroupBy;
  includeDisposed?: boolean;
  view?: "list" | "tile";
}

export interface FirearmSummary {
  id: number;
  make: string;
  model: string;
  nickname: string | null;
  serialNumber: string | null;
  caliber: string;
  /** specs/004-cartridges-action-types FR-001: `null` = none recorded. */
  cartridge: string | null;
  firearmTypeName: string;
  /** specs/004-cartridges-action-types FR-020: the action's name; `null` =
   * none recorded. */
  actionTypeName: string | null;
  /** specs/005-regulated-item-types FR-016: the classification's name;
   * `null` = none. */
  registeredAs: string | null;
  status: FirearmStatus;
  thumbnailPhotoId: number | null;
  genericThumbnailKey: string;
  estimatedValue: number | null;
  insuranceWarning: InsuranceWarning;
  insurancePolicyId: number | null;
  scheduledCoverageAmount: number | null;
  /** specs/006-accessory-links FR-013: the direct host only. */
  mountedOn: RecordLabel | null;
  /** Everything mounted below, at any depth (contracts/ui-accessories.md §9). */
  mountedCount: number;
}

export interface FirearmGroup {
  key: string;
  firearms: FirearmSummary[];
}

/** A group as rendered: possibly only its first rows (progressive
 * rendering), with `total` still counting all of them. */
export interface VisibleGroup extends FirearmGroup {
  total: number;
}

export interface ListFirearmsOutput {
  groups: FirearmGroup[];
}

/** Which heading the grouping menu puts an option under (specs/005
 * contracts/ui-registration.md §6). */
export type GroupBySection = "firearm" | "maker" | "registration";

export const GROUP_BY_OPTIONS: { value: GroupBy; label: string; section: GroupBySection }[] = [
  { value: "type", label: "Type", section: "firearm" },
  { value: "action_type", label: "Action", section: "firearm" },
  { value: "caliber", label: "Caliber", section: "firearm" },
  { value: "cartridge", label: "Cartridge", section: "firearm" },
  { value: "make", label: "Make", section: "maker" },
  { value: "origin", label: "Origin", section: "maker" },
  { value: "registered_as", label: "Registered as", section: "registration" },
  { value: "registered_to", label: "Registered to", section: "registration" },
];

/** The fixed headings of the grouping menu, in order. */
export const GROUP_BY_SECTIONS: { section: GroupBySection; heading: string }[] = [
  { section: "firearm", heading: "The firearm" },
  { section: "maker", heading: "Its maker" },
  { section: "registration", heading: "Registration" },
];

/** specs/004-cartridges-action-types FR-027: where browsing shows the
 * caliber, "9x19mm Parabellum (9mm)" with a cartridge, else the caliber. */
export function caliberText(firearm: { cartridge: string | null; caliber: string }): string {
  return firearm.cartridge ? `${firearm.cartridge} (${firearm.caliber})` : firearm.caliber;
}

/** Browse controls that persist while the user moves between pages. */
export interface BrowseState {
  query: string;
  groupBy: GroupBy | undefined;
  includeDisposed: boolean;
  view: "list" | "tile";
}
