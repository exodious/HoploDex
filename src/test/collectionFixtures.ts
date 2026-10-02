import type { AccessoryKind } from "../features/accessories/types";
import type { ActionType, FirearmTypeInfo, RegistrationClass } from "../features/firearms/types";

/** The seeded firearm types (src-tauri/src/db/migrations/0003_seed_firearm_types.sql),
 * as `list_firearm_types` returns them, in list order (Other last), so every
 * frontend test builds its `CollectionState` from one copy of the seed. */
export const FIREARM_TYPES: FirearmTypeInfo[] = (
  [
    [1, "Handgun", "handgun"],
    [2, "Rifle", "rifle"],
    [3, "Shotgun", "shotgun"],
    [5, "Suppressor", "suppressor"],
    [4, "Other", "other"],
  ] as const
).map(([id, name, key]) => {
  // Suppressor has no action, barrel length or capacity (FR-003), and its
  // caliber is never derived (FR-002).
  const applies = key !== "suppressor";
  return {
    id,
    name,
    genericThumbnailKey: key,
    actionTypeApplies: applies,
    barrelLengthApplies: applies,
    capacityApplies: applies,
    caliberFromCartridge: applies,
  };
});

/** The six seeded registration classifications, all offered. */
export const REGISTRATION_CLASSES: RegistrationClass[] = [
  "Suppressor",
  "Short-barreled rifle",
  "Short-barreled shotgun",
  "Any other weapon",
  "Machine gun",
  "Destructive device",
].map((name, index) => ({ id: index + 1, name, offered: true }));

/** 004's twelve actions and 005's "Automatic or select-fire" (id 13), with
 * their real ids, in list order: id 13 sits seventh. */
export const ACTION_TYPES: ActionType[] = [
  [1, "Semi-automatic"],
  [2, "Revolver"],
  [3, "Bolt action"],
  [4, "Lever action"],
  [5, "Pump action"],
  [6, "Break action"],
  [13, "Automatic or select-fire"],
  [7, "Falling block"],
  [8, "Rolling block"],
  [9, "Single shot (other)"],
  [10, "Flintlock"],
  [11, "Percussion"],
  [12, "Inline muzzleloader"],
].map(([id, name]) => ({ id: id as number, name: name as string }));

/** The fourteen seeded accessory kinds (specs/006-accessory-links/data-model.md;
 * src-tauri/src/db/migrations/0003_seed_firearm_types.sql), as
 * `list_accessory_kinds` returns them, in `sortOrder`, all offered, so every
 * frontend test builds its state from one copy of the seed. */
export const ACCESSORY_KINDS: AccessoryKind[] = [
  [1, "Optic", "optic"],
  [2, "Light or laser", "light"],
  [3, "Magazine", "magazine"],
  [4, "Stock or brace", "stock"],
  [5, "Upper receiver", "upper"],
  [6, "Barrel", "barrel"],
  [13, "Trigger", "trigger"],
  [7, "Muzzle device", "muzzle"],
  [8, "Conversion kit", "conversion"],
  [9, "Mount or rail", "mount"],
  [14, "Bipod", "bipod"],
  [10, "Sling", "sling"],
  [11, "Case", "case"],
  [12, "Other", "accessory"],
].map(([id, name, key], index) => ({
  id: id as number,
  name: name as string,
  genericThumbnailKey: key as string,
  sortOrder: index + 1,
  offered: true,
}));
