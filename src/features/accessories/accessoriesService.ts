// Typed wrappers over `invoke` for the accessory commands: 006 contracts/tauri-commands.md
// "Accessories (new)"; the screens that use them are in 006 contracts/ui-accessories.md.

import type { SessionScope } from "../session/sessionScope";
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

export function listAccessoryKinds(scope: SessionScope): Promise<AccessoryKindsOutput> {
  return scope.invoke<AccessoryKindsOutput>("list_accessory_kinds");
}

export function createAccessory(scope: SessionScope, input: AccessoryInput): Promise<Accessory> {
  return scope.invoke<Accessory>("create_accessory", { input });
}

export function updateAccessory(
  scope: SessionScope,
  id: number,
  input: AccessoryInput,
): Promise<Accessory> {
  return scope.invoke<Accessory>("update_accessory", { id, input });
}

export function getAccessory(scope: SessionScope, id: number): Promise<AccessoryDetail> {
  return scope.invoke<AccessoryDetail>("get_accessory", { id });
}

export function listAccessories(
  scope: SessionScope,
  input: ListAccessoriesInput,
): Promise<ListAccessoriesOutput> {
  return scope.invoke<ListAccessoriesOutput>("list_accessories", { input });
}

/** The same input a firearm's disposal takes (`withMounted` included). */
export function disposeAccessory(
  scope: SessionScope,
  id: number,
  input: DisposeInput,
): Promise<Accessory> {
  return scope.invoke<Accessory>("dispose_accessory", { id, input });
}

export function reverseAccessoryDisposition(
  scope: SessionScope,
  id: number,
  input: ReverseAccessoryInput,
): Promise<Accessory> {
  return scope.invoke<Accessory>("reverse_accessory_disposition", { id, input });
}

export function deleteAccessory(
  scope: SessionScope,
  id: number,
  confirmed: boolean,
): Promise<{ deleted: boolean }> {
  return scope.invoke<{ deleted: boolean }>("delete_accessory", { id, confirmed });
}

/** `policyId: null` unschedules the accessory (blanket cover, if any). */
export function assignAccessoryCoverage(
  scope: SessionScope,
  accessoryId: number,
  policyId: number | null,
  scheduledCoverageAmount: number | null,
): Promise<Accessory> {
  // `assign_accessory_coverage(accessory_id, input)`, as `assign_firearm_coverage` is.
  return scope.invoke<Accessory>("assign_accessory_coverage", {
    accessoryId,
    input: { policyId, scheduledCoverageAmount },
  });
}
