"""Concept 1, "Covered": a bronze hoplon in front of the app's own line drawings."""
import json
import pathlib

import build

HERE = pathlib.Path(__file__).parent
DRAWINGS = json.loads((HERE / "drawings.json").read_text())

# Bronze, lit from the upper left. Hoplons were faced with it; cartridges are brass.
B = {
    "edge": "#5a3818",  # the rim's back edge, turned away
    "rim": "#a8702f",
    "rimlight": "#e2b565",
    "shadow": "#8a5824",
    "base": "#c08a42",
    "light": "#e3b86c",
    "spec": "#fbe7bd",
    "line": "#3b2410",
}


def shield(cx, cy, R, uid, cls="shield"):
    """A hoplon at three-quarters, face toward the viewer's left. The dome
    bulges toward the viewer, so it sits left of centre: a sliver of rim on
    the left, a wide band on the right. The rim's thickness shows beyond it."""
    rx, bx, by = 0.7 * R, 0.56 * R, 0.84 * R
    dx = cx - 0.1 * R  # the dome's centre
    return f"""<g class="{cls}">
<defs><clipPath id="bowl{uid}"><ellipse cx="{dx}" cy="{cy}" rx="{bx}" ry="{by}"/></clipPath>
<clipPath id="rim{uid}"><ellipse cx="{cx}" cy="{cy}" rx="{rx}" ry="{R}"/></clipPath></defs>
<ellipse class="cast" cx="{cx+0.16*R}" cy="{cy+0.06*R}" rx="{rx}" ry="{R}"/>
<ellipse cx="{cx+0.08*R}" cy="{cy}" rx="{rx}" ry="{R}" fill="{B['edge']}"/>
<ellipse cx="{cx}" cy="{cy}" rx="{rx}" ry="{R}" fill="{B['rim']}"/>
<g clip-path="url(#rim{uid})"><ellipse cx="{cx-0.03*R}" cy="{cy+0.02*R}" rx="{rx}" ry="{R}" fill="none"
 stroke="{B['rimlight']}" stroke-width="{0.05*R}" opacity=".9"/></g>
<ellipse cx="{dx}" cy="{cy}" rx="{bx}" ry="{by}" fill="{B['shadow']}"/>
<g clip-path="url(#bowl{uid})">
<ellipse cx="{dx-0.1*R}" cy="{cy-0.06*R}" rx="{bx}" ry="{by}" fill="{B['base']}"/>
<ellipse cx="{dx-0.16*R}" cy="{cy-0.28*R}" rx="{0.2*R}" ry="{0.34*R}" fill="{B['light']}"
 transform="rotate(16 {dx-0.16*R} {cy-0.28*R})"/>
<ellipse cx="{dx-0.22*R}" cy="{cy-0.4*R}" rx="{0.06*R}" ry="{0.12*R}" fill="{B['spec']}"
 transform="rotate(16 {dx-0.22*R} {cy-0.4*R})"/>
</g>
<ellipse cx="{dx}" cy="{cy}" rx="{bx}" ry="{by}" fill="none" stroke="{B['line']}" stroke-opacity=".3" stroke-width="{0.012*R}"/>
<ellipse cx="{cx}" cy="{cy}" rx="{rx}" ry="{R}" fill="none" stroke="{B['line']}" stroke-opacity=".55" stroke-width="{0.014*R}"/>
</g>"""


def drawing(key, transform, cls="dwg-gun", solid=False):
    d = DRAWINGS[key]
    out = []
    for p in d["parts"]:
        if solid and p["role"] == "detail":
            continue
        role = "part" if solid else p["role"]
        if "circle" in p:
            x, y, r = p["circle"]
            out.append(f'<circle cx="{x}" cy="{y}" r="{r}" class="{role}" pathLength="1"/>')
        else:
            out.append(f'<path d="{p["d"]}" class="{role}" pathLength="1"/>')
    x0, y, x1 = d["axis"]
    if not solid:
        out.append(f'<path d="M{x0} {y}H{x1}" class="axis"/>')
    return f'<g class="{cls}" transform="{transform}">{"".join(out)}</g>'


# ── Icon ────────────────────────────────────────────────────────────────
TILE = """<defs><linearGradient id="steel" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#223041"/>
<stop offset="1" stop-color="#0f1419"/></linearGradient>
<pattern id="igrid" width="32" height="32" patternUnits="userSpaceOnUse"><path d="M32 0H0V32" fill="none" stroke="rgb(147 168 255/.09)" stroke-width="2"/></pattern></defs>
<rect x="16" y="16" width="480" height="480" rx="104" fill="url(#steel)"/>
<rect x="16" y="16" width="480" height="480" rx="104" fill="url(#igrid)"/>"""

RIFLE_T = "translate(262 246) rotate(-34) scale(1.5) translate(-160 -89)"


def icon(size, uid):
    small = size <= 32
    stroke = 2.2 if size >= 200 else 1.1
    css = f"""<style>
#i{uid} .part{{fill:#1b2633;stroke:#cdd5de;stroke-width:{stroke}px;vector-effect:non-scaling-stroke;stroke-linejoin:round}}
#i{uid} .open,#i{uid} .detail{{fill:none;stroke:#cdd5de;stroke-width:{stroke*0.7}px;vector-effect:non-scaling-stroke}}
#i{uid} .axis{{stroke:#93a8ff;stroke-width:{stroke*0.8}px;vector-effect:non-scaling-stroke;stroke-dasharray:{size/20} {size/80} {size/160} {size/80}}}
#i{uid} .solid .part{{fill:#cdd5de;stroke:none}}
#i{uid} .cast{{fill:#05080b;opacity:.45}}</style>"""
    gun = drawing("rifle", RIFLE_T, "solid" if small else "dwg-gun", solid=small)
    return (f'<svg id="i{uid}" width="{size}" height="{size}" viewBox="0 0 512 512">{css}{TILE}{gun}'
            f'{shield(214, 292, 164 if small else 150, uid)}</svg>')


# ── Chooser ─────────────────────────────────────────────────────────────
CHOOSER_CSS = build.GRID + """
.col{width:560px;padding:40px 0 0 42px}
.art{position:absolute;inset:0;width:100%;height:100%;pointer-events:none;overflow:visible}
.art .part{fill:var(--vellum);stroke:var(--line);stroke-width:1.5px;vector-effect:non-scaling-stroke;stroke-linejoin:round}
.art .open{fill:none;stroke:var(--line);stroke-width:1.5px;vector-effect:non-scaling-stroke}
.art .detail{fill:none;stroke:var(--line);stroke-width:1px;vector-effect:non-scaling-stroke}
.art .axis{fill:none;stroke:var(--niter);stroke-width:1px;stroke-dasharray:18 4 3 4;opacity:.7;vector-effect:non-scaling-stroke}
.art .cast{fill:var(--ink);opacity:.07}
:root[data-theme=dark] .art .cast{fill:#000;opacity:.45}
/* launch: the record draws in, then the shield swings round in front of it */
.anim .part,.anim .open,.anim .detail{stroke-dasharray:1;stroke-dashoffset:1;animation:draw 1.3s cubic-bezier(.2,.7,.2,1) forwards}
.anim .axis{opacity:0;animation:axis .5s 1s forwards}
.anim .shield{transform-box:fill-box;transform-origin:50% 50%;opacity:0;
  animation:swing .85s 1.05s cubic-bezier(.2,.8,.2,1) forwards}
@keyframes draw{to{stroke-dashoffset:0}}
@keyframes axis{to{opacity:.7}}
@keyframes swing{0%{opacity:0;transform:scaleX(.06)}25%{opacity:1}100%{opacity:1;transform:scaleX(1)}}
@media (prefers-reduced-motion:reduce){.anim *{animation:none!important;stroke-dasharray:none!important}.anim .shield{opacity:1}.anim .axis{opacity:.7}}
"""

ART = (drawing("rifle", "translate(600 110) scale(1.78)")
       + drawing("shotgun", "translate(596 330) scale(1.78)")
       + drawing("other", "translate(880 520) scale(0.8)")
       + shield(752, 330, 178, "c"))


def chooser(theme, content, anim=False):
    svg = f'<svg class="art{" anim" if anim else ""}" viewBox="0 0 1200 744">{ART}</svg>'
    return build.page(theme, f'<main>{svg}<div class="col">{content}</div></main>', CHOOSER_CSS)


if __name__ == "__main__":
    pages = {
        "c1-list-light": chooser("light", build.LIST),
        "c1-first-dark": chooser("dark", build.FIRST),
        "c1-list-dark": chooser("dark", build.LIST),
        "c1-first-light": chooser("light", build.FIRST),
        "c1-anim-light": chooser("light", build.FIRST, anim=True),
    }
    for name, html in pages.items():
        (HERE / f"{name}.html").write_text(html)
    sizes = [256, 64, 32, 16]
    strip = lambda bg: f'<div class="bg {bg}">' + "".join(icon(s, f"{bg}{s}") for s in sizes) + "</div>"
    (HERE / "c1-icons.html").write_text(f"""<!doctype html><html><head><meta charset="utf-8"><style>
*{{margin:0}}body{{background:#d8dbd5;padding:24px;display:flex;gap:16px;width:1100px}}
.bg{{display:flex;align-items:flex-end;gap:24px;padding:20px 24px;border-radius:10px}}.light{{background:#f3f3f1}}.dark{{background:#2a2d31}}
svg{{display:block}}</style></head><body>{strip("light")}{strip("dark")}</body></html>""")
    print("ok")
