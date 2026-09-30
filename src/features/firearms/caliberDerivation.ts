// specs/004-cartridges-action-types research.md §8 and contracts/ui-entry.md
// §3: the caliber field's state on the firearm form, beside its text. Pure,
// so every transition is tested without rendering the form.

import type { DerivedCaliber } from "./types";

/** `derived`: the caliber follows the cartridge. `edited`: the user's (or a
 * saved firearm's) caliber, which a cartridge change never overwrites
 * (FR-006). */
export type CaliberMode = "derived" | "edited";

export interface CaliberState {
  caliber: string;
  mode: CaliberMode;
  /** How a derived caliber was filled; a guess is marked as one (FR-005). */
  source: DerivedCaliber["source"] | null;
  /** In `edited` mode, the caliber the current cartridge derives when it
   * differs from the field's (US1-7). */
  suggestion: string | null;
  /** The cartridge no caliber could be read from, for the prompt (US1-4). */
  prompt: string | null;
  /** The last cartridge settled and what it derived, so a caliber left
   * empty can be derived again at once. */
  derivedFrom: { cartridge: string; derived: DerivedCaliber | null } | null;
}

export type CaliberAction =
  | { type: "cartridgeSettled"; cartridge: string; derived: DerivedCaliber | null }
  | { type: "cartridgeCleared" }
  | { type: "caliberTyped"; caliber: string }
  | { type: "caliberLeft" }
  | { type: "suggestionUsed" };

/** A new firearm's caliber starts derived and empty; a saved firearm's
 * starts edited (FR-006: its saved caliber counts as already edited). */
export function initialCaliberState(savedCaliber?: string): CaliberState {
  return {
    caliber: savedCaliber ?? "",
    mode: savedCaliber === undefined ? "derived" : "edited",
    source: null,
    suggestion: null,
    prompt: null,
    derivedFrom: null,
  };
}

/** Fills a derived caliber from what the cartridge derived: its caliber, or
 * empty with the prompt when nothing could be read. */
function derive(
  state: CaliberState,
  derivedFrom: NonNullable<CaliberState["derivedFrom"]>,
): CaliberState {
  const { cartridge, derived } = derivedFrom;
  return {
    ...state,
    derivedFrom,
    mode: "derived",
    caliber: derived?.caliber ?? "",
    source: derived?.source ?? null,
    suggestion: null,
    prompt: derived ? null : cartridge,
  };
}

export function caliberReducer(state: CaliberState, action: CaliberAction): CaliberState {
  switch (action.type) {
    case "cartridgeSettled": {
      const derivedFrom = { cartridge: action.cartridge, derived: action.derived };
      if (state.mode === "derived") return derive(state, derivedFrom);
      const offered = action.derived?.caliber;
      return {
        ...state,
        derivedFrom,
        suggestion: offered !== undefined && offered !== state.caliber ? offered : null,
      };
    }
    case "cartridgeCleared":
      if (state.mode === "derived") {
        return { ...state, derivedFrom: null, caliber: "", source: null, prompt: null };
      }
      return { ...state, derivedFrom: null, suggestion: null };
    case "caliberTyped":
      return {
        ...state,
        caliber: action.caliber,
        mode: "edited",
        source: null,
        prompt: null,
        // contracts/ui-entry.md §3: the suggestion line goes once the
        // caliber is edited to the suggested value.
        suggestion: state.suggestion === action.caliber.trim() ? null : state.suggestion,
      };
    case "caliberLeft":
      if (state.caliber.trim() !== "") return state;
      if (state.derivedFrom) return derive(state, state.derivedFrom);
      return { ...state, mode: "derived", source: null, suggestion: null };
    case "suggestionUsed":
      if (state.suggestion === null) return state;
      return {
        ...state,
        caliber: state.suggestion,
        mode: "edited",
        source: null,
        suggestion: null,
      };
  }
}

/** The caliber field's hint for its state (contracts/ui-entry.md §3), or
 * none. */
export function caliberHint(state: CaliberState): string | undefined {
  if (state.prompt !== null) {
    return `We couldn't work out a caliber from “${state.prompt}”. Enter it.`;
  }
  if (state.mode !== "derived") return undefined;
  if (state.source === "catalog") return "From the cartridge.";
  if (state.source === "guess") return "Guessed from the cartridge. Check it before saving.";
  return undefined;
}
