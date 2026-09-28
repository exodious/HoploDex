"""Athena's owl as on the Athenian tetradrachm, drawn as an engraving: a
little owl (no ear tufts), body in profile facing right, head turned to face
the viewer, and a small olive sprig behind. The coins' crescent is left out:
it's minor there, and alone it's easily read as a political or religious
symbol. Drawn in a ±100 box, y down.

Line work like the rest of the plate, so it can be drawn in stroke by stroke:
ol = outline, fe = feather and vein detail, pf = the few solid marks (pupils,
beak), wash = a faint bronze ground, paper = the field, hiding lines behind."""
import pathlib

HERE = pathlib.Path(__file__).parent

HEAD = "M-40-50C-41-74-28-88 4-88C36-88 47-74 46-50C45-24 28-10 4-10C-20-10-39-24-40-50Z"
BODY = "M-28-20C-50 2-56 38-48 60L-62 90H-40L-30 82C-12 92 22 92 36 72C54 48 52 4 34-22"
BODY_CLOSED = BODY + "Z"


def _scallops(rows, w=6, h=4):
    """Rows of small U-shaped feathers, engraving-style: [(y, x0, x1), …]."""
    out = []
    for i, (y, x0, x1) in enumerate(rows):
        x = x0 + (w * 0.66 if i % 2 else 0)
        while x + w <= x1:
            out.append(f"M{x:.1f} {y}q{w/2} {h} {w} 0")
            x += w * 1.33
    return f'<path class="fe" pathLength="1" d="{"".join(out)}"/>'


def _disc(cx, cy, r):
    """The facial disc: an arc over the eye from lower-inner to lower-outer."""
    import math
    a0, a1 = math.radians(160), math.radians(380)
    return (f"M{cx + r*math.cos(a0):.1f} {cy + r*math.sin(a0):.1f}"
            f"A{r} {r} 0 1 1 {cx + r*math.cos(a1):.1f} {cy + r*math.sin(a1):.1f}")


EYES = [(-15, -52), (23, -52)]

COIN_OWL = f"""<g class="device" transform="translate(20 0)">
<!-- olive sprig: a short twig, two leaves with their midribs, a berry -->
<path class="ol" pathLength="1" d="M-80-34C-72-48-60-60-46-70"/>
<path class="ol" pathLength="1" d="M-52-66C-64-80-62-94-56-100C-48-90-46-76-52-66Z"/>
<path class="ol" pathLength="1" d="M-60-58C-74-60-88-66-92-74C-80-76-66-72-60-58Z"/>
<path class="fe" pathLength="1" d="M-52-66C-54-78-55-88-56-96M-60-58C-70-62-80-67-88-73"/>
<circle class="pf" cx="-80" cy="-34" r="4"/>
<!-- body: ground, outline, wing, feathering -->
<path class="wash" d="{BODY_CLOSED}"/>
<path class="ol" pathLength="1" d="{BODY}"/>
<path class="ol" pathLength="1" d="M-24-8C-6 14-2 44-12 70L-40 88"/>
{_scallops([(8, -44, -18), (18, -48, -14), (28, -50, -12), (38, -50, -12)])}
<path class="fe" pathLength="1" d="M-18 48C-24 62-32 74-44 86M-28 48C-34 62-42 74-52 86M-40 48C-44 62-50 74-58 88"/>
{_scallops([(4, 6, 38), (14, 2, 44), (24, 2, 46), (34, 2, 46), (44, 4, 44), (54, 6, 40), (64, 10, 32)])}
<!-- feet gripping the ground line -->
<path class="ol" pathLength="1" d="M4 84V91M18 84V91"/>
<path class="fe" pathLength="1" d="M4 91L-2 97M4 91V98M4 91L10 97M18 91L12 97M18 91V98M18 91L24 97M-54 98H40"/>
<!-- head: paper first, so the body's lines stop at it -->
<path class="paper" d="{HEAD}"/><path class="wash" d="{HEAD}"/>
<path class="ol" pathLength="1" d="{HEAD}"/>
{_scallops([(-80, -18, 26), (-73, -26, 34)], w=5, h=3)}
<path class="fe" pathLength="1" d="{"".join(_disc(x, y, 20) for x, y in EYES)}"/>
<path class="ol" pathLength="1" d="{"".join(f"M{x+13} {y}A13 13 0 1 1 {x-13} {y}A13 13 0 1 1 {x+13} {y}" for x, y in EYES)}"/>
{"".join(f'<circle class="pf" cx="{x+1}" cy="{y}" r="9"/>' for x, y in EYES)}
<path class="pf" d="M-3-40H9C9-33 7-27 3-21C2-27-1-33-3-40Z"/>
</g>"""

# For 32 px and below, where lines disappear: the same owl as a silhouette,
# with its eyes cut out of it.
SOLID_OWL = f"""<g class="device solid" transform="translate(20 0)">
<path class="sf" d="M-52-66C-64-80-62-94-56-100C-48-90-46-76-52-66ZM-60-58C-74-60-88-66-92-74C-80-76-66-72-60-58Z"/>
<path class="sf" d="{BODY_CLOSED}"/><path class="sf" d="{HEAD}"/>
{"".join(f'<circle class="sk" cx="{x}" cy="{y}" r="15"/><circle class="sf" cx="{x+1}" cy="{y}" r="9"/>' for x, y in EYES)}
</g>"""

CSS = """.device .ol{fill:none;stroke:var(--o);stroke-width:1.6px;vector-effect:non-scaling-stroke;stroke-linecap:round;stroke-linejoin:round}
.device .fe{fill:none;stroke:var(--o);stroke-width:1px;vector-effect:non-scaling-stroke;stroke-linecap:round}
.device .pf{fill:var(--o)}.device .paper{fill:var(--p)}.device .wash{fill:var(--o);opacity:.13}
.device .sf{fill:var(--o)}.device .sk{fill:var(--p)}"""

if __name__ == "__main__":
    cells = "".join(
        f'<div style="--p:{p};--o:{o};background:{p}"><svg width="{s}" height="{s}" viewBox="-110 -110 220 220">'
        f'{SOLID_OWL if s <= 32 else COIN_OWL}</svg></div>'
        for p, o in (("#edefea", "#9a6a2c"), ("#16202b", "#e2b565")) for s in (360, 64, 32))
    (HERE / "owl-test.html").write_text(f"""<!doctype html><meta charset="utf-8"><style>{CSS}
body{{margin:0;display:flex;flex-wrap:wrap;align-items:flex-end;gap:12px;padding:12px;background:#999}}div{{padding:10px}}svg{{display:block}}</style>{cells}""")
