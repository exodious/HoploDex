// Typed wrappers over `invoke` for the accessory commands: contracts/tauri-commands.md
// "Accessories (new)"; the screens that use them are in contracts/ui-accessories.md.

import { invoke } from "../../services/tauriClient";
import type { DisposeInput } from "../firearms/types";
import type {
  Accessory,
  AccessoryDetail,
  AccessoryInput,
  AccessoryKindsOutput,
  ListAccessoriesInput,
  ListAccessoriesOutput,
  ReverseAccessoryInput,
} from "./types";

export function listAccessoryKinds(): Promise<AccessoryKindsOutput> {
  return invoke<AccessoryKindsOutput>("list_accessory_kinds");
}

export function createAccessory(input: AccessoryInput): Promise<Accessory> {
  return invoke<Accessory>("create_accessory", { input });
}

export function updateAccessory(id: number, input: AccessoryInput): Promise<Accessory> {
  return invoke<Accessory>("update_accessory", { id, input });
}

export function getAccessory(id: number): Promise<AccessoryDetail> {
  return invoke<AccessoryDetail>("get_accessory", { id });
}

export function listAccessories(input: ListAccessoriesInput): Promise<ListAccessoriesOutput> {
  return invoke<ListAccessoriesOutput>("list_accessories", { input });
}

/** The same input a firearm's disposal takes (`withMounted` included). */
export function disposeAccessory(id: number, input: DisposeInput): Promise<Accessory> {
  return invoke<Accessory>("dispose_accessory", { id, input });
}

export function reverseAccessoryDisposition(
  id: number,
  input: ReverseAccessoryInput,
): Promise<Accessory> {
  return invoke<Accessory>("reverse_accessory_disposition", { id, input });
}

export function deleteAccessory(id: number, confirmed: boolean): Promise<{ deleted: boolean }> {
  return invoke<{ deleted: boolean }>("delete_accessory", { id, confirmed });
}

/** `policyId: null` unschedules the accessory (blanket cover, if any). */
export function assignAccessoryCoverage(
  accessoryId: number,
  policyId: number | null,
  scheduledCoverageAmount: number | null,
): Promise<Accessory> {
  return invoke<Accessory>("assign_accessory_coverage", {
    accessoryId,
    policyId,
    scheduledCoverageAmount,
  });
}
