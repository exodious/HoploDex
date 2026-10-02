import type { AccessorySummary } from "../accessories/types";
import type { FirearmSummary, InsuranceWarning } from "../browse/types";
import type { RecordLabel } from "../mounts/types";

// specs/006-accessory-links FR-009, contracts/ui-accessories.md §10: the
// insurance screens list firearms and accessories together, each named by
// `RecordName` and keyed by its `RecordRef` (a firearm and an accessory can
// share an id).

/** A firearm or an accessory as the insurance lists show it. */
export interface InsuranceRecord {
  label: RecordLabel;
  estimatedValue: number | null;
  insuranceWarning: InsuranceWarning;
}

export const refKey = (record: { kind: string; id: number }) => `${record.kind}:${record.id}`;

export function firearmRecord(f: FirearmSummary): InsuranceRecord {
  return {
    label: {
      record: { kind: "firearm", id: f.id },
      make: f.make,
      model: f.model,
      nickname: f.nickname,
      typeName: f.firearmTypeName,
      serialNumber: f.serialNumber,
      status: f.status,
    },
    estimatedValue: f.estimatedValue,
    insuranceWarning: f.insuranceWarning,
  };
}

export function accessoryRecord(a: AccessorySummary): InsuranceRecord {
  return {
    label: {
      record: { kind: "accessory", id: a.id },
      make: a.make,
      model: a.model,
      nickname: null,
      typeName: a.kindName,
      serialNumber: a.serialNumber,
      status: a.status,
    },
    estimatedValue: a.estimatedValue,
    insuranceWarning: a.insuranceWarning,
  };
}
