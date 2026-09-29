// The plate's draw-in timing: every element's animation, worked out from the
// knobs (seconds; their defaults are in concept2.py). The gallery, the
// c2-animated pages and the tuner all use this, so they can't drift apart.
const hoplon = (() => {
  const EASE = "cubic-bezier(.2,.7,.2,1)"; // the rifle's and the owl's lines: EASE_CURVE below

  const EASE_CURVE = [0.2, 0.7, 0.2, 1];

  // A cubic-bezier's progress at time `v`, or with `inverse`, the time at
  // which it reaches progress `v` (both 0..1).
  function curve([x1, y1, x2, y2], v, inverse = false) {
    const at = (u, a, b) => 3 * a * u * (1 - u) ** 2 + 3 * b * u * u * (1 - u) + u ** 3;
    const [ia, ib, oa, ob] = inverse ? [y1, y2, x1, x2] : [x1, x2, y1, y2];
    let lo = 0, hi = 1;
    for (let i = 0; i < 40; i++) {
      const u = (lo + hi) / 2;
      if (at(u, ia, ib) < v) lo = u; else hi = u;
    }
    return at((lo + hi) / 2, oa, ob);
  }

  // When the eased sweep has gone `frac` of the way round.
  const sweepTime = (k, frac) => k.RIM_START + k.RIM_TIME * curve(k.RIM_EASE, frac, true);

  // When an eased line work stage looks finished (95% drawn), as a fraction of its time.
  const looksDone = curve(EASE_CURVE, 0.95, true);

  // When the braid closes back at 12 o'clock; the owl counts from here.
  const braidClosed = k => k.RIM_START + k.RIM_TIME + k.BRAID_LAG;
  const owlStart = k => braidClosed(k) + k.OWL_GAP;

  // Each stage as [start, end] in unscaled seconds, for the tuner's timeline.
  function phases(k) {
    const owl = owlStart(k);
    const lastTongue = sweepTime(k, 1 - 1 / k.TONGUES) + k.BRAID_LAG + k.TONGUE_TIME;
    const fillStart = sweepTime(k, k.SHIELD_FILL_AT);
    const pupils = owl + k.OWL_LINES_TIME + k.PUPIL_PAUSE;
    return [
      { id: "key", label: "Greek key", tone: "orn", span: [k.KEY_START, k.KEY_START + k.KEY_TIME] },
      { id: "rifle", label: "Rifle", tone: "line", span: [k.RIFLE_START, k.RIFLE_START + k.RIFLE_TIME], eased: true },
      { id: "rim", label: "Rim sweep", tone: "orn", span: [k.RIM_START, Math.max(braidClosed(k), lastTongue)] },
      { id: "ground", label: "Shield ground", tone: "fill", span: [fillStart, fillStart + k.SHIELD_FILL_TIME] },
      { id: "owl", label: "Owl's lines", tone: "orn", span: [owl, owl + k.OWL_LINES_TIME], eased: true },
      { id: "beak", label: "Beak and berry", tone: "fill", span: [owl + k.BEAK_FILL_AFTER, owl + k.BEAK_FILL_AFTER + k.BEAK_FILL_TIME] },
      { id: "wash", label: "Bronze wash", tone: "fill", span: [owl + k.WASH_AFTER, owl + k.WASH_AFTER + k.WASH_TIME] },
      { id: "pupils", label: "Pupils", tone: "solid", span: [pupils, pupils + k.PUPIL_TIME] },
      { id: "labels", label: "Captions", tone: "ink", span: [k.LABELS_START, k.LABELS_START + k.LABELS_TIME] },
    ];
  }

  // Gives every element of the plate its animation.
  function apply(svg, k) {
    const s = t => `${(t * k.SPEED).toFixed(3)}s`;
    const set = (sel, value) => svg.querySelectorAll(sel).forEach(el => { el.style.animation = value; });

    // 1. the Greek key, from both ends to the middle
    const units = [...svg.querySelectorAll(".key.unit")];
    const far = Math.max(1, ...units.map(el => +el.dataset.key));
    const step = (k.KEY_TIME - k.KEY_UNIT_TIME) / far;
    units.forEach(el => { el.style.animation = `draw ${s(k.KEY_UNIT_TIME)} ${s(k.KEY_START + +el.dataset.key * step)} linear forwards`; });
    set(".key:not(.unit)", `draw ${s(k.KEY_TIME)} ${s(k.KEY_START)} linear forwards`);

    // 2. the rifle
    const rifle = `draw ${s(k.RIFLE_TIME)} ${s(k.RIFLE_START)} ${EASE} forwards`;
    set(".dwg-gun .open, .dwg-gun .thin, .dwg-gun .detail, .dwg-gun .cut", rifle);
    set(".dwg-gun .part", `${rifle}, fillin .9s ${s(k.RIFLE_START + 0.4)} linear forwards`);

    // 3. the rim: each piece starts as the sweep reaches it
    svg.querySelectorAll("[data-sweep]").forEach(el => {
      const [f0, f1, kind] = el.dataset.sweep.split(" ");
      const lag = kind === "line" ? 0 : k.BRAID_LAG;
      const t0 = sweepTime(k, +f0) + lag;
      if (kind === "bead") el.style.animation = `fade ${s(k.BEAD_TIME)} ${s(t0)} linear forwards`;
      else if (kind === "tongue") el.style.animation = `draw ${s(k.TONGUE_TIME)} ${s(t0)} linear forwards`;
      else el.style.animation = `draw ${s(sweepTime(k, +f1) - sweepTime(k, +f0))} ${s(t0)} linear forwards`;
    });
    set(".ground", `fillin ${s(k.SHIELD_FILL_TIME)} ${s(sweepTime(k, k.SHIELD_FILL_AT))} linear forwards`);

    // 4. the owl, after the braid closes: lines, grounds, wash, then pupils
    const owl = owlStart(k);
    set(".device .ol, .device .fe", `draw ${s(k.OWL_LINES_TIME)} ${s(owl)} ${EASE} forwards`);
    set(".device .paper", `fillin .9s ${s(owl + k.OWL_PAPER_AFTER)} linear forwards`);
    set(".device .pfi", `fade ${s(k.BEAK_FILL_TIME)} ${s(owl + k.BEAK_FILL_AFTER)} linear forwards`);
    set(".device .wash", `wash ${s(k.WASH_TIME)} ${s(owl + k.WASH_AFTER)} linear forwards`);
    // the pupils ease in: slow to start, then they open
    set(".device .pf", `fade ${s(k.PUPIL_TIME)} ${s(owl + k.OWL_LINES_TIME + k.PUPIL_PAUSE)} ease-in forwards`);

    // 5. captions, scale bars and the rifle's centreline
    set(".hatch, .axis, text, rect", `fade ${s(k.LABELS_TIME)} ${s(k.LABELS_START)} linear forwards`);
    svg.querySelectorAll(".dwg-gun .axis").forEach(el => { el.style.display = k.BORE_AXIS === false ? "none" : ""; });
  }

  // How long the whole draw-in takes, in scaled seconds.
  const duration = k => k.SPEED * Math.max(...phases(k).map(p => p.span[1]));

  return { apply, phases, duration, looksDone, DEFAULTS: null };
})();
