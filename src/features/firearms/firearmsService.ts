import type { SessionScope } from "../session/sessionScope";
import type {
  ActionTypesOutput,
  DisposeInput,
  EntryFieldName,
  Firearm,
  FirearmDetail,
  FirearmInput,
  FirearmTypesOutput,
  RegistrationClassesOutput,
  ReverseDispositionInput,
  SettleEntryOutput,
  Suggestion,
} from "./types";

/** `confirmedWarnings` resends after an `ORIGINAL_MARKS_MATCH` (002 FR-009). */
export function createFirearm(
  scope: SessionScope,
  input: FirearmInput,
  confirmedWarnings?: boolean,
): Promise<Firearm> {
  return scope.invoke<Firearm>("create_firearm", { input, confirmedWarnings });
}

export function updateFirearm(
  scope: SessionScope,
  id: number,
  input: FirearmInput,
  confirmedWarnings?: boolean,
): Promise<Firearm> {
  return scope.invoke<Firearm>("update_firearm", { id, input, confirmedWarnings });
}

export function disposeFirearm(
  scope: SessionScope,
  id: number,
  input: DisposeInput,
): Promise<Firearm> {
  return scope.invoke<Firearm>("dispose_firearm", { id, input });
}

export function reverseDisposition(
  scope: SessionScope,
  id: number,
  input: ReverseDispositionInput,
): Promise<Firearm> {
  return scope.invoke<Firearm>("reverse_disposition", { id, input });
}

export function deleteFirearm(
  scope: SessionScope,
  id: number,
  confirmed: boolean,
): Promise<{ deleted: boolean }> {
  return scope.invoke<{ deleted: boolean }>("delete_firearm", { id, confirmed });
}

export function getFirearm(scope: SessionScope, id: number): Promise<FirearmDetail> {
  return scope.invoke<FirearmDetail>("get_firearm", { id });
}

/** 004 FR-017/FR-018: the fixed action list and
 * which actions each firearm type allows. */
export function listActionTypes(scope: SessionScope): Promise<ActionTypesOutput> {
  return scope.invoke<ActionTypesOutput>("list_action_types");
}

/** 005 FR-001/FR-003: the fixed firearm types, in
 * id order, with the fields each omits. */
export function listFirearmTypes(scope: SessionScope): Promise<FirearmTypesOutput> {
  return scope.invoke<FirearmTypesOutput>("list_firearm_types");
}

/** 005 FR-007: the fixed registration
 * classifications in list order, including any no longer offered. */
export function listRegistrationClasses(scope: SessionScope): Promise<RegistrationClassesOutput> {
  return scope.invoke<RegistrationClassesOutput>("list_registration_classes");
}

/** specs/004-cartridges-action-types: what an entered value becomes, and for
 * a cartridge the caliber it derives (004 contracts/tauri-commands.md). */
export function settleEntry(
  scope: SessionScope,
  field: EntryFieldName,
  text: string,
): Promise<SettleEntryOutput> {
  return scope.invoke<SettleEntryOutput>("settle_entry", { input: { field, text } });
}

/** The ranked suggestions for what is typed so far, best first, at most 20.
 * `make` is the make on the form, which limits a model's list to that make's
 * models (004 contracts/tauri-commands.md). Nothing is kept between calls
 * (004 FR-011). */
export async function suggestEntries(
  scope: SessionScope,
  field: EntryFieldName,
  text: string,
  make?: string | null,
): Promise<Suggestion[]> {
  const { suggestions } = await scope.invoke<{ suggestions: Suggestion[] }>("suggest_entries", {
    input: { field, text, make: make ?? null },
  });
  return suggestions;
}
