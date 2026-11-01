# render3d: offline 3D-to-sprite-sheet pipeline

Several native games use pre-rendered 2D sprites generated from CC0 3D models (Kenney asset kits) rather than procedural vector shapes. Models are rendered offline once using Three.js inside headless Chromium, and only the packed PNG sprite sheet is embedded into the final game binary.

```sh
npm install                      # install three.js (once)
sh fetch.sh car-kit nature-kit   # download Kenney packs into models/ (gitignored)
python3 ../../<game>/art/jobs.py # run from the game's art/ pipeline to emit jobs.json
node render.mjs ../../<game>/art/jobs.json /tmp/<game>
python3 pack.py ../../<game>/art/jobs.json /tmp/<game> ../../<game>/src/art.png ../../<game>/src/art.rs --colours 256
```

`node render.mjs --bbox MODEL` and `--anims MODEL` inspect a model's bounding box and available animation clips. `render.mjs` requires Playwright's Chromium binary (automatically using `/opt/pw-browsers` when present).

Each job entry renders a single `<name>.png` image:

- `parts`: array of model instances `{url, scale, pos, rot, colors, anim, t}`, where `url` is relative to `models/`, `colors` remaps material names or `#rrggbb` hex values to replacement colors, and `anim` + `t` sample a skeletal animation at timestamp `t`. `texture` overrides a textured mesh's diffuse map (`jobs/...` resolves relative to the jobs file). `hold: [k, node]` parents the part to bone/node `node` (e.g., `arm-right`) on part `k` so it follows that limb's pose. Procedural helpers `{bus: "#rrggbb", len, wheel}` and `{hive: true}` generate a Car Kit–style low-poly bus and a straw beehive, respectively.
- `turn`: rotation in degrees around the vertical Y axis (models face `+Z` by default, pointing toward the camera at `yaw` 0).
- `pitch`, `yaw`: orthographic camera elevation angle downward from the horizon and azimuth angle around `+Z`, rendered at `ppu` pixels per model unit inside a `w` × `h` viewport centered on `center` (defaults to the origin).
- `shadow: false` disables the ground contact shadow; `shadowOpacity`, `hemi`, `sun`, and `sunPos` configure lighting and shadow intensity.

`pack.py` trims transparent padding from each rendered frame, packs all sprites into a single atlas, and generates `art.rs` containing the embedded PNG bytes, bounding rectangles, projected model-origin anchor coordinates, pixels-per-unit scale factors, and an index constant for each job name. Games decode the atlas at startup via `wg::Image::png(art::SHEET)` and slice individual sprites with `crop`.
