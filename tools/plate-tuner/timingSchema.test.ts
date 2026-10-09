import { describe, expect, it } from "vitest";
import source from "../../src/features/databases/plate/timing.ts?raw";
import { PLATE_TIMING } from "../../src/features/databases/plate/timing";
import { EASES, GROUPS, PRESETS, STYLES } from "./knobs";
import { literal, writeTiming } from "./saveTiming";
import { CYCLE_STYLES, SCHEMA, validateTiming } from "./timingSchema";

describe("the Save schema", () => {
  it("lists exactly timing.ts's members, each of the kind its value is", () => {
    expect(Object.keys(SCHEMA).sort()).toEqual(Object.keys(PLATE_TIMING).sort());
    for (const [name, value] of Object.entries(PLATE_TIMING)) {
      const spec = SCHEMA[name as keyof typeof SCHEMA];
      if (Array.isArray(value)) {
        expect(spec.kind).toBe("tuple");
        if (spec.kind === "tuple") expect(spec.length).toBe(value.length);
      } else {
        expect(spec.kind).toBe(
          typeof value === "number" ? "number" : typeof value === "boolean" ? "boolean" : "enum",
        );
      }
    }
    expect([...CYCLE_STYLES].sort()).toEqual(STYLES.map(([style]) => style).sort());
  });

  it("accepts timing.ts's own values, every slider's ends, the eases and the presets", () => {
    expect(validateTiming(PLATE_TIMING)).toEqual(PLATE_TIMING);
    for (const [, knobs] of GROUPS)
      for (const knob of knobs)
        if (knob.kind === "range") {
          expect(() => validateTiming({ [knob.name]: knob.min })).not.toThrow();
          expect(() => validateTiming({ [knob.name]: knob.max })).not.toThrow();
        }
    for (const [, ease] of EASES) expect(() => validateTiming({ RIM_EASE: ease })).not.toThrow();
    for (const [, , set] of PRESETS) expect(() => validateTiming(set)).not.toThrow();
  });

  it.each([
    ["a key outside the schema", { SPEED: 1, NOT_A_KNOB: 1 }],
    ["Object's own names", { constructor: 1 }],
    ["a __proto__ key", JSON.parse('{"__proto__": {"SPEED": 1}}')],
    ["a string for a number", { SPEED: "1" }],
    ["NaN", { SPEED: NaN }],
    ["Infinity", { SPEED: Infinity }],
    ["a number past its range", { SPEED: 11 }],
    ["a negative where there can't be", { SPEED: -1 }],
    ["null", { KEY_TIME: null }],
    ["an array for a number", { KEY_TIME: [1] }],
    ["a string for the ease", { RIM_EASE: "0, 0, 1, 1" }],
    ["an ease of 3", { RIM_EASE: [0, 0, 1] }],
    ["an ease of 5", { RIM_EASE: [0, 0, 1, 1, 1] }],
    ["a string among the ease's numbers", { RIM_EASE: [0, 0, "1); alert(1); (", 1] }],
    ["an ease x past 1", { RIM_EASE: [0, 0, 2, 1] }],
    ["a number for the switch", { CYCLE: 1 }],
    ["a string for the switch", { CYCLE: "true" }],
    ["a style it doesn't have", { CYCLE_STYLE: "wipe" }],
    ["a style that is code", { CYCLE_STYLE: 'erase", X: (alert(1)), Y: "' }],
    ["a list", [{ SPEED: 1 }]],
    ["null", null],
    ["a string", "SPEED"],
  ])("refuses %s", (_name, input) => {
    expect(() => validateTiming(input)).toThrow();
  });

  it("returns a copy holding only the checked values", () => {
    const input = { SPEED: 1, RIM_EASE: [0, 0, 1, 1] as [number, number, number, number] };
    const out = validateTiming(input);
    expect(out).toEqual(input);
    expect(out.RIM_EASE).not.toBe(input.RIM_EASE);
  });
});

describe("the Save serializer", () => {
  it("refuses an array member that is code, and an unknown key, writing nothing", () => {
    const attack = { RIM_EASE: [0, 0, "1); alert(1); (", 1] } as never;
    expect(() => writeTiming(source, attack)).toThrow();
    expect(() => writeTiming(source, { NOT_A_KNOB: 1 } as never)).toThrow(/NOT_A_KNOB/);
    expect(() => writeTiming(source, { SPEED: 2, "KEY_TIME: 1, //": 1 } as never)).toThrow();
  });

  it("never lets code through a literal", () => {
    expect(() => literal(["1); alert(1); (", 0, 0, 1] as never)).toThrow();
    expect(() => literal([0, 0, NaN, 1])).toThrow();
    expect(literal([0.45, 0, 0.55, 1])).toBe("[0.45, 0, 0.55, 1]");
    expect(literal(0.1234567)).toBe("0.123");
  });
});
