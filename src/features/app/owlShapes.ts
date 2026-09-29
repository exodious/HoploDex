/*
 * Athena's owl, as on the Athenian tetradrachm (issue #22): the shapes
 * shared by the chooser plate's engraved owl and the solid owl of the
 * program icon's small sizes and the top bar's mark. Drawn in a ±100 box,
 * y down, then moved 20 right to centre it.
 */

export const OWL_OFFSET = "translate(20 0)";
export const OWL_HEAD =
  "M-40-50C-41-74-28-88 4-88C36-88 47-74 46-50C45-24 28-10 4-10C-20-10-39-24-40-50Z";
export const OWL_BODY =
  "M-28-20C-50 2-56 38-48 60L-62 90H-40L-30 82C-12 92 22 92 36 72C54 48 52 4 34-22";
export const OWL_LEAVES = [
  "M-52-66C-64-80-62-94-56-100C-48-90-46-76-52-66Z",
  "M-60-58C-74-60-88-66-92-74C-80-76-66-72-60-58Z",
];
export const OWL_EYES: [number, number][] = [
  [-15, -52],
  [23, -52],
];
