"""Concept 2, "Catalogue": a museum plate pairing an ornate hoplon with the
app's firearm drawings, each numbered and captioned like catalogue entries."""
import math
import pathlib

import build
import concept1
import owl

HERE = pathlib.Path(__file__).parent


def circle(r, cls="thin"):
    return f'<circle r="{r:.2f}" class="{cls}" pathLength="1"/>'


def guilloche(r0, w, n):
    """Two interlaced strands around a ring, with a dot in each eye."""
    out = []
    for phase in (0, math.pi):
        pts = []
        for i in range(n * 24 + 1):
            t = 2 * math.pi * i / (n * 24)
            r = r0 + (w / 2) * math.sin(n * t + phase)
            pts.append(f"{r * math.cos(t):.2f} {r * math.sin(t):.2f}")
        out.append(f'<path class="orn" pathLength="1" d="M{"L".join(pts)}Z"/>')
    for k in range(n * 2):
        t = (k + 0.5) * math.pi / n
        out.append(f'<circle class="dot" cx="{r0 * math.cos(t):.2f}" cy="{r0 * math.sin(t):.2f}" r="{w * 0.12:.2f}"/>')
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
        out.append(f'<path class="orn" pathLength="1" d="M{p0[0]:.2f} {p0[1]:.2f}Q{c0[0]:.2f} {c0[1]:.2f} {tip[0]:.2f} {tip[1]:.2f}'
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
    return (f'<circle r="{R}" class="part" pathLength="1"/>'
            + circle(R * 0.965)
            + guilloche(R * 0.9, R * 0.09, 30)
            + circle(R * 0.835)
            + tongues(R * 0.835, R * 0.77, 44)
            + circle(R * 0.77, "open")
            + (f'<g transform="scale({R * 0.0064:.4f})">{emblem or OWL}</g>' if device else "")
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


KEY_TIME = 1.25  # s: the Greek key is drawn first, then everything else
UNIT_TIME = 0.4  # s: one key unit


def meander(x, y, width, u):
    """A running Greek key, `u` to the step, between two border lines. It
    draws in from both ends at once and meets in the middle: each key is its
    own line, started as the border lines reach it."""
    n = int(width // (4 * u))
    step = (KEY_TIME - UNIT_TIME) / ((n - 1) // 2)
    keys = "".join(
        f'<path class="key unit" pathLength="1" style="animation-delay:{min(i, n - 1 - i) * step:.3f}s" '
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
.plate .part{fill:var(--vellum);stroke:var(--line);stroke-width:1.5px;vector-effect:non-scaling-stroke;stroke-linejoin:round}
.plate .open{fill:none;stroke:var(--line);stroke-width:1.5px;vector-effect:non-scaling-stroke}
.plate .thin,.plate .detail{fill:none;stroke:var(--line);stroke-width:1px;vector-effect:non-scaling-stroke}
.plate .orn{fill:none;stroke:var(--orn);stroke-width:1px;vector-effect:non-scaling-stroke;stroke-linejoin:round}
.plate .dot{fill:var(--orn)}
.plate .hatch{display:none;fill:none;stroke:var(--line);stroke-width:.75px;vector-effect:non-scaling-stroke;opacity:.6}
.plate .device .body{stroke:var(--orn);stroke-width:13;stroke-linecap:round}
.plate .device .head{fill:var(--orn)}
.plate .device .tongue{fill:none;stroke:var(--orn);stroke-width:2.5;stroke-linecap:round}
.plate .device .f{fill:var(--orn)}.plate .device .ring{fill:none;stroke:var(--orn);stroke-width:1.6}.plate .device .k{fill:var(--vellum)}
.plate .device .kl{fill:none;stroke:var(--vellum);stroke-width:5;stroke-linecap:round;stroke-linejoin:round}
.plate .device .fl{fill:none;stroke:var(--orn);stroke-width:5;stroke-linecap:round}
.plate .device .kl.thin{stroke-width:3}
.plate .device .ol{fill:none;stroke:var(--orn);stroke-width:1.5px;vector-effect:non-scaling-stroke;stroke-linecap:round;stroke-linejoin:round}
.plate .device .fe{fill:none;stroke:var(--orn);stroke-width:1px;vector-effect:non-scaling-stroke;stroke-linecap:round}
.plate .device .pf,.plate .device .sf{fill:var(--orn)}.plate .device .paper,.plate .device .sk{fill:var(--vellum)}
.plate .device .wash{fill:var(--orn);opacity:.13}
.plate .key{fill:none;stroke:var(--orn);stroke-width:1.25px;vector-effect:non-scaling-stroke;stroke-linejoin:miter}
.plate .cut{fill:url(#phatch);stroke:var(--line);stroke-width:1.5px;vector-effect:non-scaling-stroke}
.plate .hl{stroke:var(--line);stroke-width:1;opacity:.5}
.plate .axis{fill:none;stroke:var(--niter);stroke-width:1px;stroke-dasharray:18 4 3 4;opacity:.7;vector-effect:non-scaling-stroke}
.plate .sb-fill{fill:var(--ink-2)}.plate .sb-empty{fill:none;stroke:var(--ink-2);stroke-width:1}
.plate .sb{font:500 12px var(--font-body);fill:var(--ink-2)}
.plate .no{font:800 30px var(--font-display);fill:var(--ink)}
.plate .cap{font:15px var(--font-body);fill:var(--ink-2)}
.plate .cap tspan.t{font-weight:700;fill:var(--ink)}
.plate .frame{fill:none;stroke:var(--rule-strong);stroke-width:1}
.plate .ptitle{font:600 14px var(--font-display);letter-spacing:.06em;fill:var(--ink-3)}
:root{--orn:#9a6a2c}
:root[data-theme=dark]{--orn:#d3a45f}
.anim .part,.anim .open,.anim .thin,.anim .detail,.anim .orn,.anim .cut{stroke-dasharray:1;stroke-dashoffset:1;animation:draw 1.6s 1.3s cubic-bezier(.2,.7,.2,1) forwards}
.anim .orn{animation-delay:1.65s}
.anim .dot,.anim .hatch,.anim .axis,.anim text,.anim rect{opacity:0;animation:fade .6s 2.6s forwards}
@keyframes draw{to{stroke-dashoffset:0}}
@keyframes fade{to{opacity:1}}
/* the owl is engraved after the rim: lines first, then its eyes and bronze ground */
.anim .device .ol,.anim .device .fe{stroke-dasharray:1;stroke-dashoffset:1;animation:draw 1.8s 2.2s cubic-bezier(.2,.7,.2,1) forwards}
.anim .device .pf{opacity:0;animation:fade .6s 3.9s forwards}
/* the Greek key comes first, from both ends to the middle; the rest waits for it */
.anim .key{stroke-dasharray:1;stroke-dashoffset:1;animation:draw 1.25s linear forwards}
.anim .key.unit{animation-duration:.4s}
.anim .device .wash{opacity:0;animation:wash .8s 3.6s forwards}
@keyframes wash{to{opacity:.13}}
@media (prefers-reduced-motion:reduce){.anim *{animation:none!important;stroke-dasharray:none!important;opacity:1!important}.anim .device .wash{opacity:.13!important}}
"""

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
{concept1.drawing("rifle", "translate(630 440) scale(1.6)")}
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


# pathLength="1" measures a line in its own units, but a non-scaling stroke is
# dashed in screen units: inside the rifle's 1.6x group the "hidden" dash
# covered only ~60% of each line. So measure every drawn line on screen and
# dash it by that length; the CSS keyframes then run the offset down to 0.
DRAW_JS = """<script>
function measureDrawing(svg) {
  svg.querySelectorAll("[pathLength]").forEach((el) => {
    const m = el.getScreenCTM();
    const len = el.getTotalLength() * Math.hypot(m.a, m.b) + 2;
    el.removeAttribute("pathLength");
    el.style.strokeDasharray = len;
    el.style.strokeDashoffset = len;
  });
}
document.querySelectorAll("svg.anim").forEach(measureDrawing);
</script>"""


def chooser(theme, content, anim=False):
    css = ".col{width:560px;padding:40px 0 0 42px;position:relative;z-index:1}" + PLATE_CSS
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
.ic .device .pf,.ic .device .sf{fill:var(--o)}.ic .device .paper,.ic .device .sk{fill:var(--p)}.ic .device .wash{fill:var(--o);opacity:.15}
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
