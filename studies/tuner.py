"""Builds hoplon-tuner.html: the plate's draw-in with its timing on sliders,
a timeline to scrub, and a few presets to compare. The timing comes from
animate.js and the starting values from concept2.py's knobs; the page gives
back the settings you changed, to paste into concept2.py."""
import json
import pathlib

import concept2

HERE = pathlib.Path(__file__).parent
FONTS = (HERE / "fonts.css").read_text()

# The tuner is for watching the animation, so it keeps animating under reduced
# motion; it just doesn't start playing on its own.
PLATE_CSS = concept2.PLATE_CSS[:concept2.PLATE_CSS.index("@media (prefers-reduced-motion")]

# The sliders, grouped in the order things draw. unit "s" is seconds; "×" and
# "%" are shown as such.
GROUPS = [
    ("Overall", [
        ("SPEED", "Overall pace", 0.5, 2.5, 0.05, "×", "Multiplies every time below. Higher is slower."),
    ]),
    ("Greek key", [
        ("KEY_START", "Starts at", 0, 5, 0.05, "s", ""),
        ("KEY_TIME", "Whole rule", 0.4, 3, 0.05, "s", "It draws from both ends and meets in the middle."),
        ("KEY_UNIT_TIME", "Each key unit", 0.1, 1.2, 0.05, "s", ""),
    ]),
    ("Rifle", [
        ("RIFLE_START", "Starts at", 0, 5, 0.05, "s", ""),
        ("RIFLE_TIME", "Draws over", 0.4, 4, 0.05, "s", "Eased, so it looks finished early. The timeline shades the tail."),
    ]),
    ("Shield rim", [
        ("RIM_START", "Sweep starts at", 0, 5, 0.05, "s", ""),
        ("RIM_TIME", "Once round", 0.5, 5, 0.05, "s", ""),
        ("RIM_EASE", "Sweep pace", None, None, None, "", ""),
        ("BRAID_LAG", "Braid behind the circles", -0.5, 0.5, 0.02, "s", "The tongues and beads keep to the braid."),
        ("TONGUE_TIME", "Each tongue draws over", 0.05, 1.5, 0.05, "s", ""),
        ("BEAD_TIME", "Each bead fades in over", 0.05, 1, 0.05, "s", ""),
        ("SHIELD_FILL_AT", "Ground fades in from", 0, 1, 0.05, "%", "How far round the sweep is. Earlier hides the grid ahead of the lines."),
        ("SHIELD_FILL_TIME", "Ground fades in over", 0.1, 2, 0.05, "s", ""),
    ]),
    ("Owl", [
        ("OWL_GAP", "Starts after the braid closes", -3, 1.5, 0.05, "s", "Negative starts the owl while the rim is still drawing."),
        ("OWL_LINES_TIME", "Lines draw over", 0.5, 4, 0.05, "s", "Eased like the rifle."),
        ("OWL_PAPER_AFTER", "Head’s ground, after its lines start", 0, 3, 0.05, "s", "Hides the body’s lines behind the head."),
        ("BEAK_FILL_AFTER", "Beak and berry fill, after its lines start", 0, 4, 0.05, "s", ""),
        ("BEAK_FILL_TIME", "Beak and berry fill over", 0.1, 2, 0.05, "s", ""),
        ("WASH_AFTER", "Bronze wash, after its lines start", 0, 4, 0.05, "s", ""),
        ("WASH_TIME", "Wash fades in over", 0.1, 3, 0.05, "s", ""),
        ("PUPIL_PAUSE", "Pause before the pupils", 0, 2.5, 0.05, "s", "Counted from the end of the lines’ time."),
        ("PUPIL_TIME", "Pupils fill over", 0.05, 2, 0.05, "s", "Smaller is quicker."),
    ]),
    ("Captions and scale bars", [
        ("LABELS_START", "Fade in at", 0, 6, 0.05, "s", ""),
        ("LABELS_TIME", "Fade in over", 0.1, 2, 0.05, "s", ""),
    ]),
]

EASES = [("Steady", [0, 0, 1, 1]), ("Eases in and out", [0.45, 0, 0.55, 1]),
         ("Starts quick, slows", [0.2, 0.6, 0.35, 1]), ("Starts slow, quickens", [0.55, 0, 0.8, 0.4])]

# Permutations to compare, as changes from concept2.py's values.
PRESETS = [
    ("Owl after the braid", "The braid closes, then the owl is engraved.", {}),
    ("Owl during the sweep", "As it was: the owl starts with the braid half way round.", {"OWL_GAP": -0.9}),
    ("Owl as the braid closes", "A short overlap: the owl starts in the sweep’s last stretch.", {"OWL_GAP": -0.4}),
    ("After the braid, unhurried", "The same order, a third slower, with a longer beat before the owl.", {"SPEED": 1.35, "OWL_GAP": 0.35}),
    ("After the braid, eased sweep", "The sweep gathers pace and settles as it closes.", {"RIM_EASE": [0.45, 0, 0.55, 1], "OWL_GAP": 0.1}),
]


def control(name, label, lo, hi, step, unit, hint):
    hint_html = f'<p class="hint" id="{name}-hint">{hint}</p>' if hint else ""
    described = f' aria-describedby="{name}-hint"' if hint else ""
    if name == "RIM_EASE":
        options = "".join(f'<option value="{i}">{text}</option>' for i, (text, _) in enumerate(EASES))
        field = f'<select id="{name}" data-knob="{name}">{options}</select>'
        out = ""
    else:
        field = (f'<input type="range" id="{name}" data-knob="{name}" min="{lo}" max="{hi}" step="{step}"'
                 f' data-unit="{unit}"{described}>')
        out = f'<output for="{name}" id="{name}-out"></output>'
    return (f'<div class="knob"><div class="knob-head"><label for="{name}">{label}</label>{out}</div>'
            f'{field}<div class="knob-foot"><code>{name}</code>{hint_html}</div></div>')


CONTROLS = "".join(
    f'<fieldset><legend>{title}</legend>{"".join(control(*k) for k in knobs)}</fieldset>' for title, knobs in GROUPS)
PRESET_BUTTONS = "".join(
    f'<button type="button" class="preset" data-preset="{i}" aria-pressed="false"><b>{name}</b><span>{note}</span></button>'
    for i, (name, note, _) in enumerate(PRESETS))

LIGHT = """--vellum:#edefea;--sheet:#f9faf7;--sunk:#e3e6e0;--ink:#161c24;--ink-2:#4f5965;--ink-3:#646d78;
--rule:#d3d7d0;--rule-strong:#858c86;--niter:#2344be;--niter-wash:#e2e7fa;--on-ink:#f9faf7;
--grid-minor:rgb(35 68 190/.045);--grid-major:rgb(35 68 190/.085);--line:rgb(79 89 101/.62);--hatch:rgb(79 89 101/.35);--orn:#9a6a2c"""
DARK = """--vellum:#0f1419;--sheet:#161d25;--sunk:#0b0f14;--ink:#e6e9ec;--ink-2:#a8b1bb;--ink-3:#8f99a5;
--rule:#28313b;--rule-strong:#67727d;--niter:#93a8ff;--niter-wash:#1c2647;--on-ink:#0f1419;
--grid-minor:rgb(147 168 255/.035);--grid-major:rgb(147 168 255/.07);--line:rgb(168 177 187/.5);--hatch:rgb(168 177 187/.25);--orn:#d3a45f"""

CSS = f"""{FONTS}
:root{{{LIGHT};--font-display:"Big Shoulders Display Variable","Arial Narrow",sans-serif;
--font-body:"Atkinson Hyperlegible Next Variable",system-ui,sans-serif;--font-mono:"Atkinson Hyperlegible Mono Variable",ui-monospace,monospace;--label-w:9.5rem}}
@media (prefers-color-scheme:dark){{:root:not([data-theme="light"]){{color-scheme:dark;{DARK}}}}}
:root[data-theme="dark"]{{color-scheme:dark;{DARK}}}
.stage[data-plate="light"]{{color-scheme:light;{LIGHT}}}
.stage[data-plate="dark"]{{color-scheme:dark;{DARK}}}
*{{box-sizing:border-box}}
body{{background:var(--vellum);color:var(--ink);font:15px/1.5 var(--font-body);padding-inline:20px;padding-block:20px 48px}}
h1{{font:800 2rem/1 var(--font-display);letter-spacing:.01em;margin:0;text-wrap:balance}}
h2{{font:700 1.25rem/1.1 var(--font-display);letter-spacing:.02em;margin:0}}
header{{display:flex;flex-wrap:wrap;align-items:end;justify-content:space-between;gap:12px 24px;margin-bottom:20px;max-width:1500px;margin-inline:auto}}
header p{{margin:6px 0 0;color:var(--ink-2);max-width:64ch}}
button,select{{font:inherit;color:inherit}}
button:focus-visible,select:focus-visible,input:focus-visible,.tl-body:focus-visible{{outline:2px solid var(--niter);outline-offset:2px}}
.seg{{display:inline-flex;border:1px solid var(--rule-strong);border-radius:6px;overflow:hidden}}
.seg button{{border:0;background:var(--sheet);padding:6px 12px;cursor:pointer;font-weight:600}}
.seg button+button{{border-left:1px solid var(--rule-strong)}}
.seg button[aria-pressed="true"]{{background:var(--ink);color:var(--on-ink)}}
.app{{display:grid;gap:20px;max-width:1500px;margin-inline:auto;grid-template-columns:minmax(0,1fr);grid-template-areas:"stage" "play" "controls"}}
@media (min-width:780px){{.app{{grid-template-columns:minmax(0,1fr) minmax(300px,360px);grid-template-areas:"stage controls" "play controls"}}}}
@media (min-width:1240px){{.app{{grid-template-columns:minmax(0,.9fr) minmax(0,1fr) 340px;grid-template-areas:"stage play controls"}}}}
.stage{{grid-area:stage;position:relative;aspect-ratio:580/700;max-width:100%;border:1px solid var(--rule);border-radius:6px;overflow:hidden;
align-self:start;background-color:var(--vellum);color:var(--ink);background-image:
 linear-gradient(var(--grid-major) 1px,transparent 1px),linear-gradient(90deg,var(--grid-major) 1px,transparent 1px),
 linear-gradient(var(--grid-minor) 1px,transparent 1px),linear-gradient(90deg,var(--grid-minor) 1px,transparent 1px);
 background-size:100px 100px,100px 100px,20px 20px,20px 20px}}
@media (min-width:780px) and (max-width:1239px){{.stage{{max-height:78vh;justify-self:center;width:auto;height:78vh}}}}
.play{{grid-area:play;display:grid;gap:14px;align-content:start}}
.transport{{display:flex;flex-wrap:wrap;align-items:center;gap:10px 12px}}
.btn{{border:1px solid var(--rule-strong);background:var(--sheet);border-radius:6px;padding:7px 14px;font-weight:700;cursor:pointer}}
.btn.primary{{background:var(--ink);color:var(--on-ink);border-color:var(--ink);min-width:6.5rem}}
.clock{{font:500 15px var(--font-mono);font-variant-numeric:tabular-nums;color:var(--ink-2);margin-right:auto}}
.clock b{{color:var(--ink);font-weight:700}}
#scrub{{width:100%;accent-color:var(--niter);margin:0}}
.tl{{border:1px solid var(--rule);border-radius:6px;background:var(--sheet);padding:12px 14px 8px}}
.tl-body{{position:relative;display:grid;gap:5px;cursor:ew-resize;touch-action:none;border-radius:4px}}
.tl-row{{display:grid;grid-template-columns:var(--label-w) minmax(0,1fr);align-items:center;gap:0;font-size:13.5px;color:var(--ink-2)}}
.tl-track{{position:relative;height:16px;background:repeating-linear-gradient(90deg,var(--rule) 0 1px,transparent 1px var(--tick-gap,10%))}}
.bar{{position:absolute;top:2px;bottom:2px;border-radius:2px;min-width:2px}}
.bar.orn,.bar.solid{{background:var(--orn)}}.bar.solid{{box-shadow:0 0 0 1.5px var(--ink)}}.bar.line{{background:var(--rule-strong)}}.bar.ink{{background:var(--ink-2)}}
.bar.fill{{background:color-mix(in srgb,var(--orn) 40%,transparent)}}
.bar.eased{{background:linear-gradient(90deg,var(--c) 0 var(--done),color-mix(in srgb,var(--c) 30%,transparent) var(--done))}}
.bar.eased.orn{{--c:var(--orn)}}.bar.eased.line{{--c:var(--rule-strong)}}
.tl-axis{{display:grid;grid-template-columns:var(--label-w) minmax(0,1fr);font:12px var(--font-mono);color:var(--ink-3);font-variant-numeric:tabular-nums;margin-top:2px}}
.ticks{{position:relative;height:16px}}.ticks span{{position:absolute;transform:translateX(-50%)}}
.playhead{{position:absolute;top:-4px;bottom:-4px;width:2px;background:var(--niter);pointer-events:none;
left:calc(var(--label-w) + (100% - var(--label-w)) * var(--p,0) - 1px)}}
.tl-note{{font-size:13px;color:var(--ink-3);margin:8px 0 0}}
.presets{{display:grid;gap:8px}}
.preset{{text-align:left;border:1px solid var(--rule);background:var(--sheet);border-radius:6px;padding:8px 12px;cursor:pointer;display:grid;gap:1px}}
.preset b{{font-size:14.5px}}.preset span{{font-size:13px;color:var(--ink-2)}}
.preset[aria-pressed="true"]{{border-color:var(--niter);box-shadow:0 0 0 1px var(--niter);background:var(--niter-wash)}}
.out{{border:1px solid var(--rule);border-radius:6px;background:var(--sheet);padding:12px 14px;display:grid;gap:8px}}
.out-head{{display:flex;align-items:center;justify-content:space-between;gap:12px}}
.out pre{{margin:0;font:13.5px/1.5 var(--font-mono);white-space:pre-wrap;overflow-wrap:anywhere;color:var(--ink)}}
.out .empty{{margin:0;color:var(--ink-2);font-size:13.5px}}
.controls{{grid-area:controls;display:grid;gap:14px;align-content:start}}
@media (min-width:780px){{.controls{{position:sticky;top:12px;max-height:calc(100vh - 24px);overflow-y:auto;padding-right:6px}}}}
fieldset{{border:0;border-top:1px solid var(--ink);margin:0;padding:8px 0 0;display:grid;gap:12px}}
legend{{font:700 1.2rem/1 var(--font-display);letter-spacing:.02em;padding:0 8px 0 0}}
.knob{{display:grid;gap:3px}}
.knob-head{{display:flex;justify-content:space-between;align-items:baseline;gap:12px}}
.knob-head label{{font-weight:600;font-size:14px}}
.knob-head output{{font:600 14px var(--font-mono);font-variant-numeric:tabular-nums;white-space:nowrap}}
.knob.changed .knob-head output{{color:var(--niter)}}
.knob input[type=range]{{width:100%;accent-color:var(--niter);margin:0}}
.knob select{{width:100%;padding:6px 8px;border:1px solid var(--rule-strong);border-radius:6px;background:var(--sheet)}}
.knob-foot{{display:flex;flex-wrap:wrap;gap:2px 10px;align-items:baseline}}
.knob-foot code{{font:12px var(--font-mono);color:var(--ink-3)}}
.hint{{margin:0;font-size:12.5px;color:var(--ink-2);flex-basis:100%}}
{PLATE_CSS}
"""

BODY = f"""<meta charset="utf-8">
<title>Hoplon Draw-in Tuner</title>
<style>{CSS}</style>
<header><div><h1>Hoplon draw-in tuner</h1>
<p>Play or scrub the startup drawing, try a preset, or move the sliders. Paused, the drawing shows that moment with the new timing; playing, it starts again when you let go. Copy what you changed into concept2.py.</p></div>
<div class="seg" role="group" aria-label="Plate colours"><button type="button" data-plate="light" aria-pressed="false">Light plate</button><button type="button" data-plate="dark" aria-pressed="false">Dark plate</button></div>
</header>
<main class="app">
<div class="stage" id="stage"><svg class="plate anim" viewBox="600 40 580 700" role="img" aria-label="The catalogue plate: the hoplon with its owl, the Greek key and the rifle">{concept2.PLATE}</svg></div>
<section class="play" aria-label="Playback">
<div class="transport"><button type="button" class="btn primary" id="toggle">Pause</button><button type="button" class="btn" id="replay">Replay</button>
<span class="clock" id="clock"></span>
<div class="seg" role="group" aria-label="Playback rate"><button type="button" data-rate="1" aria-pressed="true">1×</button><button type="button" data-rate="0.5" aria-pressed="false">½×</button><button type="button" data-rate="0.25" aria-pressed="false">¼×</button></div></div>
<input type="range" id="scrub" min="0" step="0.01" aria-label="Time in seconds">
<div class="tl"><div class="tl-body" id="tl" tabindex="0" aria-label="Timeline: drag to scrub"></div><div class="tl-axis"><span></span><div class="ticks" id="ticks"></div></div>
<p class="tl-note">Each bar is when that part draws. Eased line work looks finished where its bar fades.</p></div>
<h2>Presets</h2>
<div class="presets" id="presets">{PRESET_BUTTONS}</div>
<div class="out"><div class="out-head"><h2>Your changes</h2><div class="transport"><button type="button" class="btn" id="copy">Copy</button><button type="button" class="btn" id="reset">Reset</button></div></div>
<div id="changes"></div></div>
</section>
<form class="controls" id="controls" aria-label="Timing">{CONTROLS}</form>
</main>
<script>{concept2.ANIMATE_JS}
hoplon.DEFAULTS = {concept2.knobs()};
const EASES = {json.dumps(EASES)};
const PRESETS = {json.dumps(PRESETS)};
</script>
<script>{(HERE / "tuner.js").read_text()}</script>
"""

if __name__ == "__main__":
    (HERE / "hoplon-tuner.html").write_text(BODY)
    print(len(BODY))
