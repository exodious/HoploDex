import type { CycleStyle, PlateTiming } from "../../src/features/databases/plate/timing";

/*
 * What the tuner's Save will write into timing.ts. The request body is
 * untrusted, so nothing in it reaches the file until it has passed here,
 * and only what this returns (fresh, checked numbers, booleans and one of
 * the known style names) is serialized. `SCHEMA` has to list every
 * PlateTiming member (the compiler checks), and its test checks each
 * kind against timing.ts's own values.
 */

export type Spec =
  | { kind: "number"; min: number; max: number }
  | { kind: "tuple"; length: number; min: number[]; max: number[] }
  | { kind: "boolean" }
  | { kind: "enum"; values: readonly string[] };

export const CYCLE_STYLES: readonly CycleStyle[] = [
  "slide",
  "redraw",
  "slideDraw",
  "straightedge",
  "erase",
];

/** Seconds, and the odd negative gap: far wider than any slider. */
const time: Spec = { kind: "number", min: -100, max: 100 };

export const SCHEMA: Record<keyof PlateTiming, Spec> = {
  SPEED: { kind: "number", min: 0.1, max: 10 },
  KEY_START: time,
  KEY_TIME: time,
  KEY_UNIT_TIME: time,
  RIFLE_START: time,
  RIFLE_TIME: time,
  RIM_START: time,
  RIM_TIME: time,
  // a cubic-bezier: x1, y1, x2, y2 (the x's must stay within 0 to 1)
  RIM_EASE: { kind: "tuple", length: 4, min: [0, -2, 0, -2], max: [1, 2, 1, 2] },
  BRAID_LAG: time,
  TONGUE_TIME: time,
  BEAD_TIME: time,
  SHIELD_FILL_AT: time,
  SHIELD_FILL_TIME: time,
  OWL_GAP: time,
  OWL_LINES_TIME: time,
  OWL_PAPER_AFTER: time,
  BEAK_FILL_AFTER: time,
  BEAK_FILL_TIME: time,
  WASH_AFTER: time,
  WASH_TIME: time,
  PUPIL_PAUSE: time,
  PUPIL_TIME: time,
  LABELS_START: time,
  LABELS_TIME: time,
  CYCLE: { kind: "boolean" },
  CYCLE_HOLD: time,
  CYCLE_STYLE: { kind: "enum", values: CYCLE_STYLES },
  CYCLE_OUT_TIME: time,
  CYCLE_IN_TIME: time,
  CYCLE_OVERLAP: time,
  // plate pixels
  CYCLE_SLIDE: { kind: "number", min: -1000, max: 1000 },
  CYCLE_CAPTION_TIME: time,
  BLINK_OUT_TIME: time,
  BLINK_WAIT: time,
};

function isNumberIn(value: unknown, min: number, max: number): value is number {
  return typeof value === "number" && Number.isFinite(value) && value >= min && value <= max;
}

/**
 * Checks `input` (any parsed JSON) as a set of changes to the timing:
 * a plain object, each key one of PlateTiming's own, each value of that
 * member's kind and within its range. Throws an Error saying what is wrong,
 * and otherwise returns a new object holding only checked values.
 */
export function validateTiming(input: unknown): Partial<PlateTiming> {
  if (typeof input !== "object" || input === null || Array.isArray(input))
    throw new Error("expected an object of timing values");
  const out: Record<string, number | boolean | string | number[]> = {};
  for (const [name, value] of Object.entries(input)) {
    if (!Object.hasOwn(SCHEMA, name)) throw new Error(`${name} isn't a timing name`);
    const spec = SCHEMA[name as keyof PlateTiming];
    switch (spec.kind) {
      case "number":
        if (!isNumberIn(value, spec.min, spec.max))
          throw new Error(`${name} must be a number from ${spec.min} to ${spec.max}`);
        out[name] = value;
        break;
      case "tuple":
        if (
          !Array.isArray(value) ||
          value.length !== spec.length ||
          !value.every((n, i) => isNumberIn(n, spec.min[i], spec.max[i]))
        )
          throw new Error(
            `${name} must be ${spec.length} numbers, each within ${spec.min.join("/")} to ${spec.max.join("/")}`,
          );
        out[name] = value.map(Number);
        break;
      case "boolean":
        if (typeof value !== "boolean") throw new Error(`${name} must be true or false`);
        out[name] = value;
        break;
      case "enum": {
        const choice = spec.values.find((v) => v === value);
        if (choice === undefined)
          throw new Error(`${name} must be one of ${spec.values.join(", ")}`);
        out[name] = choice;
        break;
      }
    }
  }
  return out as Partial<PlateTiming>;
}
