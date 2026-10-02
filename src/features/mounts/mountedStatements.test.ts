import { describe, expect, it } from "vitest";
import { mountedStatements } from "./mountedStatements";
import type { MountedEntry, RecordLabel } from "./types";

// FR-014, issue #56: the statements under the dispose choices name the kinds
// they are about and what those were mounted on.

const label = (
  id: number,
  kind: "firearm" | "accessory",
  make: string,
  model: string,
  typeName: string,
): RecordLabel => ({
  record: { kind, id },
  make,
  model,
  nickname: null,
  typeName,
  serialNumber: null,
  status: "active",
});

const receiver = { kind: "firearm", id: 1 } as const;
const upper = label(11, "accessory", "BCM", "RECCE-16", "Upper receiver");
const can = label(5, "firearm", "SilencerCo", "Omega", "Suppressor");
const dot = label(12, "accessory", "Aimpoint", "T-2", "Optic");
const light = label(14, "accessory", "SureFire", "M300", "Light or laser");

/** An upper (with a red dot and a light) and a suppressor on the receiver. */
const mounted: MountedEntry[] = [
  { label: upper, host: receiver, depth: 1 },
  { label: dot, host: upper.record, depth: 2 },
  { label: light, host: upper.record, depth: 2 },
  { label: can, host: receiver, depth: 1 },
];

const key = (l: RecordLabel) => `${l.record.kind}:${l.record.id}`;

describe("mountedStatements", () => {
  it("names both kinds when firearms and accessories are kept", () => {
    expect(mountedStatements("LaRue PredatAR", mounted, {})).toEqual([
      "Kept firearms and accessories mounted on LaRue PredatAR will be unmounted.",
      "Accessories kept with what they are mounted on stay mounted.",
    ]);
  });

  it("names the record disposed with it that kept ones were mounted on", () => {
    expect(mountedStatements("LaRue PredatAR", mounted, { [key(upper)]: "" })).toEqual([
      "The kept firearm mounted on LaRue PredatAR will be unmounted.",
      "Kept accessories mounted on BCM RECCE-16 · Upper receiver will be unmounted.",
    ]);
  });

  it("says nothing about what is disposed with it", () => {
    const all = Object.fromEntries(mounted.map((entry) => [key(entry.label), ""]));
    expect(mountedStatements("LaRue PredatAR", mounted, all)).toEqual([]);
  });
});
