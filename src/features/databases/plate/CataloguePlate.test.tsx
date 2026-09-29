import { afterEach, describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/react";
import { CataloguePlate } from "./CataloguePlate";
import { cycleStart } from "./animation";
import { ENTRY_BOX, PLATE_ENTRIES, drawingBounds, entryLayout } from "./entries";
import { PLATE_TIMING } from "./timing";

function plate(timing = PLATE_TIMING) {
  const { container } = render(<CataloguePlate timing={timing} />);
  return container.querySelector("svg") as SVGSVGElement;
}

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

  it("has one layer in entry 2 for each drawing it cycles through, the first shown", () => {
    const layers = plate().querySelectorAll(".entry .layer");
    expect([...layers].map((l) => l.getAttribute("data-drawing"))).toEqual(
      PLATE_ENTRIES.map((e) => e.key),
    );
    expect(layers[0].textContent).toContain("Rifle.");
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

describe("entry 2's drawings", () => {
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
