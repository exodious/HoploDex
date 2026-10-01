import { afterEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render } from "@testing-library/react";
import { CataloguePlate } from "./CataloguePlate";
import { blinkKeyframes, blinkTime, cycleStart } from "./animation";
import {
  ENTRY_BOX,
  PLATE_ENTRIES,
  STARTUP_ENTRIES,
  drawingBounds,
  entryLayout,
  entryNumber,
  shuffled,
} from "./entries";
import type { PlateEntry } from "./entries";
import { PLATE_TIMING } from "./timing";
import { DRAWINGS } from "../../browse/typeDrawings";
import { ACCESSORY_KINDS, FIREARM_TYPES } from "../../../test/collectionFixtures";

function plate(timing = PLATE_TIMING, entries: readonly PlateEntry[] = PLATE_ENTRIES) {
  const { container } = render(<CataloguePlate timing={timing} entries={entries} />);
  return container.querySelector("svg") as SVGSVGElement;
}

const layerDrawings = (svg: SVGSVGElement) =>
  [...svg.querySelectorAll(".entry .layer")].map((l) => l.getAttribute("data-drawing"));

/** The seconds in an element's `animation` shorthand: [duration, delay]. */
function times(el: Element): number[] {
  return [...((el as SVGElement).style.animation.matchAll(/([\d.]+)s/g) ?? [])].map((m) =>
    Number(m[1]),
  );
}

/** Element.animate, which jsdom lacks, recording what the cycle asks of it. */
function mockAnimate() {
  const animate = vi.fn<
    (
      keyframes: Keyframe[] | PropertyIndexedKeyframes | null,
      options?: KeyframeAnimationOptions,
    ) => Animation
  >(() => ({ cancel: vi.fn() }) as unknown as Animation);
  Element.prototype.animate = animate as unknown as Element["animate"];
  return animate;
}

function reduceMotion(reduce: boolean) {
  vi.stubGlobal("matchMedia", (query: string) => ({ matches: reduce && query.includes("reduce") }));
}

afterEach(() => {
  vi.unstubAllGlobals();
  delete (Element.prototype as { animate?: unknown }).animate;
});

describe("CataloguePlate", () => {
  it("is decoration only, hidden from assistive technology", () => {
    expect(plate().getAttribute("aria-hidden")).toBe("true");
  });

  it("has one layer in entry 2 for each drawing it cycles through, in the order given", () => {
    const order = [
      PLATE_ENTRIES[2],
      PLATE_ENTRIES[0],
      PLATE_ENTRIES[4],
      PLATE_ENTRIES[3],
      PLATE_ENTRIES[1],
    ];
    const svg = plate(PLATE_TIMING, order);
    expect(layerDrawings(svg)).toEqual(order.map((e) => e.key));
    expect(svg.querySelector('.layer[data-layer="0"]')!.textContent).toContain("Shotgun.");
  });

  it("shows the drawings in the order shuffled at startup, unless given one", () => {
    const { container } = render(<CataloguePlate />);
    expect(layerDrawings(container.querySelector("svg")!)).toEqual(
      STARTUP_ENTRIES.map((e) => e.key),
    );
  });

  it("numbers each drawing by its place in PLATE_ENTRIES, whatever the order", () => {
    const order = [...PLATE_ENTRIES].reverse();
    const numbers = [...plate(PLATE_TIMING, order).querySelectorAll(".entry .layer")].map((l) => [
      l.getAttribute("data-drawing"),
      l.querySelector(".caption .no")!.textContent,
    ]);
    expect(numbers).toEqual([
      ["other", "6"],
      ["suppressor", "5"],
      ["shotgun", "4"],
      ["handgun", "3"],
      ["rifle", "2"],
    ]);
  });

  it("draws in with timing.ts's times, scaled by SPEED", () => {
    const k = { ...PLATE_TIMING, SPEED: 2 };
    const svg = plate(k);
    expect(svg.classList.contains("hd-catalogue--animate")).toBe(true);
    // the rifle, in the first layer only
    const rifle = svg.querySelector('.layer[data-layer="0"] .art .open')!;
    expect(times(rifle).slice(0, 2)).toEqual([2 * k.RIFLE_TIME, 2 * k.RIFLE_START]);
    expect(
      (svg.querySelector('.layer[data-layer="1"] .art .open') as SVGElement).style.animation,
    ).toBe("");
    // the pupils, last
    const owl = k.RIM_START + k.RIM_TIME + k.BRAID_LAG + k.OWL_GAP;
    const [, pupilDelay] = times(svg.querySelector(".device .pf")!);
    expect(pupilDelay).toBeCloseTo(2 * (owl + k.OWL_LINES_TIME + k.PUPIL_PAUSE), 2);
    // every piece of the rim
    for (const piece of svg.querySelectorAll("[data-sweep]")) {
      expect((piece as SVGElement).style.animation).not.toBe("");
    }
  });

  it("hides the bore axis unless BORE_AXIS is on", () => {
    const axis = (k: typeof PLATE_TIMING) =>
      (plate(k).querySelector(".art .axis") as SVGElement).style.display;
    expect(axis({ ...PLATE_TIMING, BORE_AXIS: false })).toBe("none");
    expect(axis({ ...PLATE_TIMING, BORE_AXIS: true })).toBe("");
  });

  it("starts the cycle once the draw-in has held for CYCLE_HOLD, and repeats it forever", () => {
    const animate = mockAnimate();
    plate();
    expect(animate).toHaveBeenCalled();
    const options = animate.mock.calls[0][1]!;
    expect(options.iterations).toBe(Infinity);
    expect(options.delay).toBeCloseTo(cycleStart(PLATE_TIMING) * PLATE_TIMING.SPEED * 1000);
  });

  it("keeps the first drawing when CYCLE is off", () => {
    const animate = mockAnimate();
    plate({ ...PLATE_TIMING, CYCLE: false });
    expect(animate).not.toHaveBeenCalled();
  });

  it("shows the finished plate, still, when the viewer asks for less motion", () => {
    reduceMotion(true);
    const animate = mockAnimate();
    const svg = plate();
    expect(svg.classList.contains("hd-catalogue--animate")).toBe(false);
    expect(
      [...svg.querySelectorAll<SVGElement>("path, circle, text")].every(
        (el) => !el.style.animation,
      ),
    ).toBe(true);
    expect(animate).not.toHaveBeenCalled();
  });
});

describe("the owl's blink", () => {
  const k = { ...PLATE_TIMING, SPEED: 2, BLINK_OUT_TIME: 0.1, BLINK_WAIT: 0.5, PUPIL_TIME: 0.4 };
  const pupilCalls = (animate: ReturnType<typeof mockAnimate>) =>
    animate.mock.contexts.filter((el) => (el as Element).classList.contains("pf"));

  it("puts the pupils out and back in when the beak is clicked", () => {
    const animate = mockAnimate();
    const svg = plate(k);
    expect(pupilCalls(animate)).toHaveLength(0);
    fireEvent.click(svg.querySelector(".device .beak")!);
    const pupils = svg.querySelectorAll(".device .pf");
    expect(pupilCalls(animate)).toEqual([...pupils]);
    const [frames, options] = animate.mock.calls.at(-1)!;
    expect(frames).toEqual(blinkKeyframes(k));
    expect(options).toEqual({ duration: 2 * 1000 * (0.1 + 0.5 + 0.4) });
  });

  it("goes out over BLINK_OUT_TIME, stays out for BLINK_WAIT, then fills over PUPIL_TIME", () => {
    expect(blinkTime(k)).toBeCloseTo(1);
    expect(blinkKeyframes(k).map((f) => [f.offset, f.opacity])).toEqual([
      [0, 1],
      [0.1, 0],
      [0.6, 0],
      [1, 1],
    ]);
    expect(blinkKeyframes(k)[2].easing).toBe("ease-in");
  });

  it("doesn't blink again while it is blinking", () => {
    const animate = mockAnimate();
    animate.mockImplementation(
      () => ({ cancel: vi.fn(), playState: "running" }) as unknown as Animation,
    );
    const beak = plate(k).querySelector(".device .beak")!;
    fireEvent.click(beak);
    fireEvent.click(beak);
    expect(pupilCalls(animate)).toHaveLength(2);
  });

  it("waits for the draw-in to bring the pupils in", () => {
    const animate = mockAnimate();
    const svg = plate(k);
    svg.querySelectorAll<SVGElement>(".device .pf").forEach((p) => {
      p.style.opacity = "0.4";
    });
    fireEvent.click(svg.querySelector(".device .beak")!);
    expect(pupilCalls(animate)).toHaveLength(0);
  });

  it("stops listening once the plate goes", () => {
    const animate = mockAnimate();
    const { container, unmount } = render(<CataloguePlate timing={k} />);
    const beak = container.querySelector(".device .beak")!;
    unmount();
    fireEvent.click(beak);
    expect(pupilCalls(animate)).toHaveLength(0);
  });
});

describe("entry 2's order", () => {
  it("is shuffled: every drawing once, in the order the random draws give", () => {
    // Fisher–Yates from the end: 0.99 leaves the last where it is, then 0
    // swaps the fourth with the first, the third with the first, then the
    // second with the first
    const draws = [0.99, 0, 0, 0];
    const order = shuffled(PLATE_ENTRIES, () => draws.shift()!);
    expect(order.map((e) => e.key)).toEqual(["handgun", "shotgun", "suppressor", "rifle", "other"]);
    expect(PLATE_ENTRIES.map((e) => e.key)).toEqual([
      "rifle",
      "handgun",
      "shotgun",
      "suppressor",
      "other",
    ]);
  });

  it("keeps every drawing, once, at startup", () => {
    expect(STARTUP_ENTRIES.map((e) => e.key).sort()).toEqual(
      PLATE_ENTRIES.map((e) => e.key).sort(),
    );
  });

  it("gives each drawing its own number, after the hoplon's 1", () => {
    expect(PLATE_ENTRIES.map(entryNumber)).toEqual([2, 3, 4, 5, 6]);
  });
});

describe("entry 2's drawings", () => {
  it("include every firearm type drawing, and no accessory kind's (the plate is of firearms)", () => {
    const firearmKeys = FIREARM_TYPES.map((t) => t.genericThumbnailKey).sort();
    expect(PLATE_ENTRIES.map((e) => e.key).sort()).toEqual(firearmKeys);
    // 006 added a drawing per accessory kind to DRAWINGS; none belongs here.
    for (const kind of ACCESSORY_KINDS) {
      if (kind.genericThumbnailKey in DRAWINGS && !firearmKeys.includes(kind.genericThumbnailKey)) {
        expect(PLATE_ENTRIES.map((e) => e.key)).not.toContain(kind.genericThumbnailKey);
      }
    }
  });

  it("have bounds read through smooth (S) curves", () => {
    // The suppressor's break lines are S curves, all inside its tube.
    expect(drawingBounds("suppressor")).toEqual([27, 80.5, 287, 119.5]);
  });

  it("each fit the entry's box", () => {
    for (const entry of PLATE_ENTRIES) {
      const [x0, y0, x1, y1] = drawingBounds(entry.key);
      const { scale } = entryLayout(entry);
      expect((x1 - x0) * scale).toBeLessThanOrEqual(ENTRY_BOX.width + 0.01);
      expect((y1 - y0) * scale).toBeLessThanOrEqual(ENTRY_BOX.height + 0.01);
    }
  });

  it("have scale bars true to their real sizes: the rifle's 30 cm is its width × 300 / 986 mm", () => {
    const rifle = PLATE_ENTRIES.find((e) => e.key === "rifle")!;
    const [x0, , x1] = drawingBounds("rifle");
    const { scale, barPx } = entryLayout(rifle);
    expect(scale).toBe(1.6);
    expect(barPx).toBeCloseTo(((x1 - x0) * scale * 300) / 986);
  });
});
