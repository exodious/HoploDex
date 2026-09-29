"""Builds the one-page gallery of the mock-ups for the user to look at."""
import base64
import pathlib
import re

import build
import concept1
import concept2

HERE = pathlib.Path(__file__).parent
FONTS = (HERE / "fonts.css").read_text()


def img(name, alt):
    data = base64.b64encode((HERE / "shots" / f"{name}.png").read_bytes()).decode()
    return f'<img src="data:image/png;base64,{data}" alt="{alt}" width="1200" height="800">'


# The drawing's own CSS, lifted from the mock and re-rooted on the gallery's tokens.
ART_CSS = "\n".join(line for line in concept1.CHOOSER_CSS.splitlines() if line.startswith((".art", ".anim", "@keyframes", "@media (prefers-reduced", ":root[data-theme=dark] .art", "  animation")))
DWG_CSS = "\n".join(
    line for line in build.TOKENS.splitlines() if line.startswith((".dwg", ".anim", "@keyframes", "@media (prefers-reduced"))
)

page = f"""<title>HoploDex Chooser Studies</title>
<style>{FONTS}
:root{{--vellum:#edefea;--sheet:#f9faf7;--ink:#161c24;--ink-2:#4f5965;--rule:#d3d7d0;--niter:#2344be;
--cast:rgb(22 28 36/.07);--line:rgb(79 89 101/.62);--hatch:rgb(79 89 101/.35);
--grid-minor:rgb(35 68 190/.045);--grid-major:rgb(35 68 190/.085);
--display:"Big Shoulders Display Variable","Arial Narrow",sans-serif;--body:"Atkinson Hyperlegible Next Variable",system-ui,sans-serif}}
@media (prefers-color-scheme:dark){{:root:not([data-theme="light"]){{color-scheme:dark;--vellum:#0f1419;--sheet:#161d25;--ink:#e6e9ec;
--ink-2:#a8b1bb;--rule:#28313b;--niter:#93a8ff;--cast:rgb(0 0 0/.45);--line:rgb(168 177 187/.5);--hatch:rgb(168 177 187/.25);
--grid-minor:rgb(147 168 255/.035);--grid-major:rgb(147 168 255/.07)}}}}
:root[data-theme="dark"]{{color-scheme:dark;--vellum:#0f1419;--sheet:#161d25;--ink:#e6e9ec;--ink-2:#a8b1bb;--rule:#28313b;
--niter:#93a8ff;--cast:rgb(0 0 0/.45);--line:rgb(168 177 187/.5);--hatch:rgb(168 177 187/.25);--grid-minor:rgb(147 168 255/.035);--grid-major:rgb(147 168 255/.07)}}
body{{background:var(--vellum);color:var(--ink);font:16px/1.55 var(--body)}}
.wrap{{max-width:1240px;margin:0 auto;padding-inline:20px;padding-block:32px 64px;display:grid;gap:48px}}
h1{{font:800 clamp(2rem,5vw,3rem)/1 var(--display);letter-spacing:.01em;text-wrap:balance}}
h2{{font:700 1.75rem/1.1 var(--display);letter-spacing:.01em;padding-bottom:6px;border-bottom:1px solid var(--ink);margin-bottom:12px}}
p{{max-width:68ch;color:var(--ink-2);margin:0 0 12px}}
header p{{margin-top:12px}}
.pair{{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,480px),1fr));gap:16px}}
figure{{margin:0;display:grid;gap:6px}}figcaption{{font-size:14px;color:var(--ink-2)}}
img{{display:block;width:100%;height:auto;border:1px solid var(--rule);border-radius:6px}}
.live{{position:relative;border:1px solid var(--rule);border-radius:6px;overflow:hidden;aspect-ratio:580/700;width:min(100%,480px);
background-color:var(--vellum);background-image:
 linear-gradient(var(--grid-major) 1px,transparent 1px),linear-gradient(90deg,var(--grid-major) 1px,transparent 1px),
 linear-gradient(var(--grid-minor) 1px,transparent 1px),linear-gradient(90deg,var(--grid-minor) 1px,transparent 1px);
 background-size:100px 100px,100px 100px,20px 20px,20px 20px}}
button{{font:700 15px var(--body);height:40px;padding:0 16px;border-radius:6px;border:1px solid var(--ink);background:var(--ink);
color:var(--vellum);cursor:pointer;justify-self:start}}
button:focus-visible{{outline:2px solid var(--niter);outline-offset:2px}}
{concept2.PLATE_CSS}
.live{{--font-display:var(--display);--font-body:var(--body);--ink-3:#646d78;--rule-strong:#858c86}}
@media (prefers-color-scheme:dark){{:root:not([data-theme="light"]){{--orn:#d3a45f}}}}

.live .art .cast{{fill:var(--cast);opacity:1}}
</style>
<div class="wrap">
<header><h1>HoploDex chooser and icon studies</h1>
<p>Round 7, concept 2 ("Catalogue"). The owl is now engraved rather than a flat cartoon: Athena's little owl from the
Athenian tetradrachm, drawn in fine bronze lines like the rest of the page. Its body is in profile and its head turned to
face you. It has rows of feathers, talons gripping a ground line, eyes that are mostly pupil, and a small olive sprig.
A Greek key rule divides the entries, and the top bar's mark is the shield. Rough sketches: static HTML stand-ins at the app's 1200 × 800 window, not the real screen.</p></header>

<section><h2>Database chooser</h2>
<p>Entry 1 is the hoplon head-on: a braided (guilloche) rim, a band of tongues, and the owl in the field. Athena stood for
war and wisdom together, which is arms and a catalogue. The rifle drawing is unchanged for now.</p>
<div class="pair"><figure>{img("c2-list-light", "Catalogue chooser, light, with two databases")}<figcaption>Two databases, light</figcaption></figure>
<figure>{img("c2-first-dark", "Catalogue chooser, dark, first run")}<figcaption>First run, dark</figcaption></figure></div></section>

<section><h2>Program icon</h2>
<p>256, 64, 32 and 16 px on a blued-steel tile. At 32 and 16 px, where lines vanish, the rim becomes a plain ring and the
owl a solid silhouette with its eyes cut out; the top bar uses the same small version.</p>
<figure>{img("c2-icons", "Catalogue icon candidates at four sizes on light and dark backgrounds")}</figure></section>

<section><h2>Startup animation (issue #23)</h2>
<p>The Greek key draws first, from both ends. Then the shield and rifle are drawn over the grid, their fills coming in as
their lines do. One sweep draws the whole rim from 12 o'clock, its circles, braid, beads and tongues together. The owl
is engraved next, twig and beak included, and after a pause its eyes come last. The overall timing is still to be tuned:
every time is a named knob in concept2.py.</p>
<div class="live" id="live"><svg class="plate anim" viewBox="600 40 580 700" aria-hidden="true">{concept2.PLATE}</svg></div>
<button type="button" id="replay">Replay the animation</button></section>
</div>
{concept2.animate_script("#live svg")}
<script>
document.getElementById("replay").addEventListener("click", () => {{
  document.querySelector("#live svg").getAnimations({{ subtree: true }}).forEach(a => {{ a.currentTime = 0; a.play(); }});
}});
</script>"""
page = re.sub(r"\n\s*\n", "\n", page)
(HERE / "hoplodex-chooser-studies.html").write_text(page)
print(len(page))
