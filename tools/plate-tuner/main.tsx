import { renderToStaticMarkup } from "react-dom/server";
import "@fontsource-variable/big-shoulders-display";
import "@fontsource-variable/atkinson-hyperlegible-next";
import "@fontsource-variable/atkinson-hyperlegible-mono";
import "../../src/styles/tokens.css";
import "../../src/features/databases/plate/plate.css";
import {
  applyPlateAnimation,
  cyclePhases,
  cycleStart,
  drawInEnd,
  drawInPhases,
  duration,
  looksDone,
} from "../../src/features/databases/plate/animation";
import type { Phase } from "../../src/features/databases/plate/animation";
import { PLATE_ENTRIES } from "../../src/features/databases/plate/entries";
import { PLATE_VIEWBOX, PlateArt } from "../../src/features/databases/plate/PlateArt";
import { PLATE_TIMING } from "../../src/features/databases/plate/timing";
import type { PlateTiming } from "../../src/features/databases/plate/timing";
import { EASES, GROUPS, PRESETS, STYLES } from "./knobs";
import type { Knob } from "./knobs";
import { literal } from "./saveTiming";
import "./tuner.css";

/*
 * The tuner: the chooser plate's animations driven by a clock of our own,
 * so they can be paused, scrubbed and slowed, and rebuilt whenever a knob
 * moves. It plays the app's own plate and timing code.
 */

type Values = PlateTiming;
type Name = keyof PlateTiming;

const D: Values = PLATE_TIMING;
const $ = <T extends HTMLElement = HTMLElement>(sel: string) => document.querySelector<T>(sel) as T;
const stage = $("#stage");
const scrub = $<HTMLInputElement>("#scrub");
const tl = $("#tl");
const toggle = $("#toggle");
const SAVE = "hoplodex-plate-tuner";
const LAYERS = PLATE_ENTRIES.length;
const pristine = renderToStaticMarkup(
  <svg
    className="hd-catalogue hd-catalogue--animate"
    viewBox={PLATE_VIEWBOX}
    role="img"
    aria-label="The chooser’s catalogue plate: the hoplon with its owl, the Greek key and the firearm drawings"
  >
    <PlateArt clipId="tuner" entries={PLATE_ENTRIES} />
  </svg>,
);

interface Saved {
  knobs?: Partial<Values>;
  theme?: string;
  view?: View;
}
function load(): Saved {
  try {
    return JSON.parse(localStorage.getItem(SAVE) ?? "{}") as Saved;
  } catch {
    return {};
  }
}
function store(saved: Saved) {
  try {
    localStorage.setItem(SAVE, JSON.stringify({ ...load(), ...saved }));
  } catch {
    // no storage: it just won't remember
  }
}

let k: Values = { ...D, ...(load().knobs ?? {}) };
type View = "drawIn" | "cycle";
let view: View = load().view ?? "drawIn";

let anims: Animation[] = [];
let total = 1;
let rate = 1;
let playing = false;
let at = 0; // seconds shown when the clock last changed
let since = 0; // when that was

const now = () =>
  playing ? Math.min(total, at + ((performance.now() - since) / 1000) * rate) : at;

/** How long the timeline runs in this view, in scaled seconds. */
function viewTotal(): number {
  if (view === "cycle" && k.CYCLE) return duration(k, LAYERS) + 0.3;
  return drawInEnd(k) * k.SPEED + 0.3;
}

/** Puts every animation at `t` seconds, running the ones not yet done if we're playing. */
function show(t: number) {
  at = t;
  since = performance.now();
  const ms = t * 1000;
  for (const a of anims) {
    a.playbackRate = rate;
    a.currentTime = ms;
    const end = Number(a.effect?.getComputedTiming().endTime ?? 0);
    if (playing && ms < end) a.play();
    else a.pause();
  }
}

function rebuild(t: number) {
  stage.innerHTML = pristine;
  const svg = stage.querySelector("svg") as SVGSVGElement;
  applyPlateAnimation(svg, k);
  anims = svg.getAnimations({ subtree: true });
  total = viewTotal();
  scrub.max = total.toFixed(2);
  drawTimeline();
  show(Math.min(t, total));
}

function setPlaying(p: boolean) {
  const t = now();
  playing = p;
  if (playing && t >= total - 0.01) show(0);
  else show(t);
  toggle.textContent = playing ? "Pause" : "Play";
}

// ── the timeline ──
function phases(): Phase[] {
  if (view === "drawIn" || !k.CYCLE) return drawInPhases(k);
  const names = PLATE_ENTRIES.map((e) => e.title.replace(/\.$/, ""));
  return [
    { id: "drawIn", label: "The draw-in", tone: "line", span: [0, drawInEnd(k)] },
    ...cyclePhases(k, names),
  ];
}

function drawTimeline() {
  const x = (t: number) => `${(t / total) * 100}%`;
  const tick = total > 30 ? 5 : total > 12 ? 2 : total > 4 ? 1 : 0.5;
  tl.style.setProperty("--tick-gap", x(tick));
  tl.innerHTML =
    phases()
      .map((p) => {
        const [a, b] = p.span.map((v) => v * k.SPEED);
        const eased = p.eased ? ` eased" style="--done:${looksDone * 100}%;` : `" style="`;
        return (
          `<div class="tl-row"><span>${p.label}</span><div class="tl-track">` +
          `<span class="bar ${p.tone}${eased}left:${x(a)};width:${x(Math.max(0, b - a))}" title="${a.toFixed(2)}–${b.toFixed(2)} s"></span></div></div>`
        );
      })
      .join("") + '<span class="playhead" id="playhead"></span>';
  let ticks = "";
  for (let t = 0; t <= total + 1e-6; t += tick) {
    ticks += `<span style="left:${x(t)}">${t % 1 ? t.toFixed(1) : t}</span>`;
  }
  $("#ticks").innerHTML = ticks;
}

function frame() {
  const t = now();
  if (playing && t >= total) setPlaying(false);
  $("#clock").innerHTML = `<b>${t.toFixed(2)} s</b> of ${total.toFixed(2)} s`;
  scrub.value = t.toFixed(2);
  $("#playhead")?.style.setProperty("--p", String(Math.min(1, t / total)));
  requestAnimationFrame(frame);
}

// Dragging on the timeline scrubs it.
function scrubTo(e: PointerEvent) {
  const track = (tl.querySelector(".tl-track") as HTMLElement).getBoundingClientRect();
  const t = Math.max(0, Math.min(total, ((e.clientX - track.left) / track.width) * total));
  if (playing) setPlaying(false);
  show(t);
}
tl.addEventListener("pointerdown", (e) => {
  tl.setPointerCapture(e.pointerId);
  scrubTo(e);
});
tl.addEventListener("pointermove", (e) => {
  if (tl.hasPointerCapture(e.pointerId)) scrubTo(e);
});
tl.addEventListener("keydown", (e) => {
  const d = ({ ArrowLeft: -0.1, ArrowRight: 0.1 } as Record<string, number>)[e.key];
  if (d === undefined) return;
  e.preventDefault();
  if (playing) setPlaying(false);
  show(Math.max(0, Math.min(total, now() + d * (e.shiftKey ? 10 : 1))));
});
scrub.addEventListener("input", () => {
  if (playing) setPlaying(false);
  show(Number(scrub.value));
});

toggle.addEventListener("click", () => setPlaying(!playing));
$("#replay").addEventListener("click", () => {
  playing = true;
  toggle.textContent = "Pause";
  show(0);
});
document.querySelectorAll<HTMLButtonElement>("[data-rate]").forEach((b) =>
  b.addEventListener("click", () => {
    rate = Number(b.dataset.rate);
    document
      .querySelectorAll("[data-rate]")
      .forEach((o) => o.setAttribute("aria-pressed", String(o === b)));
    show(now());
  }),
);

function setView(v: View) {
  view = v;
  store({ view });
  document.querySelectorAll("[data-view]").forEach((b) => {
    b.setAttribute("aria-pressed", String((b as HTMLElement).dataset.view === v));
  });
  rebuild(now());
}
document
  .querySelectorAll<HTMLButtonElement>("[data-view]")
  .forEach((b) => b.addEventListener("click", () => setView(b.dataset.view as View)));

/** Plays from just before the first change of drawing. */
function playFirstChange() {
  if (!k.CYCLE) {
    k = { ...k, CYCLE: true };
    syncControls();
  }
  view = "cycle";
  setView("cycle");
  playing = true;
  toggle.textContent = "Pause";
  show(Math.max(0, cycleStart(k) * k.SPEED - 1));
}
$("#to-cycle").addEventListener("click", playFirstChange);

// ── the knobs ──
const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);
const easeIndex = (e: Values["RIM_EASE"]) =>
  Math.max(
    0,
    EASES.findIndex(([, v]) => same(v, e)),
  );

function fmt(knob: Knob, v: number): string {
  if (knob.kind !== "range") return "";
  if (knob.unit === "%") return `${Math.round(v * 100)}%`;
  if (knob.unit === "×") return `${v.toFixed(2)}×`;
  if (knob.unit === "px") return `${Math.round(v)} px`;
  return `${v >= 0 ? "" : "−"}${Math.abs(v).toFixed(2)} s`;
}

const KNOBS = new Map(
  GROUPS.flatMap(([, knobs]) => knobs.map((knob) => [knob.name, knob] as const)),
);

function control(knob: Knob): string {
  const hint = knob.hint ? `<p class="hint" id="${knob.name}-hint">${knob.hint}</p>` : "";
  const described = knob.hint ? ` aria-describedby="${knob.name}-hint"` : "";
  const foot = `<div class="knob-foot"><code>${knob.name}</code>${hint}</div>`;
  if (knob.kind === "switch") {
    return (
      `<div class="knob"><label class="switch" for="${knob.name}"><input type="checkbox" id="${knob.name}" data-knob="${knob.name}"${described}>` +
      `<span>${knob.label}</span></label>${foot}</div>`
    );
  }
  let field: string;
  let out = "";
  if (knob.kind === "ease" || knob.kind === "style") {
    const options = (
      knob.kind === "ease" ? EASES.map(([text]) => text) : STYLES.map(([, text]) => text)
    )
      .map((text, i) => `<option value="${i}">${text}</option>`)
      .join("");
    field = `<select id="${knob.name}" data-knob="${knob.name}"${described}>${options}</select>`;
  } else {
    field = `<input type="range" id="${knob.name}" data-knob="${knob.name}" min="${knob.min}" max="${knob.max}" step="${knob.step}"${described}>`;
    out = `<output for="${knob.name}" id="${knob.name}-out"></output>`;
  }
  return `<div class="knob"><div class="knob-head"><label for="${knob.name}">${knob.label}</label>${out}</div>${field}${foot}</div>`;
}

$("#controls").innerHTML = GROUPS.map(
  ([title, knobs]) =>
    `<fieldset><legend>${title}</legend>${knobs.map(control).join("")}</fieldset>`,
).join("");
$("#presets").innerHTML = PRESETS.map(
  ([name, note], i) =>
    `<button type="button" class="preset" data-preset="${i}" aria-pressed="false"><b>${name}</b><span>${note}</span></button>`,
).join("");

function changed(): Name[] {
  return (Object.keys(D) as Name[]).filter((n) => !same(k[n], D[n]));
}

function syncControls() {
  document.querySelectorAll<HTMLInputElement | HTMLSelectElement>("[data-knob]").forEach((el) => {
    const name = el.dataset.knob as Name;
    const knob = KNOBS.get(name) as Knob;
    if (knob.kind === "ease") el.value = String(easeIndex(k.RIM_EASE));
    else if (knob.kind === "style")
      el.value = String(STYLES.findIndex(([s]) => s === k.CYCLE_STYLE));
    else if (knob.kind === "switch") (el as HTMLInputElement).checked = Boolean(k[name]);
    else {
      el.value = String(k[name]);
      $(`#${name}-out`).textContent = fmt(knob, Number(k[name]));
    }
    el.closest(".knob")?.classList.toggle("changed", !same(k[name], D[name]));
  });
  document.querySelectorAll<HTMLButtonElement>(".preset").forEach((b) => {
    const set = PRESETS[Number(b.dataset.preset)][2];
    b.setAttribute(
      "aria-pressed",
      String((Object.keys(set) as Name[]).every((n) => same(k[n], set[n]))),
    );
  });
  const names = changed();
  $("#changes").innerHTML = names.length
    ? `<pre id="changes-text">${names.map((n) => `${n}: ${literal(k[n])},`).join("\n")}</pre>`
    : '<p class="empty">Nothing changed yet: these are timing.ts’s values.</p>';
  $<HTMLButtonElement>("#copy").disabled = !names.length;
  $<HTMLButtonElement>("#save").disabled = !names.length;
  store({ knobs: k });
}

$("#controls").addEventListener("input", (e) => {
  const el = e.target as HTMLInputElement | HTMLSelectElement;
  const name = el.dataset.knob as Name | undefined;
  if (!name) return;
  const knob = KNOBS.get(name) as Knob;
  const value =
    knob.kind === "ease"
      ? EASES[Number(el.value)][1]
      : knob.kind === "style"
        ? STYLES[Number(el.value)][0]
        : knob.kind === "switch"
          ? (el as HTMLInputElement).checked
          : Number(el.value);
  k = { ...k, [name]: value };
  syncControls();
  if (!playing) rebuild(at);
  else drawTimeline();
});
// Playing, the drawing starts again once a slider is let go.
$("#controls").addEventListener("change", () => {
  if (playing) rebuild(view === "cycle" && k.CYCLE ? Math.max(0, cycleStart(k) * k.SPEED - 1) : 0);
});

document.querySelectorAll<HTMLButtonElement>(".preset").forEach((b) =>
  b.addEventListener("click", () => {
    k = { ...k, ...PRESETS[Number(b.dataset.preset)][2] };
    syncControls();
    rebuild(0);
    playFirstChange();
  }),
);
$("#reset").addEventListener("click", () => {
  k = { ...D };
  syncControls();
  rebuild(playing ? 0 : at);
});
$("#copy").addEventListener("click", async () => {
  const text = changed()
    .map((n) => `${n}: ${literal(k[n])},`)
    .join("\n");
  const btn = $("#copy");
  try {
    await navigator.clipboard.writeText(text);
    btn.textContent = "Copied";
  } catch {
    getSelection()?.selectAllChildren($("#changes-text"));
    btn.textContent = "Selected";
  }
  setTimeout(() => {
    btn.textContent = "Copy";
  }, 1600);
});

// Served by `npm run tuner`, the tuner can write its changes into timing.ts.
// The dev server then reloads the page, now with them as its values.
if (import.meta.env.DEV) {
  const save = $<HTMLButtonElement>("#save");
  save.hidden = false;
  save.addEventListener("click", async () => {
    const values = Object.fromEntries(changed().map((n) => [n, k[n]]));
    const response = await fetch("/__plate-timing", {
      method: "POST",
      body: JSON.stringify(values),
    }).catch(() => null);
    $("#saved").textContent = response?.ok
      ? "Saved to timing.ts."
      : `timing.ts wasn't saved: ${response ? await response.text() : "the tuner's server isn't running"}.`;
  });
}

// ── the colours ──
function setTheme(theme: string) {
  document.documentElement.dataset.theme = theme;
  document.querySelectorAll("[data-plate]").forEach((b) => {
    b.setAttribute("aria-pressed", String((b as HTMLElement).dataset.plate === theme));
  });
  store({ theme });
}
document
  .querySelectorAll<HTMLButtonElement>("button[data-plate]")
  .forEach((b) => b.addEventListener("click", () => setTheme(b.dataset.plate as string)));
setTheme(load().theme ?? (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light"));

// The tuner is for watching the animation, so it animates under reduced
// motion too; it just doesn't start playing on its own.
const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
// A link can open it on a preset (?preset=2, counting from 1) and paused at
// a moment (?at=5.2, in seconds, with ?view=cycle for one in the loop).
const link = new URLSearchParams(location.search);
const preset = PRESETS[Number(link.get("preset")) - 1];
if (preset) k = { ...k, ...preset[2] };
if (link.get("view") === "cycle" || link.get("view") === "drawIn") view = link.get("view") as View;
syncControls();
setView(view);
rebuild(0);
if (link.has("at")) show(Math.min(total, Number(link.get("at"))));
else if (reduced) {
  show(total);
  toggle.textContent = "Play";
} else setPlaying(true);
requestAnimationFrame(frame);
