// Mirrors src-tauri/src/models/firearm.rs's wire shapes (camelCase, per
// contracts/tauri-commands.md).

export type FirearmStatus = "active" | "disposed";

export type DispositionType = "sold" | "traded" | "gifted" | "destroyed" | "lost_stolen";

export type CoverageKind = "individually_scheduled" | "blanket";

export interface Firearm {
  id: number;
  make: string;
  model: string;
  serialNumber: string | null;
  noSerialAttested: boolean;
  caliber: string;
  firearmTypeId: number;
  notes: string | null;
  accessories: string | null;
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
  coverageKind: CoverageKind | null;
  scheduledCoverageAmount: number | null;
  createdAt: string;
  updatedAt: string;
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

export function dispositionLabel(type: DispositionType | null): string {
  return DISPOSITION_TYPE_OPTIONS.find((o) => o.value === type)?.label ?? "Disposed";
}
