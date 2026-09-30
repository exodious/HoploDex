import { createContext, useContext } from "react";
import type { FirearmSummary } from "../browse/types";
import type { ActionTypesOutput } from "../firearms/types";
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
  /** specs/004-cartridges-action-types FR-017: the fixed action list and
   * its mapping to firearm types, loaded once per open database. */
  actionTypes: ActionTypesOutput;
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

/** Before the list has loaded, or outside a provider: no actions to offer. */
export const NO_ACTION_TYPES: ActionTypesOutput = { actions: [], allowedByFirearmType: {} };

/** The action list and mapping, for a form that may render on its own (as
 * the form's tests do): empty without a provider, rather than an error. */
export function useActionTypes(): ActionTypesOutput {
  return useContext(CollectionContext)?.actionTypes ?? NO_ACTION_TYPES;
}

/** "Glock 19", or `Glock 19 “Old Faithful”` when it has a nickname (FR-031)
 * — the display name for a firearm wherever it is named in plain text. */
export function firearmName(firearm: {
  make: string;
  model: string;
  nickname?: string | null;
}): string {
  const name = `${firearm.make} ${firearm.model}`;
  return firearm.nickname ? `${name} “${firearm.nickname}”` : name;
}
