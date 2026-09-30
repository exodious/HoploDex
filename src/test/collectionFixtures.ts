import type { ActionType, FirearmTypeInfo, RegistrationClass } from "../features/firearms/types";

/** The seeded firearm types (src-tauri/src/db/migrations/0003_seed_firearm_types.sql),
 * as `list_firearm_types` returns them, so every frontend test builds its
 * `CollectionState` from one copy of the seed. */
export const FIREARM_TYPES: FirearmTypeInfo[] = [
  ["Handgun", "handgun"],
  ["Rifle", "rifle"],
  ["Shotgun", "shotgun"],
  ["Other", "other"],
].map(([name, key], index) => ({
  id: index + 1,
  name,
  genericThumbnailKey: key,
  actionTypeApplies: true,
  barrelLengthApplies: true,
  capacityApplies: true,
}));

// Suppressor (id 5) has no action, barrel length or capacity (FR-003).
FIREARM_TYPES.push({
  id: 5,
  name: "Suppressor",
  genericThumbnailKey: "suppressor",
  actionTypeApplies: false,
  barrelLengthApplies: false,
  capacityApplies: false,
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
