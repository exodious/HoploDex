"""Throwaway icon candidates, shown at bundle sizes on light and dark desktops."""
import pathlib

HERE = pathlib.Path(__file__).parent
FONTS = (HERE / "fonts.css").read_text()

GRID = """<pattern id="g{k}" width="32" height="32" patternUnits="userSpaceOnUse">
<path d="M32 0H0V32" fill="none" stroke="{c}" stroke-width="2"/></pattern>"""


def tile(fill, k, grid=None):
    g = GRID.format(k=k, c=grid) if grid else ""
    over = f'<rect x="16" y="16" width="480" height="480" rx="104" fill="url(#g{k})"/>' if grid else ""
    return f'<defs>{fill}{g}</defs><rect x="16" y="16" width="480" height="480" rx="104" fill="url(#bg{k})"/>{over}'


AXIS = 'stroke-dasharray="64 14 10 14" stroke-linecap="butt"'

ICONS = {
    # A: the brand mark on a blued-steel plate.
    "A · Blued plate": tile(
        '<linearGradient id="bgA" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#1f2b3a"/>'
        '<stop offset="1" stop-color="#0f1419"/></linearGradient>', "A")
    + f"""<g fill="none" stroke="#e6e9ec"><circle cx="256" cy="256" r="172" stroke-width="26"/>
<circle cx="256" cy="256" r="106" stroke-width="18"/></g>
<path d="M40 256H472" stroke="#93a8ff" stroke-width="16" {AXIS}/>""",
    # B: the same mark in ink on drafting vellum, with its grid.
    "B · Vellum sheet": tile(
        '<linearGradient id="bgB" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#f9faf7"/>'
        '<stop offset="1" stop-color="#e3e6e0"/></linearGradient>', "B", grid="rgb(35 68 190/.10)")
    + f"""<g fill="none" stroke="#161c24"><circle cx="256" cy="256" r="172" fill="#edefea" stroke-width="26"/>
<circle cx="256" cy="256" r="106" stroke-width="18"/></g>
<path d="M40 256H472" stroke="#2344be" stroke-width="14" {AXIS}/>""",
    # C: the shield in two views, elevation and section, on one axis.
    "C · Two views": tile(
        '<linearGradient id="bgC" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#1f2b3a"/>'
        '<stop offset="1" stop-color="#0f1419"/></linearGradient>', "C", grid="rgb(147 168 255/.10)")
    + f"""<g fill="none" stroke="#e6e9ec" stroke-linejoin="round"><circle cx="208" cy="256" r="148" stroke-width="22"/>
<circle cx="208" cy="256" r="100" stroke-width="14"/>
<path d="M398 108V142C448 150 468 210 468 256C468 302 448 362 398 370V404" stroke-width="20"/></g>
<path d="M36 256H484" stroke="#93a8ff" stroke-width="12" {AXIS}/>
<path d="M216 108H384M216 404H384" stroke="#93a8ff" stroke-width="5" opacity=".7"/>""",
}

SIZES = [256, 64, 32, 16]


def svg(body, size):
    return f'<svg width="{size}" height="{size}" viewBox="0 0 512 512">{body}</svg>'


rows = ""
for name, body in ICONS.items():
    cells = "".join(
        f'<div class="bg {bg}">' + "".join(svg(body, s) for s in SIZES) + "</div>" for bg in ("light", "dark")
    )
    rows += f'<section><h2>{name}</h2><div class="pair">{cells}</div></section>'

html = f"""<!doctype html><html><head><meta charset="utf-8"><title>Icons</title><style>{FONTS}
*{{margin:0;box-sizing:border-box}}body{{background:#d8dbd5;font-family:"Atkinson Hyperlegible Next Variable";padding:24px;width:1400px}}
h2{{font:700 22px "Big Shoulders Display Variable";letter-spacing:.02em;margin:0 0 8px}}section{{margin-bottom:20px}}
.pair{{display:flex;gap:16px}}.bg{{display:flex;align-items:flex-end;gap:24px;padding:20px 24px;border-radius:10px}}
.light{{background:#f3f3f1}}.dark{{background:#2a2d31}}svg{{display:block;image-rendering:auto}}
</style></head><body>{rows}</body></html>"""
(HERE / "icons.html").write_text(html)
