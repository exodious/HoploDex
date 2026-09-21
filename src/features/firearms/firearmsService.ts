import { invoke } from "../../services/tauriClient";
import type {
  DisposeFirearmInput,
  Firearm,
  FirearmDetail,
  FirearmInput,
  ReverseDispositionInput,
} from "./types";

export function createFirearm(input: FirearmInput): Promise<Firearm> {
  return invoke<Firearm>("create_firearm", { input });
}

export function updateFirearm(id: number, input: FirearmInput): Promise<Firearm> {
  return invoke<Firearm>("update_firearm", { id, input });
}

export function disposeFirearm(id: number, input: DisposeFirearmInput): Promise<Firearm> {
  return invoke<Firearm>("dispose_firearm", { id, input });
}

export function reverseDisposition(id: number, input: ReverseDispositionInput): Promise<Firearm> {
  return invoke<Firearm>("reverse_disposition", { id, input });
}

export function deleteFirearm(id: number, confirmed: boolean): Promise<{ deleted: boolean }> {
  return invoke<{ deleted: boolean }>("delete_firearm", { id, confirmed });
}

export function getFirearm(id: number): Promise<FirearmDetail> {
  return invoke<FirearmDetail>("get_firearm", { id });
}
