import { useCallback, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { CommandFailure } from "../../services/tauriClient";
import * as browseService from "../browse/browseService";
import type { FirearmSummary } from "../browse/types";
import { listActionTypes } from "../firearms/firearmsService";
import type { ActionTypesOutput } from "../firearms/types";
import * as insuranceService from "../insurance/insuranceService";
import type { InsurancePolicy, ValueSummary } from "../insurance/types";
import { CollectionContext, NO_ACTION_TYPES } from "./collectionStore";
import type { CollectionState } from "./collectionStore";

export function CollectionProvider({ children }: { children: ReactNode }) {
  const [firearms, setFirearms] = useState<FirearmSummary[]>([]);
  // The list is fixed at run time (FR-017), so it is fetched once, beside the
  // policies but not with every refresh.
  const [actionTypes, setActionTypes] = useState<ActionTypesOutput>(NO_ACTION_TYPES);
  const [summary, setSummary] = useState<ValueSummary | null>(null);
  const [policies, setPolicies] = useState<InsurancePolicy[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);

  const refresh = useCallback(async () => {
    try {
      const [listing, valueSummary, policyList] = await Promise.all([
        browseService.listFirearms({ includeDisposed: true }),
        insuranceService.getValueSummary(),
        insuranceService.listInsurancePolicies(),
      ]);
      setFirearms(listing.groups.flatMap((group) => group.firearms));
      setSummary(valueSummary);
      setPolicies(policyList);
      setError(null);
    } catch (e) {
      setError(
        e instanceof CommandFailure
          ? e.message
          : "The collection couldn't be loaded. Restart HoploDex to try again.",
      );
    } finally {
      setLoaded(true);
      setRevision((r) => r + 1);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    let current = true;
    listActionTypes().then(
      (list) => current && setActionTypes(list),
      // Only guidance for the form: without it Action offers just
      // "Unspecified", and the backend still checks every save.
      () => undefined,
    );
    return () => {
      current = false;
    };
  }, []);

  const value = useMemo<CollectionState>(
    () => ({
      firearms,
      firearmsById: new Map(firearms.map((f) => [f.id, f])),
      summary,
      policies,
      policiesById: new Map(policies.map((p) => [p.id, p])),
      actionTypes,
      loaded,
      error,
      revision,
      refresh,
    }),
    [firearms, summary, policies, actionTypes, loaded, error, revision, refresh],
  );

  return <CollectionContext.Provider value={value}>{children}</CollectionContext.Provider>;
}
