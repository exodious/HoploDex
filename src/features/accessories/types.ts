// Mirrors "Accessories (new)" in specs/006-accessory-links/contracts/tauri-commands.md
// (camelCase over IPC); screens: contracts/ui-accessories.md.

import type { DispositionHistoryEntry, DispositionType } from "../firearms/types";
import type { MountDetail, RecordLabel, RecordRef } from "../mounts/types";

export type AccessoryKind = {
  id: number;
  name: string;
  genericThumbnailKey: string;
  sortOrder: number;
  /** false: still shown on records that hold it, not offered for new choices. */
  offered: boolean;
};

export type AccessoryInput = {
  /** Required; any kind that exists. */
  accessoryKindId: number;
  /** 004 entry rules when set. */
  make: string | null;
  model: string | null;
  /** Free text; no uniqueness (FR-004). */
  serialNumber: string | null;
  /** 004 entry rules; derived from the cartridge on the form. */
  caliber: string | null;
  cartridge: string | null;
  notes: string | null;
  status: "active" | "disposed";
  /** Whole dollars. */
  estimatedValue: number | null;
  acquisitionSource: string | null;
  /** YYYY-MM-DD, not in the future. */
  acquisitionDate: string | null;
  acquisitionPrice: number | null;
  dispositionType: DispositionType | null;
  dispositionRecipient: string | null;
  dispositionDate: string | null;
  dispositionPrice: number | null;
  insurancePolicyId: number | null;
  scheduledCoverageAmount: number | null;
  /** FR-010; same errors as FirearmInput's. */
  mountedOn: RecordRef | null;
};

export type Accessory = AccessoryInput & {
  id: number;
  thumbnailPhotoId: number | null;
  createdAt: string;
  updatedAt: string;
};

export type AccessoryDetail = Accessory & {
  /** As FirearmDetail's. */
  dispositionHistory: DispositionHistoryEntry[];
  mount: MountDetail;
};

export type ListAccessoriesInput = {
  /** FR-018: every text field; 1-2 chars use LIKE. */
  query?: string | null;
  groupBy?: "kind" | "make" | "caliber" | "cartridge" | "mounted_on" | null;
  /** Default false (001 FR-025). */
  includeDisposed?: boolean;
};

export type AccessorySummary = {
  id: number;
  accessoryKindId: number;
  kindName: string;
  genericThumbnailKey: string;
  make: string | null;
  model: string | null;
  serialNumber: string | null;
  caliber: string | null;
  cartridge: string | null;
  status: "active" | "disposed";
  thumbnailPhotoId: number | null;
  estimatedValue: number | null;
  insuranceWarning: "none" | "uninsured" | "under_insured";
  insurancePolicyId: number | null;
  scheduledCoverageAmount: number | null;
  /** Direct host only (FR-013). */
  mountedOn: RecordLabel | null;
};

export type AccessoryGroup = {
  /** The group's heading: a kind, make, caliber, cartridge, "Unspecified",
   * "Not mounted", or "All". */
  key: string;
  /** Set only when grouped by mounted_on, for a host's group. */
  host: RecordLabel | null;
  accessories: AccessorySummary[];
};

/** `list_accessory_kinds`'s output: every kind, offered or not, in `sortOrder`. */
export type AccessoryKindsOutput = { kinds: AccessoryKind[] };

/** `list_accessories`'s output. */
export type ListAccessoriesOutput = { groups: AccessoryGroup[] };

/** `reverse_accessory_disposition`'s input (FR-006): no nickname or identity
 * re-check, so only what to do with the disposition being reversed. */
export type ReverseAccessoryInput = { history: "keep" | "discard" };
