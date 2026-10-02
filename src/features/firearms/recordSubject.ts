import type { Accessory, AccessoryKind } from "../accessories/types";
import { firearmName, useAccessoryKinds } from "../app/collectionStore";
import { accessoryNameText } from "../mounts/recordNames";
import type { RecordKind } from "../mounts/types";
import type { Firearm } from "./types";

// specs/006-accessory-links FR-009, contracts/ui-accessories.md §7, §10: the
// dispose, restore and coverage dialogs serve a record of either kind. A
// caller gives exactly one of `firearm` and `accessory`.

export type RecordSubjectProps =
  { firearm: Firearm; accessory?: undefined } | { accessory: Accessory; firearm?: undefined };

/** What a dialog needs to know about the record it serves. */
export interface RecordSubject {
  kind: RecordKind;
  id: number;
  /** The record's name in plain text, as `RecordName` shows it (FR-005). */
  name: string;
  /** What to call it in a sentence: "firearm" or "accessory". */
  noun: "firearm" | "accessory";
  /** A firearm's nickname, for the one dialog that renames; an accessory has none. */
  nickname: string | null;
  acquisitionDate: string | null;
  estimatedValue: number | null;
  insurancePolicyId: number | null;
  scheduledCoverageAmount: number | null;
  dispositionType: Firearm["dispositionType"];
  dispositionRecipient: string | null;
  dispositionDate: string | null;
  dispositionPrice: number | null;
}

/** "{make} {model} · {kind}" (FR-005). */
export function accessoryName(
  accessory: Pick<Accessory, "make" | "model" | "accessoryKindId">,
  kinds: AccessoryKind[],
): string {
  const kind = kinds.find((k) => k.id === accessory.accessoryKindId)?.name ?? "Accessory";
  return accessoryNameText(accessory.make, accessory.model, kind);
}

export function useRecordSubject({ firearm, accessory }: RecordSubjectProps): RecordSubject {
  const { kinds } = useAccessoryKinds();
  if (accessory) {
    return {
      kind: "accessory",
      id: accessory.id,
      name: accessoryName(accessory, kinds),
      noun: "accessory",
      nickname: null,
      acquisitionDate: accessory.acquisitionDate,
      estimatedValue: accessory.estimatedValue,
      insurancePolicyId: accessory.insurancePolicyId,
      scheduledCoverageAmount: accessory.scheduledCoverageAmount,
      dispositionType: accessory.dispositionType,
      dispositionRecipient: accessory.dispositionRecipient,
      dispositionDate: accessory.dispositionDate,
      dispositionPrice: accessory.dispositionPrice,
    };
  }
  return {
    kind: "firearm",
    id: firearm.id,
    name: firearmName(firearm),
    noun: "firearm",
    nickname: firearm.nickname,
    acquisitionDate: firearm.acquisitionDate,
    estimatedValue: firearm.estimatedValue,
    insurancePolicyId: firearm.insurancePolicyId,
    scheduledCoverageAmount: firearm.scheduledCoverageAmount,
    dispositionType: firearm.dispositionType,
    dispositionRecipient: firearm.dispositionRecipient,
    dispositionDate: firearm.dispositionDate,
    dispositionPrice: firearm.dispositionPrice,
  };
}
