import { useCallback, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { CommandFailure } from "../../services/tauriClient";
import { listAccessories, listAccessoryKinds } from "../accessories/accessoriesService";
import type { AccessoryKindsOutput, AccessorySummary } from "../accessories/types";
import * as browseService from "../browse/browseService";
import type { FirearmSummary } from "../browse/types";
import {
  listActionTypes,
  listFirearmTypes,
  listRegistrationClasses,
} from "../firearms/firearmsService";
import type {
  ActionTypesOutput,
  FirearmTypesOutput,
  RegistrationClassesOutput,
} from "../firearms/types";
import * as insuranceService from "../insurance/insuranceService";
import type { InsurancePolicy, ValueSummary } from "../insurance/types";
import { loadDocumentOpening } from "../media/documentOpening";
import {
  CollectionContext,
  NO_ACCESSORY_KINDS,
  NO_ACTION_TYPES,
  NO_FIREARM_TYPES,
  NO_REGISTRATION_CLASSES,
} from "./collectionStore";
import type { CollectionState } from "./collectionStore";

export function CollectionProvider({ children }: { children: ReactNode }) {
  const [firearms, setFirearms] = useState<FirearmSummary[]>([]);
  // The list is fixed at run time (FR-017), so it is fetched once, beside the
  // policies but not with every refresh.
  const [accessories, setAccessories] = useState<AccessorySummary[]>([]);
  const [actionTypes, setActionTypes] = useState<ActionTypesOutput>(NO_ACTION_TYPES);
  const [actionTypesFailed, setActionTypesFailed] = useState(false);
  const [firearmTypes, setFirearmTypes] = useState<FirearmTypesOutput>(NO_FIREARM_TYPES);
  const [firearmTypesFailed, setFirearmTypesFailed] = useState(false);
  const [accessoryKinds, setAccessoryKinds] = useState<AccessoryKindsOutput>(NO_ACCESSORY_KINDS);
  const [accessoryKindsFailed, setAccessoryKindsFailed] = useState(false);
  const [registrationClasses, setRegistrationClasses] =
    useState<RegistrationClassesOutput>(NO_REGISTRATION_CLASSES);
  const [registrationClassesFailed, setRegistrationClassesFailed] = useState(false);
  const [summary, setSummary] = useState<ValueSummary | null>(null);
  const [policies, setPolicies] = useState<InsurancePolicy[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);

  const refresh = useCallback(async () => {
    try {
      const [listing, accessoryListing, valueSummary, policyList] = await Promise.all([
        browseService.listFirearms({ includeDisposed: true }),
        listAccessories({ includeDisposed: true }),
        insuranceService.getValueSummary(),
        insuranceService.listInsurancePolicies(),
      ]);
      setFirearms(listing.groups.flatMap((group) => group.firearms));
      setAccessories(accessoryListing.groups.flatMap((group) => group.accessories));
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
      // Without it Action offers just "Unspecified" and the form says so;
      // the backend still checks every save.
      () => current && setActionTypesFailed(true),
    );
    return () => {
      current = false;
    };
  }, []);

  useEffect(() => {
    let current = true;
    listFirearmTypes().then(
      (list) => current && setFirearmTypes(list),
      // Without it Type offers nothing and the form says so; the backend
      // still checks every save.
      () => current && setFirearmTypesFailed(true),
    );
    return () => {
      current = false;
    };
  }, []);

  useEffect(() => {
    let current = true;
    listAccessoryKinds().then(
      (list) => current && setAccessoryKinds(list),
      // Without it Kind offers nothing and the form says so; the backend
      // still checks every save.
      () => current && setAccessoryKindsFailed(true),
    );
    return () => {
      current = false;
    };
  }, []);

  useEffect(() => {
    let current = true;
    listRegistrationClasses().then(
      (list) => current && setRegistrationClasses(list),
      // Without it Registered as offers only Unspecified and the form says
      // so; the backend still checks every save.
      () => current && setRegistrationClassesFailed(true),
    );
    return () => {
      current = false;
    };
  }, []);

  // This computer's "Open documents" setting is read once the collection is open, for the
  // document lists to follow. Until it is read, or if it can't be, the default "preview" holds.
  useEffect(() => {
    loadDocumentOpening().catch(() => {});
  }, []);

  const value = useMemo<CollectionState>(
    () => ({
      firearms,
      firearmsById: new Map(firearms.map((f) => [f.id, f])),
      accessories,
      accessoriesById: new Map(accessories.map((a) => [a.id, a])),
      summary,
      policies,
      policiesById: new Map(policies.map((p) => [p.id, p])),
      actionTypes,
      actionTypesFailed,
      firearmTypes,
      firearmTypesFailed,
      accessoryKinds,
      accessoryKindsFailed,
      registrationClasses,
      registrationClassesFailed,
      loaded,
      error,
      revision,
      refresh,
    }),
    [
      firearms,
      accessories,
      summary,
      policies,
      actionTypes,
      actionTypesFailed,
      firearmTypes,
      firearmTypesFailed,
      accessoryKinds,
      accessoryKindsFailed,
      registrationClasses,
      registrationClassesFailed,
      loaded,
      error,
      revision,
      refresh,
    ],
  );

  return <CollectionContext.Provider value={value}>{children}</CollectionContext.Provider>;
}
