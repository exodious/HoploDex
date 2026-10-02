/*
 * The catalogue plate's timing: when each part of the chooser's drawing
 * draws in, and how it then cycles through the firearm drawings. Every
 * time is in seconds. The app builds these in as they are here.
 *
 * To tune them, run the tuner (`npm run tuner`, see DEVELOPMENT.md
 * "Tuning the chooser's drawing"): it plays the drawing next to these
 * values on sliders, and its Save button writes what you changed back into
 * this file. Keep one value per line, as `NAME: value,`, so it can.
 */

/** How entry 2 changes from one firearm drawing to the next. */
export type CycleStyle = "slide" | "redraw" | "slideDraw" | "straightedge" | "erase";

export interface PlateTiming {
  SPEED: number;
  KEY_START: number;
  KEY_TIME: number;
  KEY_UNIT_TIME: number;
  RIFLE_START: number;
  RIFLE_TIME: number;
  RIM_START: number;
  RIM_TIME: number;
  RIM_EASE: [number, number, number, number];
  BRAID_LAG: number;
  TONGUE_TIME: number;
  BEAD_TIME: number;
  SHIELD_FILL_AT: number;
  SHIELD_FILL_TIME: number;
  OWL_GAP: number;
  OWL_LINES_TIME: number;
  OWL_PAPER_AFTER: number;
  BEAK_FILL_AFTER: number;
  BEAK_FILL_TIME: number;
  WASH_AFTER: number;
  WASH_TIME: number;
  PUPIL_PAUSE: number;
  PUPIL_TIME: number;
  LABELS_START: number;
  LABELS_TIME: number;
  CYCLE: boolean;
  CYCLE_HOLD: number;
  CYCLE_STYLE: CycleStyle;
  CYCLE_OUT_TIME: number;
  CYCLE_IN_TIME: number;
  CYCLE_OVERLAP: number;
  CYCLE_SLIDE: number;
  CYCLE_CAPTION_TIME: number;
  BLINK_OUT_TIME: number;
  BLINK_WAIT: number;
}

export const PLATE_TIMING: PlateTiming = {
  SPEED: 0.65, // multiplies every time below, the cycle's too: 1.5 plays it all half again slower

  // 1. The Greek key draws first, from both ends to the middle.
  KEY_START: 0,
  KEY_TIME: 1.25, // the whole rule
  KEY_UNIT_TIME: 0.4, // one key unit

  // 2. Entry 2's first drawing, once the key is done: the rifle in the
  //    tuner, and whichever drawing the startup shuffle put first in the app.
  RIFLE_START: 1.3,
  RIFLE_TIME: 2.1, // eased: most of it is drawn in the first half

  // 3. The shield's rim. One sweep draws its circles, braid, beads and
  //    tongues together, from 12 o'clock clockwise back to 12.
  RIM_START: 1.3, // when the sweep leaves 12 o'clock
  RIM_TIME: 1.8, // once round
  RIM_EASE: [0, 0, 1, 1], // the sweep's cubic-bezier: [0, 0, 1, 1] is steady
  BRAID_LAG: 0.16, // the braid runs this far behind the rim circles (the tongues and beads keep to it)
  TONGUE_TIME: 0.35, // each tongue takes this long, starting as the sweep reaches it
  BEAD_TIME: 0.35, // each bead fades in over this, as the sweep reaches the middle of its eye
  SHIELD_FILL_AT: 0.8, // the shield's ground starts fading in once the sweep is this far round,
  SHIELD_FILL_TIME: 0.6, // so the grid shows through until the rim is nearly closed

  // 4. The owl is engraved once the braid has closed: its lines, then its
  //    bronze wash, and its pupils last. Its times count from when its lines
  //    start.
  OWL_GAP: 0.2, // after the braid closes; negative starts the owl during the sweep
  OWL_LINES_TIME: 1.8, // eased like the rifle, so the lines look done well before this
  OWL_PAPER_AFTER: 0.4, // the head's ground, which hides the body's lines behind it
  BEAK_FILL_AFTER: 1.1, // the beak and berry fill once their outlines are drawn
  BEAK_FILL_TIME: 0.5,
  WASH_AFTER: 1.4,
  WASH_TIME: 0.8,
  PUPIL_PAUSE: 0.5, // the beat after the owl's lines finish before its pupils fill
  PUPIL_TIME: 0.4, // how long the pupils take to fill: smaller is quicker

  // 5. The captions and scale bars.
  LABELS_START: 3.35,
  LABELS_TIME: 1.3,

  // 6. Once everything is drawn, entry 2 cycles through the firearm
  //    drawings until a database is opened. Each one stays for CYCLE_HOLD.
  CYCLE: true, // false keeps the first drawing
  CYCLE_HOLD: 9, // how long each drawing stays, and the wait after the draw-in
  CYCLE_STYLE: "erase", // "slide", "redraw", "slideDraw", "straightedge" or "erase"
  CYCLE_OUT_TIME: 1.5, // the old drawing leaving
  CYCLE_IN_TIME: 2, // the new one arriving (drawing in, for "redraw", "slideDraw" and "erase")
  CYCLE_OVERLAP: 0, // the new one starts this long before the old has gone; negative leaves a gap
  CYCLE_SLIDE: 40, // how far a drawing slides, in the plate's pixels ("slide" and "slideDraw")
  CYCLE_CAPTION_TIME: 0.5, // the caption and scale bar fade out, then in, over this each

  // 7. A click on the owl's beak makes it blink, once its pupils are drawn:
  //    they go out quickly, as if its eyes closed, and after a moment fill
  //    back in over PUPIL_TIME, as they did in the draw-in.
  BLINK_OUT_TIME: 0.1, // how quickly the pupils go out
  BLINK_WAIT: 0.8, // how long they stay out before filling back in
};
