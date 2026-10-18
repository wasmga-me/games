# wasmga.me games

The games written for [wasmga.me](https://wasmga.me). Each one builds to a
single `.wasm` file with no JavaScript glue; the site's play page draws the
setup, pause and game over screens around it.

| Game | Play | Source | Written in |
| --- | --- | --- | --- |
| 2048 | [wasmga.me/games/2048](https://wasmga.me/games/2048) | [2048.c](2048.c) | C |
| Breakwall | [wasmga.me/games/breakwall](https://wasmga.me/games/breakwall) | [breakwall/](breakwall) | Rust |
| Facets | [wasmga.me/games/facets](https://wasmga.me/games/facets) | [facets/](facets) | Rust |
| Flipside | [wasmga.me/games/flipside](https://wasmga.me/games/flipside) | [flipside.c](flipside.c) | C |
| Sapper | [wasmga.me/games/sapper](https://wasmga.me/games/sapper) | [sapper.c](sapper.c) | C |
| Slider | [wasmga.me/games/slider](https://wasmga.me/games/slider) | [slider/](slider) | Rust |
| Snake | [wasmga.me/games/snake](https://wasmga.me/games/snake) | [snake.c](snake.c) | C |
| Sourdough Solitaire | [wasmga.me/games/sourdough](https://wasmga.me/games/sourdough) | [sourdough.c](sourdough.c) | C |
| Sudoku | [wasmga.me/games/sudoku](https://wasmga.me/games/sudoku) | [sudoku/](sudoku) | Rust |

`wgdemo/` is the smallest whole game on the Rust engine, to copy for a new one.

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
[Inter](https://rsms.me/inter/), and its other scripts from glyphs of Noto
Sans JP, KR and SC (the `.glyphs.h` and `glyphs.rs` files), all under the SIL
Open Font License 1.1.
