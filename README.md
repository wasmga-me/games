# wasmga.me games

Original WebAssembly games written for [wasmga.me](https://wasmga.me). Each title compiles to a standalone `.wasm` module with no JavaScript glue code; the site's play page renders the setup, pause, and game-over UI around the module.

| Game | Play | Source | Written in |
| --- | --- | --- | --- |
| 2048 | [wasmga.me/games/2048](https://wasmga.me/games/2048) | [2048.c](2048.c) | C |
| Bankshot | [wasmga.me/games/bankshot](https://wasmga.me/games/bankshot) | [bankshot/](bankshot) | Rust |
| Boarding | [wasmga.me/games/boarding](https://wasmga.me/games/boarding) | [boarding/](boarding) | Rust |
| Branchwise | [wasmga.me/games/branchwise](https://wasmga.me/games/branchwise) | [branchwise/](branchwise) | Rust |
| Breakwall | [wasmga.me/games/breakwall](https://wasmga.me/games/breakwall) | [breakwall/](breakwall) | Rust |
| Decant | [wasmga.me/games/decant](https://wasmga.me/games/decant) | [decant/](decant) | Rust |
| Facets | [wasmga.me/games/facets](https://wasmga.me/games/facets) | [facets/](facets) | Rust |
| Flipside | [wasmga.me/games/flipside](https://wasmga.me/games/flipside) | [flipside.c](flipside.c) | C |
| Flowline | [wasmga.me/games/flowline](https://wasmga.me/games/flowline) | [flowline/](flowline) | Rust |
| Greenside | [wasmga.me/games/greenside](https://wasmga.me/games/greenside) | [greenside/](greenside) | Rust |
| Hexpile | [wasmga.me/games/hexpile](https://wasmga.me/games/hexpile) | [hexpile/](hexpile) | Rust |
| Higher Floor | [wasmga.me/games/higher-floor](https://wasmga.me/games/higher-floor) | [higher-floor/](higher-floor) | Rust |
| Hive Guard | [wasmga.me/games/hive-guard](https://wasmga.me/games/hive-guard) | [hive-guard/](hive-guard) | Rust |
| Offgrid | [wasmga.me/games/offgrid](https://wasmga.me/games/offgrid) | [offgrid/](offgrid) | Rust |
| Pantry Solitaire | [wasmga.me/games/pantry](https://wasmga.me/games/pantry) | [pantry/](pantry) | Rust |
| Partial Combat: Clickbait Tactics | [wasmga.me/games/partial-combat](https://wasmga.me/games/partial-combat) | [partial-combat/](partial-combat) | Rust |
| Pinwork | [wasmga.me/games/pinwork](https://wasmga.me/games/pinwork) | [pinwork/](pinwork) | Rust |
| Sapper | [wasmga.me/games/sapper](https://wasmga.me/games/sapper) | [sapper.c](sapper.c) | C |
| Slider | [wasmga.me/games/slider](https://wasmga.me/games/slider) | [slider/](slider) | Rust |
| Snake | [wasmga.me/games/snake](https://wasmga.me/games/snake) | [snake.c](snake.c) | C |
| Sourdough Solitaire | [wasmga.me/games/sourdough](https://wasmga.me/games/sourdough) | [sourdough.c](sourdough.c) | C |
| Sudoku | [wasmga.me/games/sudoku](https://wasmga.me/games/sudoku) | [sudoku/](sudoku) | Rust |
| Tilestack | [wasmga.me/games/tilestack](https://wasmga.me/games/tilestack) | [tilestack/](tilestack) | Rust |
| Unbolt | [wasmga.me/games/unbolt](https://wasmga.me/games/unbolt) | [unbolt/](unbolt) | Rust |
| Unpark | [wasmga.me/games/unpark](https://wasmga.me/games/unpark) | [unpark/](unpark) | Rust |
| Updraft | [wasmga.me/games/updraft](https://wasmga.me/games/updraft) | [updraft/](updraft) | Rust |
| Zork I, II and III (Infocom's) | [wasmga.me/games/zork1](https://wasmga.me/games/zork1) | [zmachine/](zmachine), [zork1/](zork1), [zork2/](zork2), [zork3/](zork3) | Rust |

`branchwise/` links [libgit2](https://github.com/libgit2/libgit2) 1.9.1 (GPL-2.0 with linking exception), compiled to WebAssembly via `branchwise/libgit2/build.sh` (which downloads `wasi-sdk` and `libgit2` and applies `wasi.diff`); each puzzle runs real Git operations against an in-memory filesystem. Running `node branchwise/tools/check.mjs` verifies the solution for every lesson.

`partial-combat/` uses hardware-accelerated 3D rendering: the Wasm module emits a scene description each frame (`src/scene.rs`), which the play page renders via WebGL2 while compositing the module's 2D framebuffer on top as a HUD. Its 3D models and particle textures come from CC0 asset packs by [Quaternius](https://quaternius.com) and [Kenney](https://kenney.nl); running `COMBAT_OUT=out node partial-combat/assets/pack.mjs` (after `npm install` in that folder) downloads the source packs and generates the `.glb` bundles and `fx.png` sprite sheet in `out/`. Running `cargo test --release -- --nocapture` inside `partial-combat/` simulates bot playthroughs across all campaign levels and endless mode.

`greenside/` and `updraft/` include automated test bots (`src/bot.rs`): `cargo test --release` verifies that a skilled bot clears every Greenside hole within par and that Updraft's generated platforms remain reachable across every difficulty pace. `cargo run --release --example par` in `greenside/` reports stroke counts per hole for skilled and imprecise bots, and `cargo run --release --example climb` in `updraft/` measures maximum bot altitude.

`tilestack/art/riichi/` contains tile artwork from FluffyStuff's [riichi-mahjong-tiles](https://github.com/FluffyStuff/riichi-mahjong-tiles) (CC0 / public domain), supplemented by original flower, season, and white dragon tiles in `tilestack/art/extra/`. Running `node tilestack/art/build.mjs` (requires `sharp`) rasterizes the SVGs into `tilestack/src/art.bin`, which is embedded into the compiled binary.

`bankshot/`, `boarding/`, `higher-floor/`, `hive-guard/`, `pinwork/`, and `unpark/` use pre-rendered sprites generated from [Kenney](https://kenney.nl)'s CC0 3D kits: each game's `art/jobs.py` defines render jobs processed by `tools/render3d/` (see its `README.md`) to produce `src/art.png`.

`pinwork/art/draw.py` draws Pinwork's knight, orc, chests and coins as SVG;
`pinwork/art/build.sh` renders them with sharp and packs them into
`pinwork/src/art.png` with `tools/render3d/pack.py`.

`wgdemo/` is a minimal reference game built on the Rust engine, intended as a starter template for new titles.

`zmachine/` implements a Z-machine (version 3) interpreter on the Rust engine for running Infocom story files; `zork1/`, `zork2/`, and `zork3/` each package a story file into a standalone module. Running `cargo test` in `zmachine/` tests the opening sequences of all three games.

## Engines

- C games compile against the freestanding headers in [wasmgame-wg-c](https://github.com/wasmga-me/wasmgame-wg-c) (`wg.h` defines the exported Wasm ABI consumed by the host page), pinned by commit hash in `wg-c.rev`.
- Rust games depend on [wasmgame-wg](https://github.com/wasmga-me/wasmgame-wg), pinned per crate via `Cargo.lock`.

## Build

```sh
sh build.sh
```

Building C games requires `clang` and `wasm-ld` with `wasm32` target support (LLVM 15+), and building Rust games requires Cargo with `rustup target add wasm32-unknown-unknown`. Compiled binaries are written to `out/<id>/<id>.wasm`.

## Gameplay checks

Run `python3 check-ad-games.py` to check Pinwork, Decant, Unpark, Higher Floor,
Flowline, Boarding, Unbolt, Hive Guard, Hexpile and Bankshot. The native Rust tests replay
campaign solutions and check failure states, retry, saves and physics regressions.
Pass game IDs to run only those suites, for example
`python3 check-ad-games.py boarding flowline`.

Boarding moves passengers into matching buses through a five-seat waiting bench.
Flowline requires a sealed route to the garden before the timer expires while
keeping the hero dry. Unpark ends an attempt after three collisions. Higher Floor
reserves the marked boss for last; Pinwork seals fluid barriers and teaches its
eight mechanisms in eight distinct chambers. Decant uses increasing solution-length
targets, and Unbolt retains a verified winning screw-removal sequence for every deal.

Bankshot follows [Mr Bullet](https://poki.com/en/g/mr-bullet)'s shooting rules and
opening lessons: direct shots, aligned enemies, stairs and breakable wooden cover.
Its 30 fixed layouts replace random rooms. Four bullets allow mistakes, while
visible shot targets reward efficient clears with up to three stars. Tests check
campaign wins, reachable losses, retry, cover saves and consistent aiming tolerance.

## Licence

MIT; see [LICENSE](LICENSE). In-game Latin text is rendered from [Inter](https://rsms.me/inter/) glyphs and Simplified Chinese text from [Noto Sans SC](https://fonts.google.com/noto/specimen/Noto+Sans+SC) glyphs (`.zh.h` and `zh_glyphs` files), both licensed under the SIL Open Font License 1.1. The Zork story files (`zork1.z3`, `zork2.z3`, `zork3.z3`) are unmodified Infocom originals from [historicalsource](https://github.com/historicalsource), released by Microsoft under the MIT License (see `STORY-LICENSE` in each game directory).
