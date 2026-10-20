# wasmga.me games

The games written for [wasmga.me](https://wasmga.me). Each one builds to a
single `.wasm` file with no JavaScript glue; the site's play page draws the
setup, pause and game over screens around it.

| Game | Play | Source | Written in |
| --- | --- | --- | --- |
| 2048 | [wasmga.me/games/2048](https://wasmga.me/games/2048) | [2048.c](2048.c) | C |
| Branchwise | [wasmga.me/games/branchwise](https://wasmga.me/games/branchwise) | [branchwise/](branchwise) | Rust |
| Breakwall | [wasmga.me/games/breakwall](https://wasmga.me/games/breakwall) | [breakwall/](breakwall) | Rust |
| Facets | [wasmga.me/games/facets](https://wasmga.me/games/facets) | [facets/](facets) | Rust |
| Flipside | [wasmga.me/games/flipside](https://wasmga.me/games/flipside) | [flipside.c](flipside.c) | C |
| Pantry Solitaire | [wasmga.me/games/pantry](https://wasmga.me/games/pantry) | [pantry/](pantry) | Rust |
| Sapper | [wasmga.me/games/sapper](https://wasmga.me/games/sapper) | [sapper.c](sapper.c) | C |
| Slider | [wasmga.me/games/slider](https://wasmga.me/games/slider) | [slider/](slider) | Rust |
| Snake | [wasmga.me/games/snake](https://wasmga.me/games/snake) | [snake.c](snake.c) | C |
| Sourdough Solitaire | [wasmga.me/games/sourdough](https://wasmga.me/games/sourdough) | [sourdough.c](sourdough.c) | C |
| Sudoku | [wasmga.me/games/sudoku](https://wasmga.me/games/sudoku) | [sudoku/](sudoku) | Rust |
| Tilestack | [wasmga.me/games/tilestack](https://wasmga.me/games/tilestack) | [tilestack/](tilestack) | Rust |
| Zork I, II and III (Infocom's) | [wasmga.me/games/zork1](https://wasmga.me/games/zork1) | [zmachine/](zmachine), [zork1/](zork1), [zork2/](zork2), [zork3/](zork3) | Rust |

`branchwise/` links [libgit2](https://github.com/libgit2/libgit2) 1.9.1 (GPL-2.0
with a linking exception), built for WebAssembly by `branchwise/libgit2/build.sh`,
which fetches wasi-sdk and libgit2 and applies `wasi.diff`; each lesson runs real
git on an in-memory file system. `node branchwise/tools/check.mjs` plays every
lesson's solution.

`tilestack/art/riichi/` holds the tile pictures of FluffyStuff's
[riichi-mahjong-tiles](https://github.com/FluffyStuff/riichi-mahjong-tiles)
(public domain, CC0); the flowers, seasons and white dragon in
`tilestack/art/extra/` are ours. `node tilestack/art/build.mjs` (needs sharp)
renders them all into `tilestack/src/art.bin`, which the module embeds.

`wgdemo/` is the smallest whole game on the Rust engine, to copy for a new one.

`zmachine/` is a Z-machine (version 3) on the Rust engine, which plays
Infocom's story files; `zork1/`, `zork2/` and `zork3/` each build one into a
module. `cargo test` in `zmachine/` plays the openings of all three.

## Engines

- C games include the headers of [wasmgame-wg-c](https://github.com/wasmga-me/wasmgame-wg-c)
  (`wg.h` describes every export the page reads). `wg-c.rev` pins its commit.
- Rust games depend on [wasmgame-wg](https://github.com/wasmga-me/wasmgame-wg);
  each game's `Cargo.lock` pins its commit.

## Build

```sh
sh build.sh
```

Needs clang and wasm-ld with the wasm32 target (LLVM 15+) for the C games, and
cargo with `rustup target add wasm32-unknown-unknown` for the Rust ones. The
modules land in `out/<id>/<id>.wasm`.

## Licence

MIT, see [LICENSE](LICENSE). The text in every game is drawn from glyphs of
[Inter](https://rsms.me/inter/), and its Chinese from glyphs of
[Noto Sans SC](https://fonts.google.com/noto/specimen/Noto+Sans+SC) (the
`.zh.h` and `zh_glyphs` files), both under the SIL Open Font License 1.1.
The Zork story files (`zork1.z3`, `zork2.z3`, `zork3.z3`) are Infocom's,
unchanged, from [historicalsource](https://github.com/historicalsource), where
Microsoft published them under the MIT licence (`STORY-LICENSE` next to each).
