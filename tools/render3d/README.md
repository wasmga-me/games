# render3d: 3D models into sprite sheets

Some of our games draw art rendered from free 3D models (Kenney's kits,
CC0) instead of shapes. The models are rendered once here, in headless
Chromium with three.js, and only the packed PNG sheet goes into the game.

```sh
npm install                      # three.js, once
sh fetch.sh car-kit nature-kit   # Kenney packs into models/ (not committed)
python3 ../../<game>/art/jobs.py # from the game's art/ folder: writes jobs.json
node render.mjs ../../<game>/art/jobs.json /tmp/<game>
python3 pack.py ../../<game>/art/jobs.json /tmp/<game> ../../<game>/src/art.png ../../<game>/src/art.rs --colours 256
```

`node render.mjs --bbox MODEL` and `--anims MODEL` print a model's bounds
and its animations. render.mjs needs Playwright's Chromium (the one in
/opt/pw-browsers when present).

A job renders one PNG, `name`.png:

- `parts`: models, each `{url, scale, pos, rot, colors, anim, t}`; `url` is
  under `models/`, `colors` maps a material name or `#rrggbb` to a new
  colour, `anim` and `t` pose a rigged model at time `t` of that animation.
  `texture` swaps a textured model's texture for another (`jobs/...` is
  next to the jobs file). `hold: [k, node]` puts the part in node `node`
  (such as `arm-right`) of part k, so it moves with that arm's pose.
  `{bus: "#rrggbb", len, wheel}` builds a low-poly bus in the Car Kit's
  style, `{hive: true}` a straw beehive.
- `turn`: degrees about the up axis (the models' fronts face +z, toward the
  camera at `yaw` 0).
- `pitch`, `yaw`: the camera's angle down from level and round from +z;
  orthographic, `ppu` pixels per model unit, `w` x `h` pixels, centred on
  `center` (the origin by default).
- `shadow: false` leaves out the shadow cast on the ground;
  `shadowOpacity`, `hemi`, `sun`, `sunPos` tune the light.

pack.py crops each render to what it draws, packs them into one sheet and
writes `art.rs`: the sheet's bytes, each sprite's box, where the model's
origin lands in it and its pixels per unit, and a constant per job name.
The game loads it with `wg::Image::png(art::SHEET)` and `crop`s each sprite.
