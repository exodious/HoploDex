import type { CycleStyle, PlateTiming } from "../../src/features/databases/plate/timing";

/*
 * The tuner's sliders, grouped in the order things draw, and its presets.
 * Every knob is one of PlateTiming's (src/features/databases/plate/timing.ts).
 */

export type Knob =
  | {
      name: keyof PlateTiming;
      label: string;
      kind: "range";
      min: number;
      max: number;
      step: number;
      /** "s" is seconds; "×", "%" and "px" are shown as such. */
      unit: "s" | "×" | "%" | "px";
      hint?: string;
    }
  | { name: keyof PlateTiming; label: string; kind: "switch"; hint?: string }
  | { name: keyof PlateTiming; label: string; kind: "ease"; hint?: string }
  | { name: keyof PlateTiming; label: string; kind: "style"; hint?: string };

export const GROUPS: [string, Knob[]][] = [
  [
    "Overall",
    [
      {
        name: "SPEED",
        label: "Overall pace",
        kind: "range",
        min: 0.3,
        max: 2.5,
        step: 0.05,
        unit: "×",
        hint: "Multiplies every time below. Higher is slower.",
      },
    ],
  ],
  [
    "Greek key",
    [
      {
        name: "KEY_START",
        label: "Starts at",
        kind: "range",
        min: 0,
        max: 5,
        step: 0.05,
        unit: "s",
      },
      {
        name: "KEY_TIME",
        label: "Whole rule",
        kind: "range",
        min: 0.4,
        max: 3,
        step: 0.05,
        unit: "s",
        hint: "It draws from both ends and meets in the middle.",
      },
      {
        name: "KEY_UNIT_TIME",
        label: "Each key unit",
        kind: "range",
        min: 0.1,
        max: 1.2,
        step: 0.05,
        unit: "s",
      },
    ],
  ],
  [
    "Rifle",
    [
      {
        name: "RIFLE_START",
        label: "Starts at",
        kind: "range",
        min: 0,
        max: 5,
        step: 0.05,
        unit: "s",
      },
      {
        name: "RIFLE_TIME",
        label: "Draws over",
        kind: "range",
        min: 0.4,
        max: 4,
        step: 0.05,
        unit: "s",
        hint: "Eased, so it looks finished early. The timeline shades the tail.",
      },
      {
        name: "BORE_AXIS",
        label: "Bore axis line",
        kind: "switch",
        hint: "The dashed centreline along the barrel. It fades in with the captions.",
      },
    ],
  ],
  [
    "Shield rim",
    [
      {
        name: "RIM_START",
        label: "Sweep starts at",
        kind: "range",
        min: 0,
        max: 5,
        step: 0.05,
        unit: "s",
      },
      {
        name: "RIM_TIME",
        label: "Once round",
        kind: "range",
        min: 0.5,
        max: 5,
        step: 0.05,
        unit: "s",
      },
      { name: "RIM_EASE", label: "Sweep pace", kind: "ease" },
      {
        name: "BRAID_LAG",
        label: "Braid behind the circles",
        kind: "range",
        min: -0.5,
        max: 0.5,
        step: 0.02,
        unit: "s",
        hint: "The tongues and beads keep to the braid.",
      },
      {
        name: "TONGUE_TIME",
        label: "Each tongue draws over",
        kind: "range",
        min: 0.05,
        max: 1.5,
        step: 0.05,
        unit: "s",
      },
      {
        name: "BEAD_TIME",
        label: "Each bead fades in over",
        kind: "range",
        min: 0.05,
        max: 1,
        step: 0.05,
        unit: "s",
      },
      {
        name: "SHIELD_FILL_AT",
        label: "Ground fades in from",
        kind: "range",
        min: 0,
        max: 1,
        step: 0.05,
        unit: "%",
        hint: "How far round the sweep is. Earlier hides the grid ahead of the lines.",
      },
      {
        name: "SHIELD_FILL_TIME",
        label: "Ground fades in over",
        kind: "range",
        min: 0.1,
        max: 2,
        step: 0.05,
        unit: "s",
      },
    ],
  ],
  [
    "Owl",
    [
      {
        name: "OWL_GAP",
        label: "Starts after the braid closes",
        kind: "range",
        min: -3,
        max: 1.5,
        step: 0.05,
        unit: "s",
        hint: "Negative starts the owl while the rim is still drawing.",
      },
      {
        name: "OWL_LINES_TIME",
        label: "Lines draw over",
        kind: "range",
        min: 0.5,
        max: 4,
        step: 0.05,
        unit: "s",
        hint: "Eased like the rifle.",
      },
      {
        name: "OWL_PAPER_AFTER",
        label: "Head’s ground, after its lines start",
        kind: "range",
        min: 0,
        max: 3,
        step: 0.05,
        unit: "s",
        hint: "Hides the body’s lines behind the head.",
      },
      {
        name: "BEAK_FILL_AFTER",
        label: "Beak and berry fill, after its lines start",
        kind: "range",
        min: 0,
        max: 4,
        step: 0.05,
        unit: "s",
      },
      {
        name: "BEAK_FILL_TIME",
        label: "Beak and berry fill over",
        kind: "range",
        min: 0.1,
        max: 2,
        step: 0.05,
        unit: "s",
      },
      {
        name: "WASH_AFTER",
        label: "Bronze wash, after its lines start",
        kind: "range",
        min: 0,
        max: 4,
        step: 0.05,
        unit: "s",
      },
      {
        name: "WASH_TIME",
        label: "Wash fades in over",
        kind: "range",
        min: 0.1,
        max: 3,
        step: 0.05,
        unit: "s",
      },
      {
        name: "PUPIL_PAUSE",
        label: "Pause before the pupils",
        kind: "range",
        min: 0,
        max: 2.5,
        step: 0.05,
        unit: "s",
        hint: "Counted from the end of the lines’ time.",
      },
      {
        name: "PUPIL_TIME",
        label: "Pupils fill over",
        kind: "range",
        min: 0.05,
        max: 2,
        step: 0.05,
        unit: "s",
        hint: "Smaller is quicker.",
      },
      {
        name: "BLINK_OUT_TIME",
        label: "Blink: pupils go out over",
        kind: "range",
        min: 0.02,
        max: 1,
        step: 0.02,
        unit: "s",
        hint: "Click the owl’s beak, once its pupils are in, to watch a blink. They fill back in over the pupils’ time above.",
      },
      {
        name: "BLINK_WAIT",
        label: "Blink: pupils stay out for",
        kind: "range",
        min: 0,
        max: 3,
        step: 0.05,
        unit: "s",
      },
    ],
  ],
  [
    "Captions and scale bars",
    [
      {
        name: "LABELS_START",
        label: "Fade in at",
        kind: "range",
        min: 0,
        max: 7,
        step: 0.05,
        unit: "s",
      },
      {
        name: "LABELS_TIME",
        label: "Fade in over",
        kind: "range",
        min: 0.1,
        max: 2,
        step: 0.05,
        unit: "s",
      },
    ],
  ],
  [
    "Cycling the drawings",
    [
      {
        name: "CYCLE",
        label: "Cycle through the drawings",
        kind: "switch",
        hint: "Off, the first drawing stays.",
      },
      { name: "CYCLE_STYLE", label: "How one gives way to the next", kind: "style" },
      {
        name: "CYCLE_HOLD",
        label: "Each drawing stays for",
        kind: "range",
        min: 0.5,
        max: 20,
        step: 0.5,
        unit: "s",
        hint: "Also the wait after the draw-in. Shorten it to watch changes back to back.",
      },
      {
        name: "CYCLE_OUT_TIME",
        label: "The old one leaves over",
        kind: "range",
        min: 0.1,
        max: 3,
        step: 0.05,
        unit: "s",
      },
      {
        name: "CYCLE_IN_TIME",
        label: "The new one arrives over",
        kind: "range",
        min: 0.2,
        max: 4,
        step: 0.05,
        unit: "s",
        hint: "Its lines draw in over this, where they draw. The straightedge crosses in this time.",
      },
      {
        name: "CYCLE_OVERLAP",
        label: "The new one starts before the old has gone",
        kind: "range",
        min: -1.5,
        max: 2,
        step: 0.05,
        unit: "s",
        hint: "Negative leaves a gap between them.",
      },
      {
        name: "CYCLE_SLIDE",
        label: "Slides this far",
        kind: "range",
        min: 0,
        max: 300,
        step: 5,
        unit: "px",
        hint: "In the plate’s pixels. The entry is 500 across.",
      },
      {
        name: "CYCLE_CAPTION_TIME",
        label: "Captions fade out, then in, over",
        kind: "range",
        min: 0.1,
        max: 1.5,
        step: 0.05,
        unit: "s",
      },
    ],
  ],
];

export const EASES: [string, PlateTiming["RIM_EASE"]][] = [
  ["Steady", [0, 0, 1, 1]],
  ["Eases in and out", [0.45, 0, 0.55, 1]],
  ["Starts quick, slows", [0.2, 0.6, 0.35, 1]],
  ["Starts slow, quickens", [0.55, 0, 0.8, 0.4]],
];

export const STYLES: [CycleStyle, string][] = [
  ["slide", "Slide: out to the left, in from the right"],
  ["redraw", "Redraw: fade out, then draw in line by line"],
  ["slideDraw", "Slide and draw: slide out, draw in while settling"],
  ["straightedge", "Straightedge: a rule wipes one into the next"],
  ["erase", "Erase: lines draw back out, then the new draw in"],
];

/** Choices to compare, as changes from timing.ts's values. Choosing one
 * plays from just before the first change. */
export const PRESETS: [string, string, Partial<PlateTiming>][] = [
  [
    "Slide across",
    "The old drawing fades out drifting left as the new one slides in from the right.",
    {
      CYCLE: true,
      CYCLE_STYLE: "slide",
      CYCLE_OUT_TIME: 0.8,
      CYCLE_IN_TIME: 1.0,
      CYCLE_OVERLAP: 0.4,
      CYCLE_SLIDE: 60,
    },
  ],
  [
    "Fade, then redraw",
    "The old one fades where it is; the new one is drawn in line by line, like the rifle at the start.",
    {
      CYCLE: true,
      CYCLE_STYLE: "redraw",
      CYCLE_OUT_TIME: 0.6,
      CYCLE_IN_TIME: 1.8,
      CYCLE_OVERLAP: 0.2,
    },
  ],
  [
    "Slide out, draw in",
    "The old one drifts off left; the new one draws in as it settles from the right.",
    {
      CYCLE: true,
      CYCLE_STYLE: "slideDraw",
      CYCLE_OUT_TIME: 0.7,
      CYCLE_IN_TIME: 1.8,
      CYCLE_OVERLAP: 0.3,
      CYCLE_SLIDE: 40,
    },
  ],
  [
    "Straightedge",
    "A blue drafting rule crosses the entry, wiping away the old drawing and leaving the new behind it.",
    { CYCLE: true, CYCLE_STYLE: "straightedge", CYCLE_IN_TIME: 1.4 },
  ],
  [
    "Erase and redraw",
    "The old one's lines draw back out, as if erased; then the new one is drawn in.",
    {
      CYCLE: true,
      CYCLE_STYLE: "erase",
      CYCLE_OUT_TIME: 1.0,
      CYCLE_IN_TIME: 1.8,
      CYCLE_OVERLAP: 0,
    },
  ],
];
