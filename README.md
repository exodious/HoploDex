# Branding studies (checkpoint, to be discarded)

Throwaway design sketches for issues #22 (program icon) and #23 (startup animation), and for filling the database chooser's empty right side. This orphan branch shares no history with `main`. It holds the sketches as they stood on 2026-09-28 so we can refer back to them, and is meant to be deleted once the real work lands.

`hoplodex-chooser-studies.html` is the self-contained gallery of the current round, with images and fonts embedded. Open it in a browser; its "Replay the animation" button replays the startup draw-in.

## Where it landed

Concept 2, "Catalogue". The chooser's right side is a museum catalogue page:

1. **Entry 1:** an ornate hoplon drawn head-on, with a braided (guilloche) rim, a band of tongues, and Athena's owl as its device.
2. **Entry 2:** the app's rifle drawing.

Each entry is numbered, captioned and has a scale bar, and a Greek key rule divides them.

- **Owl:** based on the Athenian tetradrachm: a little owl (no ear tufts), body in profile, head turned to face you, with a small olive sprig. It's an engraving in fine bronze lines, with rows of feathers, eyes that are mostly pupil, a hooked beak, and talons on a ground line. The coins' crescent moon is left out.
- **Icon:** the owl shield on a blued-steel tile. At 32 px and below, the owl is a solid silhouette with its eyes cut out, and the top-bar mark uses that version.
- **Animation:** everything draws in stroke by stroke: the outlines, the rim ornament, the Greek key, then the owl's lines. The owl's eyes and bronze wash come last, which is what gives it its watchful look; keep that. The Greek key draws in first, from both ends to the middle (about 1.25 s). Only then do the shield and rifle start. `studies/shots/animation/` has frames at 0.5, 1.0, 1.4, 2.6, 3.5 and 4.4 s. Each drawn line is dashed by its measured on-screen length, not `pathLength="1"`. With non-scaling strokes inside a scaled group, `pathLength` hid only part of each line (see `DRAW_JS` in `concept2.py`). The overall timing is still too fast.

## What was tried and dropped

The earlier rounds' shots are in `studies/shots/`.

- **`a-*`, `b-*`, `icons.png`:** a plain front-view hoplon (rings with a centreline) as a construction drawing (A) or on a drawing sheet (B). It reads as a rifle-scope lens.
- **`c1-*`:** "Covered", a three-quarter bronze shield in front of the firearm drawings. The angle looked wrong, and hiding the drawings doesn't work.
- **Other rejects, not in the shots:** a serpent device (suggests the Gadsden flag), a museum specimen-tag icon, a flat cartoon owl (too cute), brows over the eyes (annoyed), beaded eye rims (a sugar skull), and a prominent crescent moon (overemphasised, and open to political or religious readings).

## Open work

- **Rifle:** it needs more detail at this size. It was traced from photographs over many iterations, so changes must keep or improve its realism.
- **Animation timing.**
- **Placeholder captions:** the rifle's "1967" and materials.
- **Small icon sizes:** 16 and 32 px need hand-tuned drawings.

## Rebuilding

The scripts in `studies/` are plain Python 3 with no dependencies. Firefox is used to take the screenshots.

```sh
cd studies
python3 make_fonts.py /path/to/HoploDex   # inlines the app's OFL fonts into fonts.css
python3 concept2.py                       # c2-*.html pages and c2-icons.html
python3 owl.py                            # owl-test.html
python3 gallery.py                        # hoplodex-chooser-studies.html
firefox --headless --window-size=1200,800 --screenshot "$PWD/shots/c2-list-light.png" "file://$PWD/c2-list-light.html"
```

`drawings.json` is `DRAWINGS` from `src/features/browse/typeDrawings.ts` as it stood when these were made.
