import { invoke } from "../../services/tauriClient";
import type { DisposeFirearmInput, Firearm, FirearmInput, SavedFirearm } from "./types";

export function createFirearm(input: FirearmInput): Promise<SavedFirearm> {
  return invoke<SavedFirearm>("create_firearm", { input });
}

export function updateFirearm(id: number, input: FirearmInput): Promise<SavedFirearm> {
  return invoke<SavedFirearm>("update_firearm", { id, input });
}

export function disposeFirearm(id: number, input: DisposeFirearmInput): Promise<Firearm> {
  return invoke<Firearm>("dispose_firearm", { id, input });
}

export function deleteFirearm(id: number, confirmed: boolean): Promise<{ deleted: boolean }> {
  return invoke<{ deleted: boolean }>("delete_firearm", { id, confirmed });
}

export function getFirearm(id: number): Promise<Firearm> {
  return invoke<Firearm>("get_firearm", { id });
}
