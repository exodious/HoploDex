"""Concept 2, "Catalogue": a museum plate pairing an ornate hoplon with the
app's firearm drawings, each numbered and captioned like catalogue entries."""
import math
import pathlib
import sys

import build
import concept1
import owl

HERE = pathlib.Path(__file__).parent


# ── Animation knobs ─────────────────────────────────────────────────────
# Every time is in seconds from page load. Change them here, or override any
# of them for one run on the command line (no spaces around "="):
#   python3 gallery.py PUPIL_PAUSE=0.8 PUPIL_TIME=0.2 SPEED=1.5
#   python3 concept2.py RIM_EASE=.45,0,.55,1
# then open c2-animated-dark.html (click it to replay) or the gallery.

SPEED = 1.0  # multiplies every time below: 1.5 plays the whole thing half again slower

# 1. The Greek key draws first, from both ends to the middle.
KEY_TIME = 1.25  # the whole rule
KEY_UNIT_TIME = 0.4  # one key unit

# 2. The rifle, once the key is done.
RIFLE_START = 1.3
RIFLE_TIME = 1.6  # eased: most of it is drawn in the first half

# 3. The shield's rim. One sweep draws its circles, braid, beads and tongues
#    together, from 12 o'clock clockwise back to 12. Everything on it is cut
#    into short pieces, each started as the sweep reaches it, because one
#    long dashed line draws differently from engine to engine.
RIM_START = 1.3  # when the sweep leaves 12 o'clock
RIM_TIME = 1.8  # once round
RIM_EASE = (0, 0, 1, 1)  # the sweep's cubic-bezier: (0, 0, 1, 1) is steady, (.45, 0, .55, 1) eases in and out
RIM_LINE_PIECES = 60  # each rim circle is drawn in this many arcs
BRAID_PIECES_PER_WAVE = 2  # the braid's two strands, in this many pieces per wave (there are 30 waves)
BRAID_LAG = 0.0  # the braid runs this far behind the rim circles (the tongues and beads keep to the braid)
TONGUE_TIME = 0.35  # each tongue takes this long, starting as the sweep reaches it
BEAD_TIME = 0.25  # each bead fades in over this, as the sweep reaches the middle of its eye
SHIELD_FILL_AT = 0.8  # the shield's ground starts fading in once the sweep is this far round,
SHIELD_FILL_TIME = 0.6  # so the grid shows through until the rim is nearly closed

# 4. The owl is engraved: its lines, then its bronze wash, and its pupils last.
OWL_START = 2.2
OWL_LINES_TIME = 1.8  # eased like the rifle, so the lines look done well before this
OWL_PAPER_START = 2.6  # the head's ground, which hides the body's lines behind it
BEAK_FILL_START = 3.3  # the beak and berry fill once their outlines are drawn
BEAK_FILL_TIME = 0.5
WASH_START = 3.6
WASH_TIME = 0.8
PUPIL_PAUSE = 0.3  # the beat after the owl's lines finish before its pupils fill
PUPIL_TIME = 0.4  # how long the pupils take to fill: smaller is quicker

# 5. The captions, scale bars and centreline.
LABELS_START = 2.6
LABELS_TIME = 0.6


def _knob_overrides(args):
    import ast
    for arg in args:
        name, eq, value = arg.partition("=")
        if not eq:
            continue
        if not (name.isupper() and name in globals()):
            raise SystemExit(f"{name} isn't one of concept2.py's animation knobs")
        globals()[name] = ast.literal_eval(value)


_knob_overrides(sys.argv[1:])


def _s(t):
    """A time in seconds, scaled by SPEED, for CSS."""
    return f"{t * SPEED:.3f}s"


def _bezier(ease):
    x1, y1, x2, y2 = ease
    return f"cubic-bezier({x1},{y1},{x2},{y2})"


def _sweep_time(frac):
    """When the rim's sweep reaches `frac` of the way round: RIM_EASE maps
    time to progress, so find the time that gives `frac` (by bisection)."""
    x1, y1, x2, y2 = RIM_EASE

    def at(u, a, b):
        return 3 * a * u * (1 - u) ** 2 + 3 * b * u * u * (1 - u) + u ** 3

    lo, hi = 0.0, 1.0
    for _ in range(40):
        u = (lo + hi) / 2
        lo, hi = (u, hi) if at(u, y1, y2) < frac else (lo, u)
    return RIM_START + RIM_TIME * at((lo + hi) / 2, x1, x2)


def _swept(f0, f1, lag=0.0, duration=None):
    """The inline timing for a piece of the rim from `f0` to `f1` of the way
    round: it starts as the sweep reaches `f0` and, unless it has its own
    `duration`, keeps pace with it to `f1`."""
    t0 = _sweep_time(f0) + lag
    d = duration if duration is not None else _sweep_time(f1) - _sweep_time(f0)
    return f' style="animation-delay:{_s(t0)};animation-duration:{_s(d)}"'


def rim_circle(r, cls="thin"):
    """A rim circle as short arcs, each drawn as the sweep passes it."""
    n = RIM_LINE_PIECES
    pts = [(r * math.cos(2 * math.pi * k / n), r * math.sin(2 * math.pi * k / n)) for k in range(n + 1)]
    return "".join(
        f'<path class="{cls} swept" pathLength="1"{_swept(k / n, (k + 1) / n)} '
        f'd="M{pts[k][0]:.2f} {pts[k][1]:.2f}A{r} {r} 0 0 1 {pts[k + 1][0]:.2f} {pts[k + 1][1]:.2f}"/>'
        for k in range(n))


def guilloche(r0, w, n):
    """Two interlaced strands around a ring, with a dot in each eye. Each
    strand is cut into pieces that the sweep draws one after another."""
    out = []
    pieces = n * BRAID_PIECES_PER_WAVE
    steps = max(2, 24 // BRAID_PIECES_PER_WAVE)  # line segments per piece
    for phase in (0, math.pi):
        for k in range(pieces):
            pts = []
            for i in range(steps + 1):
                t = 2 * math.pi * (k + i / steps) / pieces
                r = r0 + (w / 2) * math.sin(n * t + phase)
                pts.append(f"{r * math.cos(t):.2f} {r * math.sin(t):.2f}")
            out.append(f'<path class="orn swept" pathLength="1"{_swept(k / pieces, (k + 1) / pieces, BRAID_LAG)} d="M{"L".join(pts)}"/>')
    for k in range(n * 2):
        t = (k + 0.5) * math.pi / n
        frac = (k + 0.5) / (n * 2)
        out.append(f'<circle class="dot"{_swept(frac, frac, BRAID_LAG, BEAD_TIME)} cx="{r0 * math.cos(t):.2f}" cy="{r0 * math.sin(t):.2f}" r="{w * 0.12:.2f}"/>')
    return "".join(out)


def tongues(r_out, r_in, n):
    """A ring of tongues, points inward: the bowl's border."""
    out = []
    for k in range(n):
        a0, a1 = 2 * math.pi * k / n, 2 * math.pi * (k + 1) / n
        am = (a0 + a1) / 2
        p0 = (r_out * math.cos(a0 + 0.02), r_out * math.sin(a0 + 0.02))
        p1 = (r_out * math.cos(a1 - 0.02), r_out * math.sin(a1 - 0.02))
        tip = (r_in * math.cos(am), r_in * math.sin(am))
        c0 = (r_in * 1.02 * math.cos(a0 + 0.02), r_in * 1.02 * math.sin(a0 + 0.02))
        c1 = (r_in * 1.02 * math.cos(a1 - 0.02), r_in * 1.02 * math.sin(a1 - 0.02))
        out.append(f'<path class="orn swept"{_swept(k / n, k / n, BRAID_LAG, TONGUE_TIME)} pathLength="1" d="M{p0[0]:.2f} {p0[1]:.2f}Q{c0[0]:.2f} {c0[1]:.2f} {tip[0]:.2f} {tip[1]:.2f}'
                   f'Q{c1[0]:.2f} {c1[1]:.2f} {p1[0]:.2f} {p1[1]:.2f}"/>')
    return "".join(out)


# The device painted in the field (the episema): a rearing serpent, one of
# the commonest on hoplons in vase painting. Drawn in a ±100 box.
SERPENT = """<g class="device">
<path d="M-62 78C-96 56-84 14-46 18C-8 22 6 50 34 46C70 40 72 0 44-18C24-32 6-40 10-64C13-80 30-88 44-84"
 fill="none" class="body"/>
<path d="M40-92C54-98 72-92 74-80C76-70 64-66 52-70C46-72 40-74 36-80Z" class="head"/>
<path d="M74-80L90-86M90-86L96-92M90-86L97-82" class="tongue"/>
</g>"""


# Athena's owl: arms and wisdom. Frontal, as on her shields and coins.
OWL = """<g class="device">
<path class="f" d="M-38-62L-44-96L-16-70Q0-74 16-70L44-96L38-62Q62-40 60 6C58 56 34 88 0 90C-34 88-58 56-60 6Q-62-40-38-62Z"/>
<circle class="k" cx="-22" cy="-36" r="19"/><circle class="k" cx="22" cy="-36" r="19"/>
<circle class="f" cx="-22" cy="-36" r="8"/><circle class="f" cx="22" cy="-36" r="8"/>
<path class="k" d="M-7-16H7L0 2Z"/>
<path class="kl" d="M-30 22L0 38L30 22M-26 44L0 60L26 44M-18 66L0 78L18 66"/>
<path class="fl" d="M-22 90V100M-14 90V100M14 90V100M22 90V100"/>
</g>"""

OWL = owl.COIN_OWL  # the tetradrachm owl replaces the sketch above


def hoplon(R, device=True, emblem=None):
    """The shield head-on, as a catalogue drawing: rim with a guilloche,
    a band of tongues, the bowl, and the device in the field."""
    # the rim is turned a quarter back, so its lines, the braid and the
    # braid's beads and tongues all start at 12 o'clock and run clockwise
    return ('<g transform="rotate(-90)">'
            + f'<circle r="{R}" class="ground"/>' + rim_circle(R, "open")
            + rim_circle(R * 0.965)
            + guilloche(R * 0.9, R * 0.09, 30)
            + rim_circle(R * 0.835)
            + tongues(R * 0.835, R * 0.77, 44)
            + rim_circle(R * 0.77, "open")
            + "</g>"
            + (f'<g transform="scale({R * 0.0064:.4f})" style="--u:{1 / (R * 0.0064):.4f}">{emblem or OWL}</g>' if device else "")
            # the dome, suggested by hatching on its shadowed side
            + "".join(f'<path class="hatch" d="M{R*0.72*math.cos(a):.1f} {R*0.72*math.sin(a):.1f}'
                      f'A{R*0.72:.1f} {R*0.72:.1f} 0 0 1 {R*0.72*math.cos(a+0.55):.1f} {R*0.72*math.sin(a+0.55):.1f}"'
                      f' transform="scale({s})"/>'
                      for a, s in ((0.15, 1), (0.25, 0.94), (0.35, 0.88)))
            )


def profile(R):
    """The shield's section, face up: flat rim, dished bowl, hatched."""
    h = R * 0.3
    return (f'<path class="cut" pathLength="1" d="M{-R} 0H{-0.8*R}C{-0.62*R} {-h} {-0.3*R} {-1.1*h} 0 {-1.1*h}'
            f'C{0.3*R} {-1.1*h} {0.62*R} {-h} {0.8*R} 0H{R}V{0.05*h}H{0.78*R}C{0.6*R} {-0.9*h} {0.3*R} {-1.0*h} 0 {-1.0*h}'
            f'C{-0.3*R} {-1.0*h} {-0.6*R} {-0.9*h} {-0.78*R} {0.05*h}H{-R}Z"/>'
            f'<path class="axis" d="M0 {-1.35*h}V{0.35*h}"/>')


def meander(x, y, width, u):
    """A running Greek key, `u` to the step, between two border lines. It
    draws in from both ends at once and meets in the middle: each key is its
    own line, started as the border lines reach it."""
    n = int(width // (4 * u))
    step = (KEY_TIME - KEY_UNIT_TIME) / ((n - 1) // 2)
    keys = "".join(
        f'<path class="key unit" pathLength="1" style="animation-delay:{_s(min(i, n - 1 - i) * step)}" '
        f'd="M{x+i*4*u} {y+4*u}V{y}H{x+i*4*u+3*u}V{y+3*u}H{x+i*4*u+u}V{y+u}H{x+i*4*u+2*u}"/>' for i in range(n))
    x1, mid = x + n * 4 * u, x + n * 2 * u
    lines = "".join(
        f'<path class="key" pathLength="1" d="M{x} {ly}H{mid}"{op}/><path class="key" pathLength="1" d="M{x1} {ly}H{mid}"{op}/>'
        for ly, op in ((y + 4 * u, ""), (y - u, ' opacity=".6"'), (y + 5 * u, ' opacity=".6"')))
    return keys + lines


def scale_bar(x, y, px, label):
    """A catalogue scale bar, alternately filled."""
    q = px / 4
    cells = "".join(f'<rect x="{x+i*q}" y="{y}" width="{q}" height="5" class="{"sb-fill" if i % 2 == 0 else "sb-empty"}"/>'
                    for i in range(4))
    return (cells + f'<text x="{x}" y="{y+20}" class="sb">0</text>'
            f'<text x="{x+px}" y="{y+20}" class="sb" text-anchor="end">{label}</text>')


PLATE_CSS = """
.plate{position:absolute;inset:0;width:100%;height:100%;pointer-events:none}
.plate .part{fill:var(--vellum);stroke:var(--line);stroke-width:calc(1.5px * var(--u, 1));stroke-linejoin:round}
.plate .open{fill:none;stroke:var(--line);stroke-width:calc(1.5px * var(--u, 1))}
.plate .thin,.plate .detail{fill:none;stroke:var(--line);stroke-width:calc(1px * var(--u, 1))}
.plate .orn{fill:none;stroke:var(--orn);stroke-width:calc(1px * var(--u, 1));stroke-linejoin:round}
.plate .dot{fill:var(--orn)}
.plate .ground{fill:var(--vellum)}
.plate .hatch{display:none;fill:none;stroke:var(--line);stroke-width:calc(.75px * var(--u, 1));opacity:.6}
.plate .device .body{stroke:var(--orn);stroke-width:13;stroke-linecap:round}
.plate .device .head{fill:var(--orn)}
.plate .device .tongue{fill:none;stroke:var(--orn);stroke-width:2.5;stroke-linecap:round}
.plate .device .f{fill:var(--orn)}.plate .device .ring{fill:none;stroke:var(--orn);stroke-width:1.6}.plate .device .k{fill:var(--vellum)}
.plate .device .kl{fill:none;stroke:var(--vellum);stroke-width:5;stroke-linecap:round;stroke-linejoin:round}
.plate .device .fl{fill:none;stroke:var(--orn);stroke-width:5;stroke-linecap:round}
.plate .device .kl.thin{stroke-width:3}
.plate .device .ol{fill:none;stroke:var(--orn);stroke-width:calc(1.5px * var(--u, 1));stroke-linecap:round;stroke-linejoin:round}
.plate .device .fe{fill:none;stroke:var(--orn);stroke-width:calc(1px * var(--u, 1));stroke-linecap:round}
.plate .device .pf,.plate .device .pfi,.plate .device .sf{fill:var(--orn)}.plate .device .paper,.plate .device .sk{fill:var(--vellum)}
.plate .device .wash{fill:var(--orn);opacity:.13}
.plate .key{fill:none;stroke:var(--orn);stroke-width:calc(1.25px * var(--u, 1));stroke-linejoin:miter}
.plate .cut{fill:url(#phatch);stroke:var(--line);stroke-width:calc(1.5px * var(--u, 1))}
.plate .hl{stroke:var(--line);stroke-width:1;opacity:.5}
.plate .axis{fill:none;stroke:var(--niter);stroke-width:calc(1px * var(--u, 1));stroke-dasharray:18 4 3 4;opacity:.7}
.plate .sb-fill{fill:var(--ink-2)}.plate .sb-empty{fill:none;stroke:var(--ink-2);stroke-width:1}
.plate .sb{font:500 12px var(--font-body);fill:var(--ink-2)}
.plate .no{font:800 30px var(--font-display);fill:var(--ink)}
.plate .cap{font:15px var(--font-body);fill:var(--ink-2)}
.plate .cap tspan.t{font-weight:700;fill:var(--ink)}
.plate .frame{fill:none;stroke:var(--rule-strong);stroke-width:1}
.plate .ptitle{font:600 14px var(--font-display);letter-spacing:.06em;fill:var(--ink-3)}
:root{--orn:#9a6a2c}
:root[data-theme=dark]{--orn:#d3a45f}
"""


def anim_css():
    """The draw-in, timed from the knobs at the top of this file."""
    ease = "cubic-bezier(.2,.7,.2,1)"
    return f"""
@keyframes draw{{to{{stroke-dashoffset:0}}}}
@keyframes fade{{to{{opacity:1}}}}
@keyframes fillin{{to{{fill-opacity:1}}}}
@keyframes wash{{to{{opacity:.13}}}}
/* the Greek key comes first, from both ends to the middle; the rest waits for it */
.anim .key{{stroke-dasharray:1;stroke-dashoffset:1;animation:draw {_s(KEY_TIME)} linear forwards}}
.anim .key.unit{{animation-duration:{_s(KEY_UNIT_TIME)}}}
/* the rifle */
.anim .part,.anim .open,.anim .thin,.anim .detail,.anim .cut{{stroke-dasharray:1;stroke-dashoffset:1;animation:draw {_s(RIFLE_TIME)} {_s(RIFLE_START)} {ease} forwards}}
.anim .part{{fill-opacity:0;animation:draw {_s(RIFLE_TIME)} {_s(RIFLE_START)} {ease} forwards,fillin .9s {_s(RIFLE_START + 0.4)} forwards}}
/* the rim: every piece carries its own delay and duration, set from the sweep */
.anim .swept{{stroke-dasharray:1;stroke-dashoffset:1;animation:draw 1s linear forwards}}
.anim .dot{{opacity:0;animation:fade 1s forwards}}
/* fills fade in as their lines are drawn, so the grid shows until the drawing covers it */
.anim .ground{{fill-opacity:0;animation:fillin {_s(SHIELD_FILL_TIME)} {_s(_sweep_time(SHIELD_FILL_AT))} forwards}}
/* the owl is engraved after the rim: lines first, then its bronze ground, then its eyes */
.anim .device .ol,.anim .device .fe{{stroke-dasharray:1;stroke-dashoffset:1;animation:draw {_s(OWL_LINES_TIME)} {_s(OWL_START)} {ease} forwards}}
.anim .device .paper{{fill-opacity:0;animation:fillin .9s {_s(OWL_PAPER_START)} forwards}}
.anim .device .pfi{{opacity:0;animation:fade {_s(BEAK_FILL_TIME)} {_s(BEAK_FILL_START)} forwards}}
.anim .device .wash{{opacity:0;animation:wash {_s(WASH_TIME)} {_s(WASH_START)} forwards}}
.anim .device .pf{{opacity:0;animation:fade {_s(PUPIL_TIME)} {_s(OWL_START + OWL_LINES_TIME + PUPIL_PAUSE)} ease-in forwards}}
.anim .hatch,.anim .axis,.anim text,.anim rect{{opacity:0;animation:fade {_s(LABELS_TIME)} {_s(LABELS_START)} forwards}}
@media (prefers-reduced-motion:reduce){{.anim *{{animation:none!important;stroke-dasharray:none!important;opacity:1!important;fill-opacity:1!important}}.anim .device .wash{{opacity:.13!important}}}}
"""


PLATE_CSS += anim_css()


HOPLON_R = 140
# Plate layout, in window pixels (1200 × 744 below the top bar).
PLATE = f"""<defs><pattern id="phatch" width="7" height="7" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
<line x1="0" y1="0" x2="0" y2="7" class="hl"/></pattern></defs>
<g transform="translate(880 220)">{hoplon(HOPLON_R)}</g>
{scale_bar(1000, 396, 93, "30 cm")}
<text x="640" y="452" class="no">1</text>
<text x="668" y="445" class="cap"><tspan class="t">Hoplon.</tspan> The Greek hoplite’s shield, bronze over wood.</text>
<text x="668" y="465" class="cap">About 500 BC.</text>
{meander(640, 492, 500, 3.5)}
<g style="--u:.625">{concept1.drawing("rifle", "translate(630 440) scale(1.6)")}</g>
{scale_bar(990, 668, 150, "30 cm")}
<text x="640" y="712" class="no">2</text>
<text x="668" y="705" class="cap"><tspan class="t">Rifle.</tspan> Steel, aluminium and plastic. 1967.</text>
"""


BRAND_MARK = (f'<svg class="mark" viewBox="-110 -110 220 220"><circle r="102" class="rim"/>'
              f'<circle r="84" class="rim2"/><g transform="scale(.66)">{owl.SOLID_OWL}</g></svg>')
BRAND_CSS = """.brand svg.mark{width:30px;height:30px;--o:var(--ink);--p:var(--vellum)}
.mark .rim{fill:none;stroke:var(--ink);stroke-width:12}.mark .rim2{fill:none;stroke:var(--ink);stroke-width:5;stroke-dasharray:9 7}
.mark .device .sf{fill:var(--o)}.mark .device .sk{fill:var(--p)}.mark .device .f{fill:var(--o)}.mark .device .ring{fill:none;stroke:var(--o);stroke-width:1.6}.mark .device .k{fill:var(--p)}
.mark .device .kl{fill:none;stroke:var(--p);stroke-width:6;stroke-linecap:round}.mark .device .fl{fill:none;stroke:var(--o);stroke-width:7;stroke-linecap:round}"""


def split_subpaths(svg):
    """A dash-drawn path made of several pieces draws each piece in a sliver of
    the time, so they pop in. Give every piece of the stroke-only line work its
    own path. Filled parts stay whole, since their pieces may cut holes."""
    import re

    def one(m):
        attrs, d, rest = m.group(1), m.group(2), m.group(3)
        cls = re.search(r'class="([^"]*)"', attrs + rest)
        if not cls or not set(cls.group(1).split()) & {"open", "detail", "thin", "orn", "fe", "ol"}:
            return m.group(0)
        parts = [part for part in re.split(r"(?=M)", d) if part.strip()]
        if len(parts) < 2:
            return m.group(0)
        return "".join(f'<path {attrs}d="{part}"{rest}/>' for part in parts)

    return re.sub(r'<path ([^>]*?)d="([^"]*)"([^>]*?)/>', one, svg)


PLATE = split_subpaths(PLATE)


# Lines are drawn in with pathLength="1" and a dash offset. That only works
# the same in every engine on plain strokes: with vector-effect:
# non-scaling-stroke, Firefox, WebKit and Safari each dash in different units
# (a line in a scaled group drew only partly, or in several places at once).
# So the plate's strokes scale normally, and each scaled group sets --u to
# 1/scale to keep its line weights.
DRAW_JS = ""

REPLAY_JS = """<script>
const pristine = document.querySelector("svg.plate").outerHTML;
document.addEventListener("click", () => { document.querySelector("svg.plate").outerHTML = pristine; });
</script>"""


def chooser(theme, content, anim=False):
    css = build.GRID + ".col{width:560px;padding:40px 0 0 42px;position:relative;z-index:1}" + PLATE_CSS
    svg = f'<svg class="plate{" anim" if anim else ""}" viewBox="0 0 1200 744">{PLATE}</svg>'
    html = build.page(theme, f'<main>{svg}<div class="col">{content}</div></main>{DRAW_JS if anim else ""}', css + BRAND_CSS)
    start = html.index('<div class="brand"><svg'); end = html.index("</svg>", start) + 6
    return html[:start] + '<div class="brand">' + BRAND_MARK + html[end:]


# ── Icons ───────────────────────────────────────────────────────────────
ICON_CSS = """<style>
.ic .part{fill:var(--p);stroke:var(--l);vector-effect:non-scaling-stroke}
.ic .thin,.ic .open{fill:none;stroke:var(--l);vector-effect:non-scaling-stroke}
.ic .orn{fill:none;stroke:var(--o);vector-effect:non-scaling-stroke}
.ic .dot,.ic .device .head{fill:var(--o)}
.ic .device .body{fill:none;stroke:var(--o);stroke-width:13;stroke-linecap:round}
.ic .device .tongue{fill:none;stroke:var(--o);stroke-width:3;stroke-linecap:round}
.ic .hatch{display:none}
.ic .device .f{fill:var(--o)}.ic .device .ring{fill:none;stroke:var(--o);stroke-width:1.6}.ic .device .k{fill:var(--p)}
.ic .device .kl{fill:none;stroke:var(--p);stroke-width:6;stroke-linecap:round;stroke-linejoin:round}
.ic .device .fl{fill:none;stroke:var(--o);stroke-width:5;stroke-linecap:round}
.ic .device .kl.thin{stroke-width:3}
.ic .device .ol{fill:none;stroke:var(--o);stroke-width:var(--ow);vector-effect:non-scaling-stroke;stroke-linecap:round;stroke-linejoin:round}
.ic .device .fe{fill:none;stroke:var(--o);stroke-width:calc(var(--ow) * .6);vector-effect:non-scaling-stroke;stroke-linecap:round}
.ic .device .pf,.ic .device .pfi,.ic .device .sf{fill:var(--o)}.ic .device .paper,.ic .device .sk{fill:var(--p)}.ic .device .wash{fill:var(--o);opacity:.15}
</style>"""


def icon(kind, size, emblem):
    heavy = size <= 32
    sw = 2 if size >= 200 else 1
    if kind == "shield":
        # the ornate shield alone on a blued-steel tile, drawn in bronze
        vars_ = f"--p:#16202b;--l:#e2b565;--o:#e2b565;--ow:{2.4 if size >= 200 else 1.1}px"
        body = (concept1.TILE + f'<g transform="translate(256 256)" style="stroke-width:{sw}px">'
                + (f'<circle r="196" fill="#16202b" stroke="#e2b565" stroke-width="{"34" if size <= 16 else "22"}"/>'
                   f'<circle r="140" fill="none" stroke="#e2b565" stroke-width="10" stroke-dasharray="14 12"/>'
                   f'<g transform="scale(1.25)">{owl.SOLID_OWL}</g>' if heavy else hoplon(196, emblem=emblem))
                + "</g>")
    else:
        # the shield on a museum specimen tag, tied on at the eyelet
        vars_ = "--p:#f4f1e8;--l:#3a2a18;--o:#9a6a2c"
        tag = ('<path d="M150 72H440Q468 72 468 100V412Q468 440 440 440H150L60 348V164Z" fill="#efe9da" stroke="#b9a782" stroke-width="6"/>'
               '<circle cx="104" cy="256" r="18" fill="#0f1419" stroke="#b9a782" stroke-width="6"/>'
               '<path d="M86 256C50 256 36 300 24 340" fill="none" stroke="#c9b27a" stroke-width="8" stroke-linecap="round"/>')
        body = (concept1.TILE.replace('fill="url(#igrid)"', 'fill="none"') + tag
                + f'<g transform="translate(300 256)" style="stroke-width:{sw}px">'
                + (f'<circle r="140" fill="#f4f1e8" stroke="#9a6a2c" stroke-width="24"/>'
                   f'<g transform="scale(.9)">{emblem}</g>' if heavy else hoplon(140, emblem=emblem))
                + "</g>")
    return f'<svg class="ic" style="{vars_}" width="{size}" height="{size}" viewBox="0 0 512 512">{body}</svg>'


if __name__ == "__main__":
    pages = {
        "c2-list-light": chooser("light", build.LIST),
        "c2-first-dark": chooser("dark", build.FIRST),
        # for trying out the knobs: click anywhere to replay
        "c2-animated-dark": chooser("dark", build.FIRST, anim=True).replace("</body>", REPLAY_JS + "</body>"),
        "c2-animated-light": chooser("light", build.LIST, anim=True).replace("</body>", REPLAY_JS + "</body>"),
    }
    for name, html in pages.items():
        (HERE / f"{name}.html").write_text(html)
    sizes = [256, 64, 32, 16]
    rows = ""
    for kind, emblem in (("shield", OWL),):
        rows += '<div class="row">' + "".join(
            f'<div class="bg {bg}">' + "".join(icon(kind, s, emblem) for s in sizes) + "</div>" for bg in ("light", "dark")) + "</div>"
    (HERE / "c2-icons.html").write_text(f"""<!doctype html><html><head><meta charset="utf-8">{ICON_CSS}<style>
*{{margin:0}}body{{background:#d8dbd5;padding:24px;width:1100px;display:grid;gap:16px}}.row{{display:flex;gap:16px}}
.bg{{display:flex;align-items:flex-end;gap:24px;padding:20px 24px;border-radius:10px}}.light{{background:#f3f3f1}}.dark{{background:#2a2d31}}
svg{{display:block}}</style></head><body>{rows}</body></html>""")
    print("ok")
