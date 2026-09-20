// Mirrors src-tauri/src/commands/firearms.rs's list_firearms wire shapes
// (camelCase, per contracts/tauri-commands.md), User Story 2.

import type { FirearmStatus } from "../firearms/types";

export type GroupBy = "type" | "caliber" | "make";

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
  firearmTypeName: string;
  status: FirearmStatus;
  thumbnailPhotoId: number | null;
  genericThumbnailKey: string;
  estimatedValue: number | null;
  insuranceWarning: InsuranceWarning;
  insurancePolicyId: number | null;
  scheduledCoverageAmount: number | null;
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

export const GROUP_BY_OPTIONS: { value: GroupBy; label: string }[] = [
  { value: "type", label: "Type" },
  { value: "caliber", label: "Caliber" },
  { value: "make", label: "Make" },
];

/** Browse controls that persist while the user moves between pages. */
export interface BrowseState {
  query: string;
  groupBy: GroupBy | undefined;
  includeDisposed: boolean;
  view: "list" | "tile";
}
