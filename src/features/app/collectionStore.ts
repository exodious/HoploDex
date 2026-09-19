import { createContext, useContext } from "react";
import type { FirearmSummary } from "../browse/types";
import type { InsurancePolicy, ValueSummary } from "../insurance/types";

/**
 * Everything the app shows about the collection as a whole: every firearm's
 * browse summary (active and disposed), the value summary, and the policy
 * list. Re-fetched in full after every mutating command, so no view can
 * show a stale total or warning (FR-015, SC-003) — there is no separate
 * "refresh" action.
 */
export interface CollectionState {
  firearms: FirearmSummary[];
  firearmsById: Map<number, FirearmSummary>;
  summary: ValueSummary | null;
  policies: InsurancePolicy[];
  policiesById: Map<number, InsurancePolicy>;
  loaded: boolean;
  error: string | null;
  /** Increments after every refresh; views keyed on it refetch their own
   * detail data (a record, a filtered browse list). */
  revision: number;
  refresh: () => Promise<void>;
}

export const CollectionContext = createContext<CollectionState | null>(null);

export function useCollection(): CollectionState {
  const state = useContext(CollectionContext);
  if (!state) throw new Error("useCollection must be used inside CollectionProvider");
  return state;
}

/** "Glock 19" — the display name for a firearm. */
export function firearmName(firearm: { make: string; model: string }): string {
  return `${firearm.make} ${firearm.model}`;
}
