// The tuner: the plate's animation driven by a clock of our own, so it can
// be paused, scrubbed and slowed, and rebuilt whenever a knob moves.
(() => {
  const D = hoplon.DEFAULTS;
  const $ = sel => document.querySelector(sel);
  const stage = $("#stage"), pristine = stage.innerHTML;
  const scrub = $("#scrub"), tl = $("#tl"), toggle = $("#toggle");
  const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
  const SAVE = "hoplon-tuner";

  let k = { ...D };
  try { Object.assign(k, JSON.parse(localStorage.getItem(SAVE) || "{}").knobs || {}); } catch (e) { /* no storage */ }

  let anims = [], total = 1, rate = 1;
  let playing = false, at = 0, since = 0; // at: seconds shown when the clock last changed; since: when that was

  const now = () => playing ? Math.min(total, at + (performance.now() - since) / 1000 * rate) : at;

  // Puts every animation at `t` seconds, running the ones not yet done if we're playing.
  function show(t) {
    at = t; since = performance.now();
    const ms = t * 1000;
    for (const a of anims) {
      a.playbackRate = rate;
      a.currentTime = ms;
      const end = a.effect.getComputedTiming().endTime;
      if (playing && ms < end) a.play(); else a.pause();
    }
  }

  function rebuild(t) {
    stage.innerHTML = pristine;
    const svg = stage.querySelector("svg");
    hoplon.apply(svg, k);
    anims = svg.getAnimations({ subtree: true });
    total = hoplon.duration(k) + 0.3;
    scrub.max = total.toFixed(2);
    drawTimeline();
    show(Math.min(t, total));
  }

  function setPlaying(p) {
    const t = now();
    playing = p;
    if (playing && t >= total - 0.01) show(0); else show(t);
    toggle.textContent = playing ? "Pause" : "Play";
  }

  // ── the timeline ──
  function drawTimeline() {
    const x = t => `${(t / total) * 100}%`;
    const step = total > 8 ? 2 : total > 4 ? 1 : 0.5;
    tl.style.setProperty("--tick-gap", x(step));
    tl.innerHTML = hoplon.phases(k).map(p => {
      const [a, b] = p.span.map(v => v * k.SPEED);
      const eased = p.eased ? ` eased" style="--done:${hoplon.looksDone * 100}%;` : `" style="`;
      return `<div class="tl-row"><span>${p.label}</span><div class="tl-track">` +
        `<span class="bar ${p.tone}${eased}left:${x(a)};width:${x(Math.max(0, b - a))}" title="${a.toFixed(2)}–${b.toFixed(2)} s"></span></div></div>`;
    }).join("") + '<span class="playhead" id="playhead"></span>';
    let ticks = "";
    for (let t = 0; t <= total + 1e-6; t += step) ticks += `<span style="left:${x(t)}">${t % 1 ? t.toFixed(1) : t}</span>`;
    $("#ticks").innerHTML = ticks;
  }

  function tick() {
    const t = now();
    if (playing && t >= total) setPlaying(false);
    $("#clock").innerHTML = `<b>${t.toFixed(2)} s</b> of ${total.toFixed(2)} s`;
    scrub.value = t.toFixed(2);
    const head = $("#playhead");
    if (head) head.style.setProperty("--p", Math.min(1, t / total));
    requestAnimationFrame(tick);
  }

  // Dragging on the timeline scrubs it.
  function scrubTo(e) {
    const track = tl.querySelector(".tl-track").getBoundingClientRect();
    const t = Math.max(0, Math.min(total, (e.clientX - track.left) / track.width * total));
    if (playing) setPlaying(false);
    show(t);
  }
  tl.addEventListener("pointerdown", e => { tl.setPointerCapture(e.pointerId); scrubTo(e); });
  tl.addEventListener("pointermove", e => { if (tl.hasPointerCapture(e.pointerId)) scrubTo(e); });
  tl.addEventListener("keydown", e => {
    const d = { ArrowLeft: -0.1, ArrowRight: 0.1 }[e.key];
    if (d === undefined) return;
    e.preventDefault();
    if (playing) setPlaying(false);
    show(Math.max(0, Math.min(total, now() + d * (e.shiftKey ? 10 : 1))));
  });
  scrub.addEventListener("input", () => { if (playing) setPlaying(false); show(+scrub.value); });

  toggle.addEventListener("click", () => setPlaying(!playing));
  $("#replay").addEventListener("click", () => { playing = true; toggle.textContent = "Pause"; show(0); });
  document.querySelectorAll("[data-rate]").forEach(b => b.addEventListener("click", () => {
    rate = +b.dataset.rate;
    document.querySelectorAll("[data-rate]").forEach(o => o.setAttribute("aria-pressed", o === b));
    show(now());
  }));

  // ── the knobs ──
  const fmt = (name, v) => {
    const el = document.getElementById(name);
    const unit = el.dataset.unit;
    if (unit === "%") return `${Math.round(v * 100)}%`;
    if (unit === "×") return `${v.toFixed(2)}×`;
    return `${v >= 0 ? "" : "−"}${Math.abs(v).toFixed(2)} s`;
  };
  const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
  const easeIndex = e => Math.max(0, EASES.findIndex(([, v]) => same(v, e)));

  function syncControls() {
    document.querySelectorAll("[data-knob]").forEach(el => {
      const name = el.dataset.knob;
      if (name === "RIM_EASE") el.value = easeIndex(k.RIM_EASE);
      else { el.value = k[name]; document.getElementById(`${name}-out`).textContent = fmt(name, k[name]); }
      el.closest(".knob").classList.toggle("changed", !same(k[name], D[name]));
    });
    document.querySelectorAll(".preset").forEach(b => {
      const [, , set] = PRESETS[+b.dataset.preset];
      b.setAttribute("aria-pressed", same(k, { ...D, ...set }));
    });
    showChanges();
    try { localStorage.setItem(SAVE, JSON.stringify({ knobs: k, plate: stage.dataset.plate })); } catch (e) { /* no storage */ }
  }

  const py = v => Array.isArray(v) ? `(${v.join(", ")})` : String(+v.toFixed(3));
  function changed() { return Object.keys(D).filter(n => !same(k[n], D[n])); }
  function showChanges() {
    const names = changed();
    $("#changes").innerHTML = names.length
      ? `<pre id="changes-text">${names.map(n => `${n} = ${py(k[n])}`).join("\n")}</pre>`
      : '<p class="empty">Nothing changed yet: these are concept2.py’s values.</p>';
    $("#copy").disabled = !names.length;
  }

  $("#controls").addEventListener("input", e => {
    const name = e.target.dataset.knob;
    if (!name) return;
    k[name] = name === "RIM_EASE" ? EASES[+e.target.value][1] : +e.target.value;
    syncControls();
    if (!playing) rebuild(at); else drawTimeline();
  });
  // Playing, the drawing starts again once a slider is let go.
  $("#controls").addEventListener("change", () => { if (playing) rebuild(0); });

  document.querySelectorAll(".preset").forEach(b => b.addEventListener("click", () => {
    k = { ...D, ...PRESETS[+b.dataset.preset][2] };
    syncControls();
    playing = true; toggle.textContent = "Pause";
    rebuild(0);
  }));
  $("#reset").addEventListener("click", () => { k = { ...D }; syncControls(); rebuild(playing ? 0 : at); });
  $("#copy").addEventListener("click", async () => {
    const text = changed().map(n => `${n} = ${py(k[n])}`).join("\n");
    const btn = $("#copy");
    try { await navigator.clipboard.writeText(text); btn.textContent = "Copied"; }
    catch (e) { getSelection().selectAllChildren($("#changes-text")); btn.textContent = "Selected"; }
    setTimeout(() => { btn.textContent = "Copy"; }, 1600);
  });

  // ── the plate's colours ──
  function setPlate(p) {
    stage.dataset.plate = p;
    document.querySelectorAll("[data-plate]:not(.stage)").forEach(b => b.setAttribute("aria-pressed", b.dataset.plate === p));
    syncControls();
  }
  document.querySelectorAll("button[data-plate]").forEach(b => b.addEventListener("click", () => setPlate(b.dataset.plate)));
  let plate = null;
  try { plate = JSON.parse(localStorage.getItem(SAVE) || "{}").plate; } catch (e) { /* no storage */ }
  const themed = document.documentElement.dataset.theme;
  setPlate(plate || themed || (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light"));

  // Plays on open, unless the viewer asked for less motion: then it opens on
  // the finished drawing, and Play runs it.
  rebuild(0);
  if (reduced) show(total); else setPlaying(true);
  if (reduced) toggle.textContent = "Play";
  requestAnimationFrame(tick);
})();
