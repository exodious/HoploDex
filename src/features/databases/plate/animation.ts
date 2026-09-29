import { STRAIGHTEDGE_TRAVEL, TONGUES } from "./PlateArt";
import type { CycleStyle, PlateTiming } from "./timing";

/*
 * The plate's timing: every element's animation, worked out from a
 * PlateTiming (timing.ts). The chooser and the tuner both use this, so they
 * can't drift apart.
 *
 * The draw-in is CSS animations, one per element (the keyframes are in
 * plate.css). The cycle that follows is Web Animations that repeat forever,
 * one loop through every drawing of entry 2. Both are plain animations with
 * delays, so the tuner can put the whole plate at any moment by setting
 * their currentTime.
 *
 * A click on the owl's beak makes it blink (blinkKeyframes), in real time.
 */

/** The rifle's and the owl's line work. */
const EASE_CURVE: [number, number, number, number] = [0.2, 0.7, 0.2, 1];
const EASE = `cubic-bezier(${EASE_CURVE.join(",")})`;

/** A cubic-bezier's progress at time `v`, or with `inverse`, the time at
 * which it reaches progress `v` (both 0..1). */
export function curve([x1, y1, x2, y2]: number[], v: number, inverse = false): number {
  const at = (u: number, a: number, b: number) =>
    3 * a * u * (1 - u) ** 2 + 3 * b * u * u * (1 - u) + u ** 3;
  const [ia, ib, oa, ob] = inverse ? [y1, y2, x1, x2] : [x1, x2, y1, y2];
  let lo = 0;
  let hi = 1;
  for (let i = 0; i < 40; i++) {
    const u = (lo + hi) / 2;
    if (at(u, ia, ib) < v) lo = u;
    else hi = u;
  }
  return at((lo + hi) / 2, oa, ob);
}

/** When the eased sweep has gone `frac` of the way round. */
const sweepTime = (k: PlateTiming, frac: number) =>
  k.RIM_START + k.RIM_TIME * curve(k.RIM_EASE, frac, true);

/** When eased line work looks finished (95% drawn), as a fraction of its time. */
export const looksDone = curve(EASE_CURVE, 0.95, true);

/** When the braid closes back at 12 o'clock; the owl counts from here. */
const braidClosed = (k: PlateTiming) => k.RIM_START + k.RIM_TIME + k.BRAID_LAG;
const owlStart = (k: PlateTiming) => braidClosed(k) + k.OWL_GAP;

export interface Phase {
  id: string;
  label: string;
  /** How the tuner's timeline colours it. */
  tone: "orn" | "line" | "fill" | "solid" | "ink" | "niter";
  /** [start, end] in unscaled seconds. */
  span: [number, number];
  /** Eased line work, which looks done well before its end. */
  eased?: boolean;
}

/** The draw-in's stages, in unscaled seconds. */
export function drawInPhases(k: PlateTiming): Phase[] {
  const owl = owlStart(k);
  const lastTongue = sweepTime(k, 1 - 1 / TONGUES) + k.BRAID_LAG + k.TONGUE_TIME;
  const fillStart = sweepTime(k, k.SHIELD_FILL_AT);
  const pupils = owl + k.OWL_LINES_TIME + k.PUPIL_PAUSE;
  return [
    { id: "key", label: "Greek key", tone: "orn", span: [k.KEY_START, k.KEY_START + k.KEY_TIME] },
    {
      id: "rifle",
      label: "Rifle",
      tone: "line",
      span: [k.RIFLE_START, k.RIFLE_START + k.RIFLE_TIME],
      eased: true,
    },
    {
      id: "rim",
      label: "Rim sweep",
      tone: "orn",
      span: [k.RIM_START, Math.max(braidClosed(k), lastTongue)],
    },
    {
      id: "ground",
      label: "Shield ground",
      tone: "fill",
      span: [fillStart, fillStart + k.SHIELD_FILL_TIME],
    },
    {
      id: "owl",
      label: "Owl's lines",
      tone: "orn",
      span: [owl, owl + k.OWL_LINES_TIME],
      eased: true,
    },
    {
      id: "beak",
      label: "Beak and berry",
      tone: "fill",
      span: [owl + k.BEAK_FILL_AFTER, owl + k.BEAK_FILL_AFTER + k.BEAK_FILL_TIME],
    },
    {
      id: "wash",
      label: "Bronze wash",
      tone: "fill",
      span: [owl + k.WASH_AFTER, owl + k.WASH_AFTER + k.WASH_TIME],
    },
    { id: "pupils", label: "Pupils", tone: "solid", span: [pupils, pupils + k.PUPIL_TIME] },
    {
      id: "labels",
      label: "Captions",
      tone: "ink",
      span: [k.LABELS_START, k.LABELS_START + k.LABELS_TIME],
    },
  ];
}

/** When the draw-in has finished, in unscaled seconds. */
export const drawInEnd = (k: PlateTiming) => Math.max(...drawInPhases(k).map((p) => p.span[1]));

// ── The cycle ────────────────────────────────────────────────────────────

/** One change of drawing, in seconds from its start (unscaled). */
export interface Step {
  /** The old drawing leaves over [0, outEnd]. */
  outEnd: number;
  /** The new one arrives over [inStart, inEnd]. */
  inStart: number;
  inEnd: number;
  /** The captions: the old fades out over [0, cap], the new in over
   * [capIn, capIn + cap]. */
  cap: number;
  capIn: number;
  /** When the change is over; the new drawing then holds for CYCLE_HOLD. */
  end: number;
  /** From one change's start to the next's. */
  length: number;
}

/** The straightedge wipes the old drawing away as it brings in the new, so
 * the two happen together, over CYCLE_IN_TIME. */
export function step(k: PlateTiming): Step {
  const together = k.CYCLE_STYLE === "straightedge";
  const outEnd = together ? k.CYCLE_IN_TIME : k.CYCLE_OUT_TIME;
  const inStart = together ? 0 : Math.max(0, k.CYCLE_OUT_TIME - k.CYCLE_OVERLAP);
  const inEnd = inStart + k.CYCLE_IN_TIME;
  const cap = k.CYCLE_CAPTION_TIME;
  const end = Math.max(outEnd, inEnd, 2 * cap);
  const capIn = Math.max(cap, end - cap);
  return { outEnd, inStart, inEnd, cap, capIn, end, length: end + k.CYCLE_HOLD };
}

/** When the cycle's first change starts, in unscaled seconds. */
export const cycleStart = (k: PlateTiming) => drawInEnd(k) + k.CYCLE_HOLD;

/** The cycle's changes as timeline stages, for the tuner: `count` of them
 * from the first. */
export function cyclePhases(k: PlateTiming, names: string[], count = names.length): Phase[] {
  if (!k.CYCLE || names.length < 2) return [];
  const s = step(k);
  const t0 = cycleStart(k);
  return Array.from({ length: count }, (_, j) => {
    const at = t0 + j * s.length;
    const to = names[(j + 1) % names.length];
    return {
      id: `cycle${j}`,
      label: `To the ${to.toLowerCase()}`,
      tone: "niter" as const,
      span: [at, at + s.end] as [number, number],
    };
  });
}

/** One property's value around the loop: `start` at its beginning, then
 * each change in turn. A change with no time is a jump. */
interface Change {
  at: number;
  time: number;
  to: string;
  easing?: string;
}

/** Keyframes for one property over a loop of `period`. */
export function loopKeyframes(
  property: string,
  period: number,
  start: string,
  changes: Change[],
): Keyframe[] {
  const frames: Keyframe[] = [{ offset: 0, [property]: start, easing: "linear" }];
  let value = start;
  for (const c of [...changes].sort((a, b) => a.at - b.at)) {
    frames.push({ offset: c.at / period, [property]: value, easing: c.easing ?? "linear" });
    frames.push({ offset: (c.at + c.time) / period, [property]: c.to, easing: "linear" });
    value = c.to;
  }
  frames.push({ offset: 1, [property]: value });
  return frames;
}

/** The lines' dash offsets for a drawing drawn (0), not yet drawn, or
 * erased away past its end. */
const DRAWN = "0";
const UNDRAWN = "1.01";
const ERASED = "-1.01";

interface LayerChanges {
  /** The art's opacity, transform, clip, and its lines' and parts' dashes and fills. */
  opacity: Change[];
  transform: Change[];
  clip: Change[];
  dash: Change[];
  fill: Change[];
  caption: Change[];
}

/** How layer `i` of `n` leaves and arrives around one loop. Change j (at
 * j × length) takes drawing j out and brings drawing j + 1 in, so layer i
 * leaves at change i and arrives at change i − 1 (layer 0, at the last). */
export function layerChanges(k: PlateTiming, i: number, n: number): LayerChanges {
  const s = step(k);
  const style: CycleStyle = k.CYCLE_STYLE;
  const out = i * s.length;
  const arrive = ((i - 1 + n) % n) * s.length;
  const slide = `${k.CYCLE_SLIDE}px`;
  const c: LayerChanges = { opacity: [], transform: [], clip: [], dash: [], fill: [], caption: [] };
  const inTime = s.inEnd - s.inStart;
  const outTime = s.outEnd;

  // leaving
  if (style === "slide" || style === "slideDraw") {
    c.opacity.push({ at: out, time: outTime, to: "0", easing: "ease-in" });
    c.transform.push({ at: out, time: outTime, to: `translateX(-${slide})`, easing: "ease-in" });
  } else if (style === "redraw") {
    c.opacity.push({ at: out, time: outTime, to: "0" });
  } else if (style === "erase") {
    c.dash.push({ at: out, time: outTime, to: ERASED, easing: "ease-in" });
    c.fill.push({ at: out, time: outTime * 0.6, to: "0" });
    c.opacity.push({ at: out + outTime, time: 0, to: "0" });
  } else {
    c.clip.push({
      at: out,
      time: outTime,
      to: `translateX(${STRAIGHTEDGE_TRAVEL}px)`,
      easing: "ease-in-out",
    });
    c.opacity.push({ at: out + outTime, time: 0, to: "0" });
  }
  c.caption.push({ at: out, time: s.cap, to: "0" });
  // gone, and hidden: put it back as it started, so the loop ends as it began
  const gone = out + outTime;
  c.transform.push({ at: gone, time: 0, to: "translateX(0px)" });
  if (style === "erase") {
    c.dash.push({ at: gone, time: 0, to: DRAWN });
    c.fill.push({ at: gone, time: 0, to: "1" });
  }
  if (style === "straightedge") c.clip.push({ at: gone, time: 0, to: "translateX(0px)" });

  // arriving
  const a = arrive + s.inStart;
  if (style === "slide") {
    c.transform.push({ at: a, time: 0, to: `translateX(${slide})` });
    c.transform.push({ at: a, time: inTime, to: "translateX(0px)", easing: "ease-out" });
    c.opacity.push({ at: a, time: inTime, to: "1" });
  } else if (style === "straightedge") {
    c.clip.push({ at: a, time: 0, to: `translateX(-${STRAIGHTEDGE_TRAVEL}px)` });
    c.clip.push({ at: a, time: inTime, to: "translateX(0px)", easing: "ease-in-out" });
    c.opacity.push({ at: a, time: 0, to: "1" });
  } else {
    // drawn in, line by line, like the rifle's first time
    c.dash.push({ at: a, time: 0, to: UNDRAWN });
    c.dash.push({ at: a, time: inTime, to: DRAWN, easing: EASE });
    c.fill.push({ at: a, time: 0, to: "0" });
    c.fill.push({ at: a + inTime * 0.25, time: inTime * 0.5, to: "1" });
    c.opacity.push({ at: a, time: 0, to: "1" });
    if (style === "slideDraw") {
      c.transform.push({ at: a, time: 0, to: `translateX(${slide})` });
      c.transform.push({ at: a, time: inTime, to: "translateX(0px)", easing: EASE });
    }
  }
  c.caption.push({ at: arrive + s.capIn, time: s.cap, to: "1" });
  return c;
}

/** The straightedge's travel across entry 2, at each change of the loop. */
function edgeChanges(k: PlateTiming, n: number): { opacity: Change[]; transform: Change[] } {
  const s = step(k);
  const opacity: Change[] = [];
  const transform: Change[] = [];
  const fade = Math.min(0.15, s.inEnd / 4);
  for (let j = 0; j < n; j++) {
    const at = j * s.length;
    transform.push({ at, time: 0, to: "translateX(0px)" });
    transform.push({
      at,
      time: s.inEnd,
      to: `translateX(${STRAIGHTEDGE_TRAVEL}px)`,
      easing: "ease-in-out",
    });
    opacity.push({ at, time: fade, to: "1" });
    opacity.push({ at: at + s.inEnd - fade, time: fade, to: "0" });
  }
  return { opacity, transform };
}

/** The first layer's lines and parts: the draw-in draws them. */
const LEAD = '.layer[data-layer="0"] .art';

// ── The blink ────────────────────────────────────────────────────────────

/** How long a blink takes, in unscaled seconds: the pupils go out, stay
 * out, then fill back in as they did in the draw-in. */
export const blinkTime = (k: PlateTiming) => k.BLINK_OUT_TIME + k.BLINK_WAIT + k.PUPIL_TIME;

/** The pupils' opacity through a blink. */
export function blinkKeyframes(k: PlateTiming): Keyframe[] {
  const total = blinkTime(k);
  return [
    { offset: 0, opacity: 1, easing: "linear" },
    { offset: k.BLINK_OUT_TIME / total, opacity: 0, easing: "linear" },
    // they fill back in eased, as in the draw-in: slow to start, then they open
    { offset: (k.BLINK_OUT_TIME + k.BLINK_WAIT) / total, opacity: 0, easing: "ease-in" },
    { offset: 1, opacity: 1 },
  ];
}

/** Makes a click on the owl's beak blink its eyes, once its pupils are
 * drawn and while it isn't already blinking. Returns a function that stops
 * listening, and stops a blink under way. */
function listenForBlink(svg: SVGSVGElement, k: PlateTiming): () => void {
  const beak = svg.querySelector<SVGElement>(".device .beak");
  const pupils = [...svg.querySelectorAll<SVGElement>(".device .pf")];
  if (!beak || !pupils.length) return () => {};
  let blink: Animation[] = [];
  const onClick = () => {
    if (typeof beak.animate !== "function") return;
    if (blink.some((a) => a.playState === "running")) return;
    // not yet drawn: the draw-in is still bringing them in
    if (pupils.some((p) => Number(getComputedStyle(p).opacity || "1") < 1)) return;
    const frames = blinkKeyframes(k);
    const duration = blinkTime(k) * k.SPEED * 1000;
    blink = pupils.map((p) => p.animate(frames, { duration }));
  };
  beak.addEventListener("click", onClick);
  return () => {
    beak.removeEventListener("click", onClick);
    blink.forEach((a) => a.cancel());
  };
}

/** Gives every element of the plate its animation, and returns a function
 * that stops them. */
export function applyPlateAnimation(svg: SVGSVGElement, k: PlateTiming): () => void {
  const s = (t: number) => `${(t * k.SPEED).toFixed(3)}s`;
  const set = (sel: string, value: string) =>
    svg.querySelectorAll<SVGElement>(sel).forEach((el) => {
      el.style.animation = value;
    });

  // 1. the Greek key, from both ends to the middle
  const units = [...svg.querySelectorAll<SVGElement>(".key.unit")];
  const far = Math.max(1, ...units.map((el) => Number(el.dataset.key)));
  const keyStep = (k.KEY_TIME - k.KEY_UNIT_TIME) / far;
  units.forEach((el) => {
    el.style.animation = `plate-draw ${s(k.KEY_UNIT_TIME)} ${s(k.KEY_START + Number(el.dataset.key) * keyStep)} linear forwards`;
  });
  set(".key:not(.unit)", `plate-draw ${s(k.KEY_TIME)} ${s(k.KEY_START)} linear forwards`);

  // 2. entry 2's first drawing
  const rifle = `plate-draw ${s(k.RIFLE_TIME)} ${s(k.RIFLE_START)} ${EASE} forwards`;
  set(`${LEAD} .open, ${LEAD} .detail`, rifle);
  set(`${LEAD} .part`, `${rifle}, plate-fill ${s(0.9)} ${s(k.RIFLE_START + 0.4)} linear forwards`);

  // 3. the rim: each piece starts as the sweep reaches it
  svg.querySelectorAll<SVGElement>("[data-sweep]").forEach((el) => {
    const [f0, f1, kind] = (el.dataset.sweep ?? "").split(" ");
    const lag = kind === "line" ? 0 : k.BRAID_LAG;
    const t0 = sweepTime(k, Number(f0)) + lag;
    if (kind === "bead")
      el.style.animation = `plate-fade ${s(k.BEAD_TIME)} ${s(t0)} linear forwards`;
    else if (kind === "tongue")
      el.style.animation = `plate-draw ${s(k.TONGUE_TIME)} ${s(t0)} linear forwards`;
    else
      el.style.animation = `plate-draw ${s(sweepTime(k, Number(f1)) - sweepTime(k, Number(f0)))} ${s(t0)} linear forwards`;
  });
  set(
    ".ground",
    `plate-fill ${s(k.SHIELD_FILL_TIME)} ${s(sweepTime(k, k.SHIELD_FILL_AT))} linear forwards`,
  );

  // 4. the owl, after the braid closes: lines, grounds, wash, then pupils
  const owl = owlStart(k);
  set(".device .ol, .device .fe", `plate-draw ${s(k.OWL_LINES_TIME)} ${s(owl)} ${EASE} forwards`);
  set(".device .paper", `plate-fill ${s(0.9)} ${s(owl + k.OWL_PAPER_AFTER)} linear forwards`);
  set(
    ".device .pfi",
    `plate-fade ${s(k.BEAK_FILL_TIME)} ${s(owl + k.BEAK_FILL_AFTER)} linear forwards`,
  );
  set(".device .wash", `plate-wash ${s(k.WASH_TIME)} ${s(owl + k.WASH_AFTER)} linear forwards`);
  // the pupils ease in: slow to start, then they open
  set(
    ".device .pf",
    `plate-fade ${s(k.PUPIL_TIME)} ${s(owl + k.OWL_LINES_TIME + k.PUPIL_PAUSE)} ease-in forwards`,
  );

  // 5. captions, scale bars and the rifle's centreline
  set(
    ".axis, text, rect:not(.clip)",
    `plate-fade ${s(k.LABELS_TIME)} ${s(k.LABELS_START)} linear forwards`,
  );
  svg.querySelectorAll<SVGElement>(".art .axis").forEach((el) => {
    el.style.display = k.BORE_AXIS ? "" : "none";
  });

  // 6. the cycle through entry 2's drawings, forever
  const stops: Animation[] = [];
  const stopBlink = listenForBlink(svg, k);
  const stop = () => {
    stopBlink();
    stops.forEach((a) => a.cancel());
  };
  const layers = [...svg.querySelectorAll<SVGGElement>(".entry .layer")];
  const n = layers.length;
  const clipped = k.CYCLE_STYLE === "straightedge";
  const clips = [...svg.querySelectorAll(".entry clipPath .clip")];
  layers.forEach((layer) => {
    const art = layer.querySelector<SVGGElement>(".art");
    if (art) {
      if (clipped && k.CYCLE) art.setAttribute("clip-path", art.dataset.clip ?? "");
      else art.removeAttribute("clip-path");
    }
  });
  if (!k.CYCLE || n < 2 || typeof svg.animate !== "function") return stop;
  const st = step(k);
  const period = n * st.length;
  const timing: KeyframeAnimationOptions = {
    duration: period * k.SPEED * 1000,
    delay: cycleStart(k) * k.SPEED * 1000,
    iterations: Infinity,
  };
  const play = (el: Element | null, frames: Keyframe[]) => {
    if (el) stops.push(el.animate(frames, timing));
  };
  layers.forEach((layer, i) => {
    const c = layerChanges(k, i, n);
    const shown = i === 0;
    const art = layer.querySelector(".art");
    play(art, loopKeyframes("opacity", period, shown ? "1" : "0", c.opacity));
    play(art, loopKeyframes("transform", period, "translateX(0px)", c.transform));
    play(
      layer.querySelector(".caption"),
      loopKeyframes("opacity", period, shown ? "1" : "0", c.caption),
    );
    if (clipped)
      play(clips[i] ?? null, loopKeyframes("transform", period, "translateX(0px)", c.clip));
    if (c.dash.length) {
      layer
        .querySelectorAll(".art .part, .art .open, .art .detail")
        .forEach((el) => play(el, loopKeyframes("strokeDashoffset", period, DRAWN, c.dash)));
    }
    if (c.fill.length) {
      layer
        .querySelectorAll(".art .part")
        .forEach((el) => play(el, loopKeyframes("fillOpacity", period, "1", c.fill)));
    }
  });
  if (clipped) {
    const edge = edgeChanges(k, n);
    const line = svg.querySelector(".entry .edge");
    play(line, loopKeyframes("opacity", period, "0", edge.opacity));
    play(line, loopKeyframes("transform", period, "translateX(0px)", edge.transform));
  }
  return stop;
}

/** How long the draw-in and one loop of the cycle take, in scaled seconds. */
export function duration(k: PlateTiming, layers: number): number {
  const loop = k.CYCLE && layers > 1 ? layers * step(k).length : 0;
  return k.SPEED * (loop ? cycleStart(k) + loop : drawInEnd(k));
}
