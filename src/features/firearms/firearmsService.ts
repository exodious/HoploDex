import { invoke } from "../../services/tauriClient";
import type {
  ActionTypesOutput,
  DisposeFirearmInput,
  EntryFieldName,
  Firearm,
  FirearmDetail,
  FirearmInput,
  ReverseDispositionInput,
  SettleEntryOutput,
  Suggestion,
} from "./types";

/** `confirmedWarnings` resends after an `ORIGINAL_MARKS_MATCH` (FR-009). */
export function createFirearm(input: FirearmInput, confirmedWarnings?: boolean): Promise<Firearm> {
  return invoke<Firearm>("create_firearm", { input, confirmedWarnings });
}

export function updateFirearm(
  id: number,
  input: FirearmInput,
  confirmedWarnings?: boolean,
): Promise<Firearm> {
  return invoke<Firearm>("update_firearm", { id, input, confirmedWarnings });
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

/** specs/004-cartridges-action-types FR-017/FR-018: the fixed action list and
 * which actions each firearm type allows. */
export function listActionTypes(): Promise<ActionTypesOutput> {
  return invoke<ActionTypesOutput>("list_action_types");
}

/** specs/004-cartridges-action-types: what an entered value becomes, and for
 * a cartridge the caliber it derives (contracts/tauri-commands.md). */
export function settleEntry(field: EntryFieldName, text: string): Promise<SettleEntryOutput> {
  return invoke<SettleEntryOutput>("settle_entry", { input: { field, text } });
}

/** The ranked suggestions for what is typed so far, best first, at most 20.
 * `make` is the make on the form, which limits a model's list to that make's
 * models (contracts/tauri-commands.md). Nothing is kept between calls
 * (FR-011). */
export async function suggestEntries(
  field: EntryFieldName,
  text: string,
  make?: string | null,
): Promise<Suggestion[]> {
  const { suggestions } = await invoke<{ suggestions: Suggestion[] }>("suggest_entries", {
    input: { field, text, make: make ?? null },
  });
  return suggestions;
}
