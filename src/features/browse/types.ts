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
  caliber: string;
  firearmTypeName: string;
  status: FirearmStatus;
  thumbnailPhotoId: number | null;
  genericThumbnailKey: string;
  estimatedValue: number | null;
  insuranceWarning: InsuranceWarning;
}

export interface FirearmGroup {
  key: string;
  firearms: FirearmSummary[];
}

export interface ListFirearmsOutput {
  groups: FirearmGroup[];
}

export const GROUP_BY_OPTIONS: { value: GroupBy; label: string }[] = [
  { value: "type", label: "Type" },
  { value: "caliber", label: "Caliber" },
  { value: "make", label: "Make" },
];
