import { describe, expect, it } from "vitest";
import {
  cycleStart,
  drawInEnd,
  drawInPhases,
  duration,
  layerChanges,
  loopKeyframes,
  step,
} from "./animation";
import { PLATE_ENTRIES } from "./entries";
import { PLATE_TIMING } from "./timing";
import type { CycleStyle, PlateTiming } from "./timing";

const STYLES: CycleStyle[] = ["slide", "redraw", "slideDraw", "straightedge", "erase"];
const k = (changes: Partial<PlateTiming> = {}): PlateTiming => ({ ...PLATE_TIMING, ...changes });
const span = (id: string, t: PlateTiming = k()) => drawInPhases(t).find((p) => p.id === id)!.span;

describe("the draw-in's order", () => {
  it("starts the owl OWL_GAP after the braid closes, and fills its pupils last", () => {
    const t = k();
    const braidClosed = t.RIM_START + t.RIM_TIME + t.BRAID_LAG;
    expect(span("owl")[0]).toBeCloseTo(braidClosed + t.OWL_GAP);
    const pupils = span("pupils");
    expect(pupils[0]).toBeCloseTo(span("owl")[1] + t.PUPIL_PAUSE);
    for (const id of ["owl", "beak", "wash"]) expect(span(id)[0]).toBeLessThan(pupils[0]);
  });

  it("draws the Greek key before the rifle and the rim", () => {
    expect(span("key")[1]).toBeLessThanOrEqual(span("rifle")[0]);
    expect(span("key")[1]).toBeLessThanOrEqual(span("rim")[0]);
  });

  it("ends when its last stage does", () => {
    expect(drawInEnd(k())).toBe(Math.max(...drawInPhases(k()).map((p) => p.span[1])));
  });
});

describe("a change of drawing", () => {
  it("brings the new drawing in CYCLE_OVERLAP before the old has gone", () => {
    const s = step(
      k({ CYCLE_STYLE: "slide", CYCLE_OUT_TIME: 0.8, CYCLE_OVERLAP: 0.3, CYCLE_IN_TIME: 1 }),
    );
    expect(s.inStart).toBeCloseTo(0.5);
    expect(s.inEnd).toBeCloseTo(1.5);
    expect(s.end).toBeCloseTo(1.5);
  });

  it("leaves a gap for a negative overlap, and never starts before the change", () => {
    expect(
      step(k({ CYCLE_STYLE: "redraw", CYCLE_OUT_TIME: 0.6, CYCLE_OVERLAP: -0.4 })).inStart,
    ).toBeCloseTo(1);
    expect(step(k({ CYCLE_STYLE: "redraw", CYCLE_OUT_TIME: 0.6, CYCLE_OVERLAP: 2 })).inStart).toBe(
      0,
    );
  });

  it("wipes the old one out as the straightedge brings the new one in", () => {
    const s = step(k({ CYCLE_STYLE: "straightedge", CYCLE_IN_TIME: 1.4 }));
    expect(s.inStart).toBe(0);
    expect(s.outEnd).toBeCloseTo(1.4);
  });

  it("holds each drawing CYCLE_HOLD after it arrives, and starts once the draw-in has held as long", () => {
    const t = k({ CYCLE_HOLD: 4 });
    expect(step(t).length).toBeCloseTo(step(t).end + 4);
    expect(cycleStart(t)).toBeCloseTo(drawInEnd(t) + 4);
  });

  it("times the tuner's whole loop through every drawing, scaled by SPEED", () => {
    const t = k({ SPEED: 2 });
    const n = PLATE_ENTRIES.length;
    expect(duration(t, n)).toBeCloseTo(2 * (cycleStart(t) + n * step(t).length));
    expect(duration(k({ CYCLE: false }), n)).toBeCloseTo(PLATE_TIMING.SPEED * drawInEnd(k()));
  });
});

describe("the loop's keyframes", () => {
  const n = PLATE_ENTRIES.length;

  for (const style of STYLES) {
    it(`bring every drawing in and out once a loop, seamlessly (${style})`, () => {
      const t = k({ CYCLE_STYLE: style });
      const period = n * step(t).length;
      for (let i = 0; i < n; i++) {
        const changes = layerChanges(t, i, n);
        const start = i === 0 ? "1" : "0";
        for (const [property, list, first] of [
          ["opacity", changes.opacity, start],
          ["opacity", changes.caption, start],
          ["transform", changes.transform, "translateX(0px)"],
          ["transform", changes.clip, "translateX(0px)"],
          ["strokeDashoffset", changes.dash, "0"],
          ["fillOpacity", changes.fill, "1"],
        ] as const) {
          const frames = loopKeyframes(property, period, first, list);
          const offsets = frames.map((f) => Number(f.offset));
          expect(offsets[0]).toBe(0);
          expect(offsets.at(-1)).toBe(1);
          // in order, within the loop
          offsets.forEach((o, j) => {
            expect(o).toBeGreaterThanOrEqual(j ? offsets[j - 1] : 0);
            expect(o).toBeLessThanOrEqual(1);
          });
          // it ends where it started, so the next loop carries on from it
          expect(frames.at(-1)![property]).toBe(frames[0][property]);
        }
      }
    });
  }

  it("shows each drawing, one at a time, in PLATE_ENTRIES' order", () => {
    const t = k({ CYCLE_STYLE: "slide" });
    const s = step(t);
    const period = n * s.length;
    // mid-hold after change j, drawing j + 1 alone is fully shown
    for (let j = 0; j < n; j++) {
      const at = (j * s.length + s.end + t.CYCLE_HOLD / 2) / period;
      const shown = Array.from({ length: n }, (_, i) => {
        const frames = loopKeyframes(
          "opacity",
          period,
          i === 0 ? "1" : "0",
          layerChanges(t, i, n).opacity,
        );
        return frames.filter((f) => Number(f.offset) <= at).at(-1)!.opacity;
      });
      expect(shown).toEqual(Array.from({ length: n }, (_, i) => (i === (j + 1) % n ? "1" : "0")));
    }
  });
});
