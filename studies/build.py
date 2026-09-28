"""Builds the throwaway chooser and icon mock-ups (not project code)."""
import pathlib

HERE = pathlib.Path(__file__).parent
FONTS = (HERE / "fonts.css").read_text()

TOKENS = """
:root{--vellum:#edefea;--sheet:#f9faf7;--sheet-sunk:#e3e6e0;--ink:#161c24;--ink-2:#4f5965;--ink-3:#646d78;
--rule:#d3d7d0;--rule-strong:#858c86;--niter:#2344be;--niter-wash:#e2e7fa;--on-ink:#f9faf7;
--grid-minor:rgb(35 68 190/.045);--grid-major:rgb(35 68 190/.085);--line:rgb(79 89 101/.62);--hatch:rgb(79 89 101/.35);
--shadow-raised:0 1px 2px rgb(16 22 30/.06),0 4px 16px -6px rgb(16 22 30/.12);
--font-display:"Big Shoulders Display Variable",sans-serif;--font-body:"Atkinson Hyperlegible Next Variable",sans-serif}
:root[data-theme=dark]{--vellum:#0f1419;--sheet:#161d25;--sheet-sunk:#0b0f14;--ink:#e6e9ec;--ink-2:#a8b1bb;--ink-3:#8f99a5;
--rule:#28313b;--rule-strong:#67727d;--niter:#93a8ff;--niter-wash:#1c2647;--on-ink:#0f1419;
--grid-minor:rgb(147 168 255/.035);--grid-major:rgb(147 168 255/.07);--line:rgb(168 177 187/.5);--hatch:rgb(168 177 187/.25);
--shadow-raised:0 1px 2px rgb(0 0 0/.4),0 4px 16px -6px rgb(0 0 0/.5)}
*{box-sizing:border-box;margin:0}
html,body{height:100%}
body{background:var(--vellum);color:var(--ink);font:15px/1.5 var(--font-body);width:1200px;height:800px;overflow:hidden}
.topbar{height:56px;border-bottom:1px solid var(--rule);display:flex;align-items:center;padding:0 42px;justify-content:space-between;
  background:color-mix(in srgb,var(--vellum) 92%,transparent);position:relative;z-index:2}
.brand{display:flex;align-items:center;gap:10px}
.brand svg{width:26px;height:26px;fill:none;stroke:currentColor;stroke-width:1.6}
.brand .axis{stroke:var(--niter);stroke-width:1.1;stroke-dasharray:5 1.5 1 1.5}
.brand span{font:800 1.5rem/1 var(--font-display);letter-spacing:.02em}
.seg{width:120px;height:34px;border:1px solid var(--rule-strong);border-radius:6px}
main{position:relative;height:744px;overflow:hidden}
h1{font:800 2.25rem/1.05 var(--font-display);letter-spacing:.01em}
.welcome{max-width:52ch;font-size:17px;color:var(--ink-2)}
.row{border:1px solid var(--rule);border-radius:6px;background:var(--sheet);padding:12px 16px;display:flex;gap:12px}
.row.sel{border-color:var(--niter);box-shadow:0 0 0 1px var(--niter),var(--shadow-raised);flex-direction:column}
.row b{font-size:17px}.row small{display:block;font-size:13.5px;color:var(--ink-2)}
.field{display:flex;gap:12px;align-items:flex-end;padding-left:30px}
.field label{display:block;font-size:13.5px;font-weight:700;margin-bottom:8px}
.input{flex:1;height:40px;border:1px solid var(--rule-strong);border-radius:6px;background:var(--sheet)}
.btn{height:40px;padding:0 16px;border-radius:6px;border:1px solid var(--rule-strong);display:inline-flex;align-items:center;
  font-weight:700;background:var(--sheet);gap:10px}
.btn.primary{background:var(--ink);color:var(--on-ink);border-color:var(--ink)}
.btn.large{height:52px;font-size:17px;width:360px}
.actions{display:flex;gap:12px;flex-wrap:wrap}.actions.large{flex-direction:column}
.col{position:relative;z-index:1;display:flex;flex-direction:column;gap:24px}

/* the drawing */
.dwg{position:absolute;inset:0;width:100%;height:100%;pointer-events:none}
.dwg .part{fill:var(--vellum);stroke:var(--line);stroke-width:1.5px;vector-effect:non-scaling-stroke}
.dwg .open{fill:none;stroke:var(--line);stroke-width:1.5px;vector-effect:non-scaling-stroke}
.dwg .thin{fill:none;stroke:var(--line);stroke-width:.75px;vector-effect:non-scaling-stroke}
.dwg .cut{fill:url(#hatch);stroke:var(--line);stroke-width:1.5px;vector-effect:non-scaling-stroke}
.dwg .axis{fill:none;stroke:var(--niter);stroke-width:1px;stroke-dasharray:18 4 3 4;opacity:.7;vector-effect:non-scaling-stroke}
.dwg .dim{fill:none;stroke:var(--niter);stroke-width:.75px;opacity:.75;vector-effect:non-scaling-stroke}
.dwg .arrow{fill:var(--niter);opacity:.75}
.dwg text{font-family:var(--font-display);font-weight:600;fill:var(--niter);opacity:.85;letter-spacing:.04em}
.dwg .hatchline{stroke:var(--hatch);stroke-width:1}
/* one draw-in on launch (issue 23) */
.anim .part,.anim .open,.anim .cut,.anim .thin{stroke-dasharray:1;stroke-dashoffset:1;animation:draw 1.6s cubic-bezier(.2,.7,.2,1) forwards}
.anim .late{animation-delay:.5s}
.anim .axis,.anim .dim,.anim .arrow,.anim text{opacity:0;animation:fade .6s 1.4s forwards}
.anim .cut{fill-opacity:0;animation:draw 1.6s .5s cubic-bezier(.2,.7,.2,1) forwards,fill .5s 1.6s forwards}
@keyframes draw{to{stroke-dashoffset:0}}
@keyframes fade{to{opacity:.75}}
@keyframes fill{to{fill-opacity:1}}
@media (prefers-reduced-motion:reduce){.anim *{animation:none!important;stroke-dasharray:none!important;opacity:.75;fill-opacity:1}}
"""

BRAND = """<div class="brand"><svg viewBox="0 0 28 28"><circle cx="14" cy="14" r="11.5"/><circle cx="14" cy="14" r="7"/>
<path d="M1 14h26" class="axis"/></svg><span>HoploDex</span></div>"""


def arrow(x, y, angle):
    return f'<path class="arrow" d="M0 0L-26 -7L-26 7Z" transform="translate({x} {y}) rotate({angle})"/>'


# The hoplon in two views, in millimetres: front elevation centred on the
# origin (Ø900, a 60 mm rim), and a section through the vertical centreline
# at x=560 showing the dished bowl, the flat rim, and the porpax (arm band).
HOPLON = f"""
<defs><pattern id="hatch" width="14" height="14" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
<line x1="0" y1="0" x2="0" y2="14" class="hatchline" stroke-width="3"/></pattern></defs>
<!-- projection lines -->
<path class="thin" d="M20 -450H540M20 450H540"/>
<!-- elevation -->
<circle class="part" r="450" pathLength="1"/>
<circle class="part" r="390" pathLength="1"/>
<circle class="thin late" r="372" pathLength="1"/>
<circle class="thin late" r="16" pathLength="1"/>
<!-- section A-A -->
<path class="cut" pathLength="1" d="M560 -450V-390C662 -380 712 -205 712 0C712 205 662 380 560 390V450H546V378
 C646 368 698 196 698 0C698 -196 646 -368 546 -378Z"/>
<path class="open late" pathLength="1" d="M698 -55H668V55H698"/>
<!-- centrelines -->
<path class="axis" d="M-520 0H790M0 -520V520M553 -500V500"/>
<!-- cutting plane A-A on the elevation -->
<path class="dim" d="M0 -560V-520M0 520V560M0 -560H60M0 560H60"/>
{arrow(76, -560, 0)}{arrow(76, 560, 0)}
<text x="92" y="-548" font-size="38">A</text><text x="92" y="572" font-size="38">A</text>
<!-- Ø900 -->
<path class="dim" d="M-450 20V560M450 20V560M-424 540H424"/>
{arrow(-450, 540, 180)}{arrow(450, 540, 0)}
<text x="0" y="528" font-size="40" text-anchor="middle">Ø 900</text>
<!-- depth -->
<path class="dim" d="M560 -470V-600M712 -20V-600M586 -580H686"/>
{arrow(560, -580, 180)}{arrow(712, -580, 0)}
<text x="636" y="-594" font-size="40" text-anchor="middle">152</text>
<text x="636" y="640" font-size="36" text-anchor="middle">SECTION A–A</text>
"""


def page(theme, body, extra_css=""):
    return f"""<!doctype html><html data-theme="{theme}"><head><meta charset="utf-8"><title>mock</title>
<style>{FONTS}{TOKENS}{extra_css}</style></head><body>
<header class="topbar">{BRAND}<div class="seg"></div></header>{body}</body></html>"""


LIST = """<h1>Open a database</h1>
<div class="row sel"><div><b>Main collection</b><small>~/Documents/HoploDex</small></div>
<div class="field"><div style="flex:1"><label>Passphrase for “Main collection”</label><div class="input"></div></div>
<span class="btn primary">Open</span></div></div>
<div class="row" style="margin-top:-16px"><div><b>Shared collection</b><small>~/Documents/HoploDex</small></div></div>
<div class="actions"><span class="btn">+&nbsp; Create a new database…</span><span class="btn">Open another database file…</span></div>"""

FIRST = """<h1>Welcome to HoploDex</h1>
<p class="welcome">HoploDex keeps your collection in an encrypted database file that only your passphrase opens.</p>
<div class="actions large"><span class="btn primary large">+&nbsp; Create a new database…</span>
<span class="btn large">Open another database file…</span></div>"""

GRID = """main{background-image:
 linear-gradient(var(--grid-major) 1px,transparent 1px),linear-gradient(90deg,var(--grid-major) 1px,transparent 1px),
 linear-gradient(var(--grid-minor) 1px,transparent 1px),linear-gradient(90deg,var(--grid-minor) 1px,transparent 1px);
 background-size:100px 100px,100px 100px,20px 20px,20px 20px;background-position:-1px -1px}"""


# Direction A: the construction drawing. Content moves to the left, on the
# brand's edge; the hoplon in two views fills the right.
def direction_a(theme, content, anim=False):
    css = GRID + ".col{width:560px;padding:40px 0 0 42px}"
    svg = (f'<svg class="dwg{" anim" if anim else ""}" viewBox="0 0 1200 744">'
           f'<g transform="translate(806 386) scale(0.47)">{HOPLON}</g></svg>')
    return page(theme, f'<main>{svg}<div class="col">{content}</div></main>', css)


# Direction B: the drawing sheet. Content stays centred; the window gets a
# sheet border with zone references and a title block.
def direction_b(theme, content):
    css = GRID + """.col{width:720px;margin:0 auto;padding:40px 32px 0}
.frame{position:absolute;inset:14px 14px 14px 14px;border:1px solid var(--rule-strong);pointer-events:none}
.frame::before{content:"";position:absolute;inset:18px;border:1.5px solid var(--line)}
.zones{position:absolute;display:flex;justify-content:space-around;font:600 13px var(--font-display);color:var(--ink-3)}
.zones.h{left:32px;right:32px;height:18px;align-items:center}.zones.v{top:32px;bottom:32px;width:18px;flex-direction:column;align-items:center}
.tb{position:absolute;right:32px;bottom:32px;display:grid;grid-template-columns:64px 170px 90px;grid-template-rows:34px 34px;
 border-left:1.5px solid var(--line);border-top:1.5px solid var(--line);background:var(--vellum);z-index:0}
.tb>div{border-right:1px solid var(--rule-strong);border-bottom:1px solid var(--rule-strong);padding:3px 8px;font-size:12px;color:var(--ink-2);line-height:1.2}
.tb>div:nth-child(3n){border-right:0}.tb>div:nth-child(n+4){border-bottom:0}
.tb .mark{grid-row:span 2;display:grid;place-items:center;border-bottom:0!important}
.tb .mark svg{width:40px;height:40px;fill:none;stroke:var(--ink);stroke-width:1.2}
.tb .mark .axis{stroke:var(--niter);stroke-dasharray:5 1.5 1 1.5}
.tb .name{font:800 22px/28px var(--font-display);color:var(--ink);letter-spacing:.02em}
.tb b{display:block;font-size:10px;font-weight:700;color:var(--ink-3)}"""
    nums = "".join(f"<span>{n}</span>" for n in range(8, 0, -1))
    lets = "".join(f"<span>{c}</span>" for c in "EDCBA")
    frame = f"""<div class="frame">
<div class="zones h" style="top:0">{nums}</div><div class="zones h" style="bottom:0">{nums}</div>
<div class="zones v" style="left:0">{lets}</div><div class="zones v" style="right:0">{lets}</div></div>
<div class="tb"><div class="mark"><svg viewBox="0 0 28 28"><circle cx="14" cy="14" r="11.5"/><circle cx="14" cy="14" r="7"/>
<path d="M1 14h26" class="axis"/></svg></div><div class="name">HoploDex</div><div><b>Version</b>0.1.0</div>
<div><b>Stored</b>On this computer only</div><div><b>Sheet</b>1 of 1</div></div>"""
    return page(theme, f'<main>{frame}<div class="col">{content}</div></main>', css)


pages = {
    "a-list-light": direction_a("light", LIST),
    "a-first-dark": direction_a("dark", FIRST),
    "a-list-dark": direction_a("dark", LIST),
    "a-first-light": direction_a("light", FIRST),
    "a-first-light-animated": direction_a("light", FIRST, anim=True),
    "b-list-light": direction_b("light", LIST),
    "b-first-dark": direction_b("dark", FIRST),
}
for name, html in pages.items():
    (HERE / f"{name}.html").write_text(html)
print("\n".join(pages))
