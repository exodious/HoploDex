import { describe, expect, it } from "vitest";
import source from "../../src/features/databases/plate/timing.ts?raw";
import { PLATE_TIMING } from "../../src/features/databases/plate/timing";
import { writeTiming } from "./saveTiming";

describe("the tuner's Save", () => {
  it("can write every value timing.ts has, one line each, as it is written there", () => {
    // Saving the file's own values changes nothing: each is on a line of its
    // own that Save finds, in the form Save writes.
    expect(writeTiming(source, PLATE_TIMING)).toBe(source);
  });

  it("replaces a value and keeps its line's comment", () => {
    const out = writeTiming(source, {
      SPEED: 0.8,
      RIM_EASE: [0.45, 0, 0.55, 1],
      CYCLE_STYLE: "straightedge",
    });
    expect(out).toMatch(/^ {2}SPEED: 0\.8, \/\/ multiplies every time below/m);
    expect(out).toMatch(/^ {2}RIM_EASE: \[0\.45, 0, 0\.55, 1\], \/\/ the sweep's cubic-bezier/m);
    expect(out).toMatch(/^ {2}CYCLE_STYLE: "straightedge", \/\/ "slide", "redraw"/m);
    // and nothing else
    expect(out.split("\n").length).toBe(source.split("\n").length);
    expect(out).toMatch(/^ {2}BORE_AXIS: false,/m);
  });

  it("refuses a name the file doesn't have, saving nothing", () => {
    expect(() => writeTiming(source, { SPEED: 1, NOT_A_KNOB: 1 } as never)).toThrow(/NOT_A_KNOB/);
  });
});
