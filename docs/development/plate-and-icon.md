# The chooser's drawing and the program icon

_Part of the [development docs](../../DEVELOPMENT.md)._

## Tuning the chooser's drawing

The database chooser's catalogue plate (the hoplon with its owl, the Greek
key and the firearm drawings) draws itself in stroke by stroke, then cycles
entry 2 through the firearm drawings until a database is opened. Every time
it uses, and how one drawing gives way to the next (`CYCLE_STYLE`), is in
`src/features/databases/plate/timing.ts`, one value per line. The app builds
in whatever that file holds.

To judge a change, watch it in the tuner rather than editing numbers blind:

```bash
npm run tuner         # http://localhost:1430: sliders, a scrubbable timeline, presets, Save
npm run tuner:build   # -> dist-tuner/plate-tuner.html, one file to open anywhere (no Save)
```

The tuner plays the app's own plate and animation code. Paused, it shows the
moment on the timeline with the new values; playing, it starts again when
you let go of a slider. "The whole loop" stretches the timeline over the
cycle, and the presets under "Changing drawings" each play one way of
changing drawings from just before the first change. **Save to timing.ts**
(served by `npm run tuner` only) writes what you changed into the file, and
the page reloads with them as its values; the built page lists your changes
to copy into it instead. A link can open it on a preset and paused at a
moment: `?preset=2&view=cycle&at=8.5`.

Because Save rewrites a source file, `npm run tuner` listens on loopback
only (`localhost:1430`) and accepts a save only from its own page: each run
makes a random token, puts it in the page it serves, and refuses a request
without it, from another `Origin` or `Host`, that isn't `application/json`,
is over 64 KiB, or holds anything but known timing names with numbers (a
switch, a style name) in range. So open the tuner at the address it prints,
and reload the page after restarting the server (an older page's token no
longer works).

Inside the [development container](dev-container.md)
the tuner's server can't be reached from your browser, so there use
`npm run tuner:build` and open `dist-tuner/plate-tuner.html` from your
checkout.

Entry 2's drawings, their captions and their real lengths (which size the
scale bars) are in `plate/entries.ts`; the artwork is `plate/PlateArt.tsx`.
The app shows the drawings in an order shuffled once at startup, and the
draw-in draws whichever comes first; each keeps its catalogue number (2 to
5, from its place in `PLATE_ENTRIES`). The tuner always plays them in
`PLATE_ENTRIES`' order, starting with the rifle.

## Program icon

`src-tauri/icons/` is generated from `tools/icons/AppIcon.tsx`, which draws
the chooser plate's shield and owl on a blued-steel tile. After changing
either, regenerate every size (16 to 1024 px), `icon.ico` and `icon.icns`:

```bash
npm run icons
```

At 32 px and below the icon has its own drawing, a solid owl on a plain rim,
as the top bar's mark does (`src/features/app/BrandMark.tsx`).
`src-tauri/icons/source/` keeps the two drawings as SVG, for reference.
