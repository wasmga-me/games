# cartoon: drawn sprites for the native games

Bankshot, Higher Floor and Hive Guard draw their people, monsters and props
as cartoons: outlined, shaded shapes in the chunky style of mobile puzzle
games, written as SVG by each game's `art/draw.py` with `cartoon.py`, then
turned into PNGs in headless Chromium and packed into the game's sheet by
render3d's `pack.py`:

```sh
python3 ../../<game>/art/draw.py /tmp/<game>-svg   # SVGs and jobs.json
node render.mjs /tmp/<game>-svg/jobs.json /tmp/<game>
python3 ../render3d/pack.py /tmp/<game>-svg/jobs.json /tmp/<game> ../../<game>/src/art.png ../../<game>/src/art.rs --colours 256
```

render.mjs needs Playwright's Chromium (the one in /opt/pw-browsers when
present). Sprites are drawn facing right in art units, 100 to a model unit,
with the origin on the ground under the middle, so the games place them as
they did render3d's.

`cartoon.py` has the palette, shape paths (`circle`, `rrect`, `capsule`,
`smooth` and more), `Art.part` (a shape with its shade, highlight and
outline), eyes and mouths, `person` (a big-headed little person with hooks
for hair, masks, clothes and what it holds, in any pose or knocked down),
coins and trees.
